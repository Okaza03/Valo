//! One decoder thread per open file. Video frames go to a queue the UI pops
//! from; audio goes to the output ring buffer.
use std::collections::VecDeque;
use std::ffi::{c_int, c_void, CString};
use std::path::PathBuf;
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use anyhow::{anyhow, bail, Result};
use ffmpeg::format::sample::{Sample, Type as SampleType};
use ffmpeg::format::Pixel;
use ffmpeg::media::Type;
use ffmpeg::software::{resampling, scaling};
use ffmpeg::{codec, frame, ChannelLayout, Dictionary, Packet, Rational};
use ffmpeg_next as ffmpeg;

use crate::audio::AudioShared;

const MAX_QUEUED_FRAMES: usize = 5;
/// While little audio is buffered, read on past [`MAX_QUEUED_FRAMES`] up to
/// this many frames: some files put a second or so of video before their first
/// audio, and the clock (driven by audio) would never start.
const MAX_STARVED_FRAMES: usize = 48;
const MAX_VIDEO_WIDTH: u32 = 1920;

/// Where a video comes from.
#[derive(Clone, Debug)]
pub enum Source {
    File(PathBuf),
    Url { url: String, auth: Option<String> },
}

impl Source {
    pub fn name(&self) -> String {
        match self {
            Source::File(p) => p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            Source::Url { url, .. } => {
                crate::util::percent_decode(url.rsplit('/').next().unwrap_or(url))
            }
        }
    }
}

pub struct VFrame {
    pub t: f64,
    pub w: usize,
    pub h: usize,
    pub rgba: Vec<u8>,
}

pub struct Meta {
    pub duration: f64,
    pub has_audio: bool,
    /// What's in the file, for the Info panel.
    pub specs: crate::specs::Specs,
}

enum Cmd {
    Seek(f64),
}

struct Shared {
    vq: Mutex<VecDeque<VFrame>>,
    eof: AtomicBool,
    quit: AtomicBool,
    preview: AtomicBool,
    base: AtomicU64, // f64 bits: media time at audio-clock zero
}

impl Shared {
    fn new() -> Self {
        Self {
            vq: Mutex::new(VecDeque::new()),
            eof: AtomicBool::new(false),
            quit: AtomicBool::new(false),
            preview: AtomicBool::new(false),
            base: AtomicU64::new(0f64.to_bits()),
        }
    }
    fn base(&self) -> f64 {
        f64::from_bits(self.base.load(Relaxed))
    }
    fn set_base(&self, t: f64) {
        self.base.store(t.to_bits(), Relaxed);
    }
}

pub struct Player {
    sh: Arc<Shared>,
    audio: Arc<AudioShared>,
    tx: Sender<Cmd>,
    thread: Option<JoinHandle<()>>,
    pub meta: Meta,
    pub paused: bool,
    wall: f64, // clock for files without an audio track
}

impl Player {
    /// Blocks while the source is probed (can take a moment over the network).
    pub fn open(src: &Source, audio: Arc<AudioShared>) -> Result<Self> {
        audio.reset();
        audio.set_paused(false);
        let sh = Arc::new(Shared::new());
        let (tx, rx) = mpsc::channel();
        let (mtx, mrx) = mpsc::channel();
        let (p, a, s) = (src.clone(), audio.clone(), sh.clone());
        let thread = thread::spawn(move || decode_thread(p, a, s, rx, mtx));
        let meta = mrx
            .recv()
            .map_err(|_| anyhow!("decoder thread crashed"))?
            .map_err(|e| anyhow!(e))?;
        Ok(Self {
            sh,
            audio,
            tx,
            thread: Some(thread),
            meta,
            paused: false,
            wall: 0.0,
        })
    }

    pub fn clock(&self) -> f64 {
        let t = if self.meta.has_audio {
            self.sh.base() + self.audio.played_secs()
        } else {
            self.sh.base() + self.wall
        };
        if self.meta.duration > 0.0 {
            t.min(self.meta.duration)
        } else {
            t
        }
    }

    pub fn tick(&mut self, dt: f64) {
        if !self.paused && !self.meta.has_audio {
            self.wall += dt;
        }
    }

    pub fn set_paused(&mut self, p: bool) {
        self.paused = p;
        self.audio.set_paused(p);
    }

    pub fn seek(&mut self, t: f64) {
        let t = t.max(0.0);
        self.sh.set_base(t);
        self.wall = 0.0;
        self.audio.reset();
        self.sh.vq.lock().unwrap().clear();
        self.sh.preview.store(true, Relaxed);
        self.sh.eof.store(false, Relaxed);
        let _ = self.tx.send(Cmd::Seek(t));
    }

    /// The newest frame that is due, dropping any that are already late.
    pub fn take_frame(&self) -> Option<VFrame> {
        let now = self.clock();
        let mut q = self.sh.vq.lock().unwrap();
        if self.paused {
            // after a seek while paused, show the first frame we land on
            if self.sh.preview.load(Relaxed) {
                if let Some(f) = q.pop_front() {
                    self.sh.preview.store(false, Relaxed);
                    return Some(f);
                }
            }
            return None;
        }
        self.sh.preview.store(false, Relaxed);
        let mut out = None;
        while q.front().is_some_and(|f| f.t <= now) {
            out = q.pop_front();
        }
        out
    }

    pub fn finished(&self) -> bool {
        self.sh.eof.load(Relaxed) && {
            let q = self.sh.vq.lock().unwrap();
            q.is_empty() || (self.meta.has_audio && self.audio.buffered_secs() < 0.02)
        }
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.sh.quit.store(true, Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        self.audio.reset();
    }
}

// ---------------------------------------------------------------- decoder

struct ADec {
    idx: usize,
    tb: Rational,
    dec: ffmpeg::decoder::Audio,
    rs: Option<resampling::Context>,
}

struct Decoder {
    ictx: ffmpeg::format::context::Input,
    vidx: usize,
    vtb: Rational,
    vdec: ffmpeg::decoder::Video,
    scaler: Option<scaling::Context>,
    audio: Option<ADec>,
    duration: f64,
    need_base: bool,
    specs: Option<crate::specs::Specs>,
}

fn decode_thread(
    src: Source,
    audio: Arc<AudioShared>,
    sh: Arc<Shared>,
    rx: Receiver<Cmd>,
    mtx: Sender<std::result::Result<Meta, String>>,
) {
    match Decoder::open(&src, &audio, &sh) {
        Err(e) => {
            let _ = mtx.send(Err(format!("{e:#}")));
        }
        Ok(mut d) => {
            let _ = mtx.send(Ok(Meta {
                duration: d.duration,
                has_audio: d.audio.is_some(),
                specs: d.specs.take().unwrap_or_default(),
            }));
            if let Err(e) = d.run(&sh, &audio, &rx) {
                eprintln!("decoder stopped: {e:#}");
                sh.eof.store(true, Relaxed);
            }
        }
    }
}

fn open_audio(ictx: &ffmpeg::format::context::Input) -> Result<Option<ADec>> {
    let Some(s) = ictx.streams().best(Type::Audio) else {
        return Ok(None);
    };
    let (idx, tb, par) = (s.index(), s.time_base(), s.parameters());
    let dec = codec::context::Context::from_parameters(par)?
        .decoder()
        .audio()?;
    Ok(Some(ADec {
        idx,
        tb,
        dec,
        rs: None,
    }))
}

fn fit(w: u32, h: u32, max_w: u32) -> (u32, u32) {
    if w <= max_w {
        (w & !1, h & !1)
    } else {
        let s = max_w as f64 / w as f64;
        (max_w & !1, ((h as f64 * s) as u32) & !1)
    }
}

fn secs(pts: i64, tb: Rational) -> f64 {
    pts as f64 * tb.numerator() as f64 / tb.denominator() as f64
}

unsafe extern "C" fn interrupted(opaque: *mut c_void) -> c_int {
    (*(opaque as *const Shared)).quit.load(Relaxed) as c_int
}

/// Like `ffmpeg::format::input_with_dictionary`, plus an interrupt callback so
/// a stalled network read gives up as soon as the player is dropped.
fn open_input(src: &Source, sh: &Arc<Shared>) -> Result<ffmpeg::format::context::Input> {
    let mut opts = Dictionary::new();
    let url = match src {
        Source::File(p) => p.to_string_lossy().into_owned(),
        Source::Url { url, auth } => {
            opts.set("reconnect", "1");
            opts.set("reconnect_on_network_error", "1");
            opts.set("reconnect_delay_max", "4");
            opts.set("rw_timeout", "20000000");
            opts.set("user_agent", concat!("Valo/", env!("CARGO_PKG_VERSION")));
            if let Some(a) = auth {
                opts.set("headers", &format!("Authorization: {a}\r\n"));
            }
            url.clone()
        }
    };
    let c_url = CString::new(url)?;
    // SAFETY: mirrors ffmpeg-next's own input_* helpers; `opaque` points into
    // an Arc<Shared> we leak a reference to, so it outlives the context.
    unsafe {
        use ffmpeg::sys;
        let mut ps = sys::avformat_alloc_context();
        if ps.is_null() {
            bail!("out of memory");
        }
        (*ps).interrupt_callback = sys::AVIOInterruptCB {
            callback: Some(interrupted),
            opaque: Arc::into_raw(sh.clone()) as *mut c_void,
        };
        let mut o = opts.disown();
        let r = sys::avformat_open_input(&mut ps, c_url.as_ptr(), ptr::null(), &mut o);
        drop(Dictionary::own(o));
        if r != 0 {
            return Err(ffmpeg::Error::from(r).into()); // ps is freed on failure
        }
        let r = sys::avformat_find_stream_info(ps, ptr::null_mut());
        if r < 0 {
            sys::avformat_close_input(&mut ps);
            return Err(ffmpeg::Error::from(r).into());
        }
        Ok(ffmpeg::format::context::Input::wrap(ps))
    }
}

/// "1080p" for a frame size; wide films (1920x800) count by width.
fn quality(w: i32, h: i32) -> String {
    match (w, h) {
        (w, h) if w >= 3800 || h >= 2100 => "2160p".into(),
        (w, h) if w >= 2500 || h >= 1400 => "1440p".into(),
        (w, h) if w >= 1900 || h >= 1060 => "1080p".into(),
        (w, h) if w >= 1260 || h >= 700 => "720p".into(),
        (_, h) => format!("{h}p"),
    }
}

/// Read what's in a video from its headers, without decoding: container,
/// codecs, resolution, audio channels, bitrate, size and duration.
/// Fills in the details page when the server doesn't send `file`.
pub fn probe(src: &Source) -> Result<(crate::api::FileInfo, Option<f64>)> {
    ffmpeg::init()?;
    let sh = Arc::new(Shared::new());
    let ictx = open_input(src, &sh)?;
    let mut info = crate::api::FileInfo {
        container: Some(crate::specs::container(&ictx, &src.name())),
        ..Default::default()
    };
    if let Some(v) = ictx.streams().best(Type::Video) {
        let par = v.parameters();
        info.video_codec = Some(par.id().name().to_owned());
        // SAFETY: par wraps the stream's live AVCodecParameters
        let (w, h) = unsafe { ((*par.as_ptr()).width, (*par.as_ptr()).height) };
        if h > 0 {
            info.quality = Some(quality(w, h));
        }
    }
    if let Some(a) = ictx.streams().best(Type::Audio) {
        let par = a.parameters();
        info.audio_codec = Some(par.id().name().to_owned());
        // SAFETY: as above
        let ch = unsafe { (*par.as_ptr()).ch_layout.nb_channels };
        info.audio_channels = u32::try_from(ch).ok().filter(|c| *c > 0);
    }
    info.bitrate = u64::try_from(ictx.bit_rate()).ok().filter(|b| *b > 0);
    info.size = crate::specs::size(&ictx);
    let d = ictx.duration();
    let duration = (d > 0).then(|| d as f64 / f64::from(ffmpeg::sys::AV_TIME_BASE));
    Ok((info, duration))
}

impl Decoder {
    fn open(src: &Source, audio: &AudioShared, sh: &Arc<Shared>) -> Result<Self> {
        ffmpeg::init()?;
        let ictx = open_input(src, sh)?;
        let (vidx, vtb, vpar) = {
            let s = ictx
                .streams()
                .best(Type::Video)
                .ok_or_else(|| anyhow!("no video stream"))?;
            (s.index(), s.time_base(), s.parameters())
        };
        let vdec = codec::context::Context::from_parameters(vpar)?
            .decoder()
            .video()?;
        let adec = if audio.available {
            open_audio(&ictx).ok().flatten()
        } else {
            None
        };
        let d = ictx.duration();
        let specs = crate::specs::read(&ictx, &src.name());
        Ok(Self {
            specs: Some(specs),
            duration: if d > 0 { d as f64 / 1e6 } else { 0.0 },
            ictx,
            vidx,
            vtb,
            vdec,
            scaler: None,
            audio: adec,
            need_base: true,
        })
    }

    fn run(&mut self, sh: &Shared, audio: &AudioShared, rx: &Receiver<Cmd>) -> Result<()> {
        let mut skip_until = 0.0;
        let mut drained = false;
        let mut pkt = Packet::empty();
        let mut errors = 0;
        while !sh.quit.load(Relaxed) {
            while let Ok(Cmd::Seek(t)) = rx.try_recv() {
                let ts = (t * 1e6) as i64;
                if self.ictx.seek(ts, i64::MIN..ts).is_ok() {
                    self.vdec.flush();
                    if let Some(a) = self.audio.as_mut() {
                        a.dec.flush();
                    }
                    sh.vq.lock().unwrap().clear();
                    audio.reset();
                    sh.set_base(t);
                    skip_until = t;
                    self.need_base = false;
                    drained = false;
                    sh.eof.store(false, Relaxed);
                }
            }
            if drained {
                thread::sleep(Duration::from_millis(20));
                continue;
            }
            let queued = sh.vq.lock().unwrap().len();
            let starved = self.audio.is_some() && audio.buffered_secs() < 0.3;
            let full = if starved {
                queued >= MAX_STARVED_FRAMES
            } else {
                queued >= MAX_QUEUED_FRAMES
            };
            if full || audio.buffered_secs() > 1.0 {
                thread::sleep(Duration::from_millis(4));
                continue;
            }
            match pkt.read(&mut self.ictx) {
                Ok(()) => {
                    errors = 0;
                    let i = pkt.stream();
                    if i == self.vidx {
                        if self.vdec.send_packet(&pkt).is_ok() {
                            self.drain_video(sh, skip_until)?;
                        }
                    } else if self.audio.as_ref().is_some_and(|a| a.idx == i)
                        && self.audio.as_mut().unwrap().dec.send_packet(&pkt).is_ok()
                    {
                        self.drain_audio(audio, skip_until)?;
                    }
                }
                Err(ffmpeg::Error::Eof) => {
                    let _ = self.vdec.send_eof();
                    self.drain_video(sh, skip_until)?;
                    if let Some(a) = self.audio.as_mut() {
                        let _ = a.dec.send_eof();
                    }
                    self.drain_audio(audio, skip_until)?;
                    drained = true;
                    sh.eof.store(true, Relaxed);
                }
                Err(e) => {
                    errors += 1;
                    if errors > 400 {
                        // ~2s of nothing but errors: the stream is gone
                        eprintln!("giving up on stream: {e}");
                        drained = true;
                        sh.eof.store(true, Relaxed);
                    }
                    thread::sleep(Duration::from_millis(5));
                }
            }
        }
        Ok(())
    }

    fn drain_video(&mut self, sh: &Shared, skip_until: f64) -> Result<()> {
        let mut f = frame::Video::empty();
        while self.vdec.receive_frame(&mut f).is_ok() {
            let t = secs(f.timestamp().unwrap_or(0), self.vtb);
            if t < skip_until - 0.001 {
                continue; // landed on a keyframe before the seek target
            }
            if self.scaler.is_none() {
                let (dw, dh) = fit(f.width(), f.height(), MAX_VIDEO_WIDTH);
                self.scaler = Some(scaling::Context::get(
                    f.format(),
                    f.width(),
                    f.height(),
                    Pixel::RGBA,
                    dw,
                    dh,
                    scaling::Flags::BILINEAR,
                )?);
            }
            let mut rgb = frame::Video::empty();
            self.scaler.as_mut().unwrap().run(&f, &mut rgb)?;
            let (w, h) = (rgb.width() as usize, rgb.height() as usize);
            let stride = rgb.stride(0);
            let data = rgb.data(0);
            let mut px = Vec::with_capacity(w * h * 4);
            for y in 0..h {
                px.extend_from_slice(&data[y * stride..y * stride + w * 4]);
            }
            if self.need_base {
                sh.set_base(t);
                self.need_base = false;
            }
            sh.vq
                .lock()
                .unwrap()
                .push_back(VFrame { t, w, h, rgba: px });
        }
        Ok(())
    }

    fn drain_audio(&mut self, audio: &AudioShared, skip_until: f64) -> Result<()> {
        let Some(a) = self.audio.as_mut() else {
            return Ok(());
        };
        let mut f = frame::Audio::empty();
        while a.dec.receive_frame(&mut f).is_ok() {
            if secs(f.timestamp().unwrap_or(0), a.tb) < skip_until - 0.02 {
                continue;
            }
            if f.channel_layout().is_empty() {
                f.set_channel_layout(ChannelLayout::default(f.channels() as i32));
            }
            if a.rs.is_none() {
                a.rs = Some(resampling::Context::get(
                    f.format(),
                    f.channel_layout(),
                    f.rate(),
                    Sample::F32(SampleType::Packed),
                    ChannelLayout::STEREO,
                    audio.rate,
                )?);
            }
            let mut out = frame::Audio::empty();
            a.rs.as_mut().unwrap().run(&f, &mut out)?;
            let bytes = out.data(0);
            let n = (out.samples() * 2 * 4).min(bytes.len());
            let samples: Vec<f32> = bytes[..n]
                .as_chunks::<4>()
                .0
                .iter()
                .map(|c| f32::from_ne_bytes(*c))
                .collect();
            audio.push(&samples);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A clip with a second of video before its first audio (a fragmented
    /// MP4), like some downloads: playback must still start.
    #[test]
    fn starts_when_audio_comes_late() {
        let dir = std::env::temp_dir().join(format!("valo-late-audio-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let clip = dir.join("late.mp4");
        let made = std::process::Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "testsrc=size=320x240:rate=24",
            ])
            .args([
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:sample_rate=48000",
                "-t",
                "4",
            ])
            .args(["-c:v", "libx264", "-bf", "3", "-c:a", "aac"])
            .args([
                "-movflags",
                "+frag_keyframe+empty_moov",
                "-frag_duration",
                "1000000",
            ])
            .arg(&clip)
            .status();
        if !made.is_ok_and(|s| s.success()) {
            eprintln!("no ffmpeg with libx264 here; skipping");
            return;
        }
        let audio = Arc::new(AudioShared::new(true, 48000));
        let p = Player::open(&Source::File(clip), audio.clone()).unwrap();
        assert!(p.meta.has_audio);
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while audio.buffered_secs() < 0.25 && std::time::Instant::now() < deadline {
            thread::sleep(Duration::from_millis(20));
        }
        assert!(
            audio.buffered_secs() >= 0.25,
            "the decoder stalled before reaching the audio"
        );
        drop(p);
        let _ = std::fs::remove_dir_all(dir);
    }
}
