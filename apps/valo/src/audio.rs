//! Audio output: a ring buffer drained by a cpal stream. The audio clock
//! (frames actually heard) is the master clock for video sync.
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering::Relaxed};
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, bail, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;

pub struct AudioShared {
    pub available: bool,
    pub rate: u32,
    buf: Mutex<VecDeque<f32>>, // interleaved stereo
    consumed: AtomicU64,       // stereo frames played since last reset
    /// Frames handed to the device but not heard yet: its buffer, plus
    /// Bluetooth and the like (~0.3 s on a Bluetooth headset).
    latency: AtomicU64,
    paused: AtomicBool,
    muted: AtomicBool,
    volume: AtomicU32,
}

impl AudioShared {
    pub(crate) fn new(available: bool, rate: u32) -> Self {
        Self {
            available,
            rate,
            buf: Mutex::new(VecDeque::new()),
            consumed: AtomicU64::new(0),
            latency: AtomicU64::new(0),
            paused: AtomicBool::new(false),
            muted: AtomicBool::new(false),
            volume: AtomicU32::new(1.0f32.to_bits()),
        }
    }

    pub fn push(&self, samples: &[f32]) {
        self.buf.lock().unwrap().extend(samples.iter().copied());
    }

    pub fn buffered_secs(&self) -> f64 {
        self.buf.lock().unwrap().len() as f64 / 2.0 / self.rate as f64
    }

    /// Seconds heard since the last reset; picture and subtitles follow this.
    pub fn played_secs(&self) -> f64 {
        let heard = self
            .consumed
            .load(Relaxed)
            .saturating_sub(self.latency.load(Relaxed));
        heard as f64 / self.rate as f64
    }

    pub fn reset(&self) {
        let mut b = self.buf.lock().unwrap();
        b.clear();
        self.consumed.store(0, Relaxed);
    }

    pub fn set_paused(&self, p: bool) {
        self.paused.store(p, Relaxed);
    }
    pub fn set_muted(&self, m: bool) {
        self.muted.store(m, Relaxed);
    }
    pub fn set_volume(&self, v: f32) {
        self.volume.store(v.clamp(0.0, 1.0).to_bits(), Relaxed);
    }

    fn fill<T: cpal::SizedSample + cpal::FromSample<f32>>(
        &self,
        out: &mut [T],
        ch: usize,
        info: &cpal::OutputCallbackInfo,
    ) {
        let mut buf = self.buf.lock().unwrap();
        let play = !self.paused.load(Relaxed);
        let vol = if self.muted.load(Relaxed) {
            0.0
        } else {
            f32::from_bits(self.volume.load(Relaxed))
        };
        let mut frames = 0u64;
        for frame in out.chunks_mut(ch) {
            if play && buf.len() >= 2 {
                let l = buf.pop_front().unwrap() * vol;
                let r = buf.pop_front().unwrap() * vol;
                for (i, s) in frame.iter_mut().enumerate() {
                    *s = match i {
                        0 => T::from_sample(l),
                        1 => T::from_sample(r),
                        _ => T::EQUILIBRIUM,
                    };
                }
                frames += 1;
            } else {
                frame.iter_mut().for_each(|s| *s = T::EQUILIBRIUM);
            }
        }
        self.consumed.fetch_add(frames, Relaxed);
        // the first frame of `out` is heard at `playback`; the rest after it
        let ts = info.timestamp();
        if let Some(d) = ts.playback.checked_duration_since(ts.callback) {
            let ahead = (d.as_secs_f64() * self.rate as f64) as u64;
            self.latency.store(ahead + (out.len() / ch) as u64, Relaxed);
        }
    }
}

pub struct AudioOut {
    pub shared: Arc<AudioShared>,
    _stream: Option<cpal::Stream>,
}

impl AudioOut {
    pub fn new() -> Self {
        match Self::try_new() {
            Ok(a) => a,
            Err(e) => {
                eprintln!("audio disabled: {e:#}");
                Self {
                    shared: Arc::new(AudioShared::new(false, 48_000)),
                    _stream: None,
                }
            }
        }
    }

    fn try_new() -> Result<Self> {
        let dev = cpal::default_host()
            .default_output_device()
            .ok_or_else(|| anyhow!("no audio output device"))?;
        let cfg = dev.default_output_config()?;
        let fmt = cfg.sample_format();
        let sc: cpal::StreamConfig = cfg.into();
        let (rate, ch) = (sc.sample_rate, sc.channels as usize);
        let shared = Arc::new(AudioShared::new(true, rate));
        let stream = match fmt {
            SampleFormat::F32 => build::<f32>(&dev, sc, shared.clone(), ch)?,
            SampleFormat::I16 => build::<i16>(&dev, sc, shared.clone(), ch)?,
            SampleFormat::U16 => build::<u16>(&dev, sc, shared.clone(), ch)?,
            other => bail!("unsupported sample format {other:?}"),
        };
        stream.play()?;
        Ok(Self {
            shared,
            _stream: Some(stream),
        })
    }
}

fn build<T>(
    dev: &cpal::Device,
    cfg: cpal::StreamConfig,
    sh: Arc<AudioShared>,
    ch: usize,
) -> Result<cpal::Stream>
where
    T: cpal::SizedSample + cpal::FromSample<f32>,
{
    Ok(dev.build_output_stream(
        cfg,
        move |out: &mut [T], info: &cpal::OutputCallbackInfo| sh.fill(out, ch, info),
        |e| eprintln!("audio error: {e}"),
        None,
    )?)
}
