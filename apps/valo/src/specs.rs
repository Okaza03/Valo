//! What's in the playing file, for the player's Info panel: the container,
//! the video and audio being played, and the chapters. All of it comes from
//! the headers FFmpeg has already read when the file opens.
use std::ffi::{c_char, CStr};

use ffmpeg::format::context::Input;
use ffmpeg::media::Type;
use ffmpeg::{codec, sys, Rational};
use ffmpeg_next as ffmpeg;

/// A label and its value.
pub type Row = (&'static str, String);

#[derive(Clone, Default)]
pub struct Specs {
    /// The name FFmpeg opened: a stream URL's last part for a server's videos.
    pub name: String,
    /// The title written in the file, if any.
    pub title: Option<String>,
    /// Size, format and bitrate.
    pub file: Vec<Row>,
    pub video: Vec<Row>,
    pub audio: Vec<Row>,
    /// Start (seconds) and title of each chapter.
    pub chapters: Vec<(f64, Option<String>)>,
}

impl Specs {
    /// The panel's sections, with the chapter playing at `t`. `name`: the
    /// file's real name when the server sent it; `title`: the library's, for
    /// files without one of their own.
    pub fn sections(
        &self,
        t: f64,
        name: Option<&str>,
        title: &str,
    ) -> Vec<(&'static str, Vec<Row>)> {
        let mut file = vec![
            ("Name", name.unwrap_or(&self.name).to_owned()),
            (
                "Title",
                self.title.clone().unwrap_or_else(|| title.to_owned()),
            ),
            ("Chapter", self.chapter_at(t)),
        ];
        file.extend(self.file.iter().cloned());
        let mut out = vec![("FILE", file)];
        if !self.video.is_empty() {
            out.push(("VIDEO", self.video.clone()));
        }
        if !self.audio.is_empty() {
            out.push(("AUDIO", self.audio.clone()));
        }
        out
    }

    /// "3 of 12 · The Heist" for the chapter playing at `t`.
    pub fn chapter_at(&self, t: f64) -> String {
        let n = self.chapters.len();
        if n == 0 {
            return "No chapters".into();
        }
        let i = self
            .chapters
            .iter()
            .rposition(|(start, _)| *start <= t + 0.05)
            .unwrap_or(0);
        match &self.chapters[i].1 {
            Some(title) => format!("{} of {n} · {title}", i + 1),
            None => format!("{} of {n}", i + 1),
        }
    }
}

fn cstr(p: *const c_char) -> Option<String> {
    // SAFETY: FFmpeg's name lookups return static NUL-terminated strings or null
    (!p.is_null()).then(|| unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned())
}

/// "mkv" for the file: the extension when the demuxer goes by it, else the
/// demuxer's first name ("mov,mp4,m4a,…" lists all of them).
pub fn container(ictx: &Input, name: &str) -> String {
    let names = ictx.format().name().to_owned();
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_lowercase());
    match ext {
        Some(e) if names.split(',').any(|n| n == e) => e,
        _ if names.starts_with("matroska") => "mkv".into(),
        _ if names.starts_with("mov,mp4") => "mp4".into(),
        _ => names.split(',').next().unwrap_or(&names).to_owned(),
    }
}

/// The file's size in bytes, when the protocol knows it.
pub fn size(ictx: &Input) -> Option<u64> {
    // SAFETY: pb is the open input's I/O context (null for some protocols)
    let size = unsafe {
        let pb = (*ictx.as_ptr()).pb;
        if pb.is_null() {
            -1
        } else {
            sys::avio_size(pb)
        }
    };
    u64::try_from(size).ok().filter(|s| *s > 0)
}

/// 2090000000 -> "2.09 GB"
fn bytes(n: u64) -> String {
    let f = n as f64;
    match f {
        f if f >= 1e9 => format!("{:.2} GB", f / 1e9),
        f if f >= 1e6 => format!("{:.0} MB", f / 1e6),
        f => format!("{:.0} kB", f / 1e3),
    }
}

/// 2600000 -> "2.6 Mb/s", 640000 -> "640 kb/s"
fn bitrate(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1} Mb/s", n as f64 / 1e6)
    } else {
        format!("{} kb/s", n / 1000)
    }
}

/// 23.976 -> "23.976 fps", 25 -> "25 fps"
fn fps(r: Rational) -> Option<String> {
    (r.numerator() > 0 && r.denominator() > 0).then(|| {
        let f = f64::from(r.numerator()) / f64::from(r.denominator());
        let s = format!("{f:.3}");
        let s = s.trim_end_matches('0').trim_end_matches('.');
        format!("{s} fps")
    })
}

/// A codec level as people write it: H.264 51 and HEVC 153 are both "5.1".
fn level(id: codec::Id, l: i32) -> Option<String> {
    if l <= 0 {
        return None;
    }
    Some(match id {
        codec::Id::H264 if l == 9 => "1b".into(),
        codec::Id::H264 => format!("{}.{}", l / 10, l % 10),
        codec::Id::HEVC => format!("{}.{}", l / 30, (l % 30) / 3),
        codec::Id::AV1 => format!("{}.{}", 2 + (l >> 2), l & 3),
        _ => l.to_string(),
    })
}

/// "default, forced"
fn flags(d: i32) -> String {
    const NAMES: &[(i32, &str)] = &[
        (sys::AV_DISPOSITION_DEFAULT, "default"),
        (sys::AV_DISPOSITION_FORCED, "forced"),
        (sys::AV_DISPOSITION_ORIGINAL, "original"),
        (sys::AV_DISPOSITION_DUB, "dub"),
        (sys::AV_DISPOSITION_COMMENT, "commentary"),
        (sys::AV_DISPOSITION_HEARING_IMPAIRED, "hearing impaired"),
        (sys::AV_DISPOSITION_VISUAL_IMPAIRED, "visual impaired"),
        (sys::AV_DISPOSITION_DESCRIPTIONS, "descriptions"),
        (sys::AV_DISPOSITION_ATTACHED_PIC, "cover art"),
    ];
    let on: Vec<&str> = NAMES
        .iter()
        .filter(|(bit, _)| d & bit != 0)
        .map(|(_, n)| *n)
        .collect();
    if on.is_empty() {
        "none".into()
    } else {
        on.join(", ")
    }
}

/// "English", or "Unknown" when the file doesn't say.
fn language(s: &ffmpeg::Stream) -> String {
    match s.metadata().get("language") {
        Some(l) if !l.is_empty() && l != "und" => crate::addons::lang_name(l),
        _ => "Unknown".into(),
    }
}

/// "1 of 2 · "Commentary" (stream 3)"
fn track_id(ictx: &Input, s: &ffmpeg::Stream, kind: Type) -> String {
    let same: Vec<usize> = ictx
        .streams()
        .filter(|o| o.parameters().medium() == kind)
        .map(|o| o.index())
        .collect();
    let n = same
        .iter()
        .position(|i| *i == s.index())
        .map_or(1, |p| p + 1);
    let mut out = format!("{n} of {}", same.len());
    if let Some(t) = s.metadata().get("title").filter(|t| !t.is_empty()) {
        out.push_str(&format!(" · \u{201c}{t}\u{201d}"));
    }
    out.push_str(&format!(" (stream {})", s.index()));
    out
}

/// "H.265 / HEVC (High Efficiency Video Coding) · Main 10 · Level 5.1"
fn codec_format(par: &codec::Parameters) -> String {
    let id = par.id();
    // SAFETY: par wraps the stream's live AVCodecParameters
    let (raw_id, profile, lvl) = unsafe {
        let p = &*par.as_ptr();
        (p.codec_id, p.profile, p.level)
    };
    // SAFETY: lookups by id; the descriptor is static
    let long = unsafe {
        let d = sys::avcodec_descriptor_get(raw_id);
        if d.is_null() {
            None
        } else {
            cstr((*d).long_name)
        }
    };
    // FFmpeg's names for these are the standards' ("ATSC A/52B (AC-3, E-AC-3)")
    let known = match id {
        codec::Id::AC3 => Some("Dolby Digital (AC-3)"),
        codec::Id::EAC3 => Some("Dolby Digital Plus (E-AC-3)"),
        codec::Id::TRUEHD => Some("Dolby TrueHD"),
        _ => None,
    };
    let name = known
        .map(str::to_owned)
        .or(long)
        .unwrap_or_else(|| id.name().to_owned());
    let mut parts = vec![name];
    // SAFETY: as above
    if let Some(p) = cstr(unsafe { sys::avcodec_profile_name(raw_id, profile) }) {
        parts.push(p);
    }
    if let Some(l) = level(id, lvl) {
        parts.push(format!("Level {l}"));
    }
    parts.join(" · ")
}

/// The stream's bitrate: from its header, or the tag MKVToolNix writes.
fn stream_bitrate(s: &ffmpeg::Stream) -> Option<u64> {
    // SAFETY: as above
    let b = unsafe { (*s.parameters().as_ptr()).bit_rate };
    u64::try_from(b).ok().filter(|b| *b > 0).or_else(|| {
        let m = s.metadata();
        m.get("BPS").or(m.get("BPS-eng"))?.parse().ok()
    })
}

/// "Profile 8.1 · Level 6", or "No".
fn dolby_vision(par: &codec::Parameters) -> String {
    // SAFETY: coded_side_data holds nb_coded_side_data entries, each with
    // `size` bytes; the DOVI record starts with eight u8 fields: version
    // major and minor, profile, level, rpu/el/bl present, compatibility id
    unsafe {
        let p = &*par.as_ptr();
        for i in 0..usize::try_from(p.nb_coded_side_data).unwrap_or(0) {
            let sd = &*p.coded_side_data.add(i);
            if sd.type_ != sys::AVPacketSideDataType::AV_PKT_DATA_DOVI_CONF || sd.size < 8 {
                continue;
            }
            let d = std::slice::from_raw_parts(sd.data, 8);
            let (profile, lvl, el, compat) = (d[2], d[3], d[5], d[7]);
            let mut s = if compat > 0 {
                format!("Profile {profile}.{compat}")
            } else {
                format!("Profile {profile}")
            };
            s.push_str(&format!(" · Level {lvl}"));
            if el != 0 {
                s.push_str(" · with enhancement layer");
            }
            return s;
        }
    }
    "No".into()
}

/// The file's facts, with the video and audio streams the player decodes.
pub fn read(ictx: &Input, name: &str) -> Specs {
    // ---- file
    let mut s = Specs {
        name: name.to_owned(),
        title: ictx
            .metadata()
            .get("title")
            .filter(|t| !t.trim().is_empty())
            .map(str::to_owned),
        ..Default::default()
    };
    if let Some(n) = size(ictx) {
        s.file.push(("Size", bytes(n)));
    }
    let desc = ictx.format().description().to_owned();
    s.file
        .push(("Format", format!("{desc} ({})", container(ictx, name))));
    if let Some(b) = u64::try_from(ictx.bit_rate()).ok().filter(|b| *b > 0) {
        s.file.push(("Bitrate", bitrate(b)));
    }
    s.chapters = ictx
        .chapters()
        .map(|c| {
            let tb = c.time_base();
            let start =
                c.start() as f64 * f64::from(tb.numerator()) / f64::from(tb.denominator().max(1));
            let t = c.metadata().get("title").map(str::to_owned);
            (start, t.filter(|t| !t.trim().is_empty()))
        })
        .collect();

    // ---- video
    if let Some(v) = ictx.streams().best(Type::Video) {
        let par = v.parameters();
        s.video.push(("ID", track_id(ictx, &v, Type::Video)));
        s.video.push(("Flags", flags(v.disposition().bits())));
        let mut format = codec_format(&par);
        let pix = codec::context::Context::from_parameters(par.clone())
            .ok()
            .and_then(|c| c.decoder().video().ok())
            .map(|d| d.format())
            .filter(|p| *p != ffmpeg::format::Pixel::None);
        if let Some(pix) = pix {
            // SAFETY: a valid pixel format's descriptor is static
            let (name, depth) = unsafe {
                let raw: sys::AVPixelFormat = pix.into();
                let d = sys::av_pix_fmt_desc_get(raw);
                let depth = if d.is_null() { 0 } else { (*d).comp[0].depth };
                (cstr(sys::av_get_pix_fmt_name(raw)), depth)
            };
            if let Some(n) = name {
                format.push_str(&format!(" · {n}"));
            }
            if depth > 0 {
                format.push_str(&format!(" · {depth}-bit"));
            }
        }
        // SAFETY: as above
        let (w, h, trc, sar) = unsafe {
            let p = &*par.as_ptr();
            (p.width, p.height, p.color_trc, p.sample_aspect_ratio)
        };
        use sys::AVColorTransferCharacteristic as Trc;
        match trc {
            Trc::AVCOL_TRC_SMPTE2084 => format.push_str(" · HDR (PQ)"),
            Trc::AVCOL_TRC_ARIB_STD_B67 => format.push_str(" · HDR (HLG)"),
            _ => {}
        }
        s.video.push(("Format", format));
        s.video.push(("Language", language(&v)));
        if w > 0 && h > 0 {
            s.video.push(("Resolution", format!("{w} × {h}")));
        }
        let rate = Some(v.avg_frame_rate())
            .filter(|r| r.numerator() > 0)
            .unwrap_or_else(|| v.rate());
        if let Some(f) = fps(rate) {
            s.video.push(("Frame rate", f));
        }
        let sar = if sar.num > 0 && sar.den > 0 {
            sar
        } else {
            // SAFETY: as above
            unsafe { (*v.as_ptr()).sample_aspect_ratio }
        };
        let par_text = if sar.num > 0 && sar.den > 0 {
            format!("{}:{}", sar.num, sar.den)
        } else {
            "1:1".into()
        };
        s.video.push(("Pixel aspect ratio", par_text));
        s.video.push(("Dolby Vision", dolby_vision(&par)));
        if let Some(b) = stream_bitrate(&v) {
            s.video.push(("Bitrate", bitrate(b)));
        }
    }

    // ---- audio
    if let Some(a) = ictx.streams().best(Type::Audio) {
        let par = a.parameters();
        s.audio.push(("ID", track_id(ictx, &a, Type::Audio)));
        s.audio.push(("Flags", flags(a.disposition().bits())));
        let mut format = codec_format(&par);
        // SAFETY: as above; the layout is the stream's own
        let (channels, layout, rate, bits) = unsafe {
            let p = &*par.as_ptr();
            let mut buf = [0 as c_char; 64];
            let n = sys::av_channel_layout_describe(&p.ch_layout, buf.as_mut_ptr(), buf.len());
            let layout = (n > 0).then(|| cstr(buf.as_ptr())).flatten();
            (
                p.ch_layout.nb_channels,
                layout,
                p.sample_rate,
                p.bits_per_raw_sample,
            )
        };
        if bits > 0 {
            format.push_str(&format!(" · {bits}-bit"));
        }
        s.audio.push(("Format", format));
        s.audio.push(("Language", language(&a)));
        if channels > 0 {
            s.audio.push(("Channels", channels.to_string()));
        }
        if let Some(l) = layout {
            s.audio.push(("Channel layout", l));
        }
        if rate > 0 {
            let khz = f64::from(rate) / 1000.0;
            let t = format!("{khz:.1}");
            s.audio
                .push(("Sample rate", format!("{} kHz", t.trim_end_matches(".0"))));
        }
        if let Some(b) = stream_bitrate(&a) {
            s.audio.push(("Bitrate", bitrate(b)));
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_read_like_people_write_them() {
        assert_eq!(level(codec::Id::H264, 51).as_deref(), Some("5.1"));
        assert_eq!(level(codec::Id::HEVC, 153).as_deref(), Some("5.1"));
        assert_eq!(level(codec::Id::HEVC, 120).as_deref(), Some("4.0"));
        assert_eq!(level(codec::Id::AV1, 13).as_deref(), Some("5.1"));
        assert_eq!(level(codec::Id::H264, -99), None);
        assert_eq!(
            fps(Rational::new(24000, 1001)).as_deref(),
            Some("23.976 fps")
        );
        assert_eq!(fps(Rational::new(25, 1)).as_deref(), Some("25 fps"));
        assert_eq!(bitrate(640_000), "640 kb/s");
        assert_eq!(
            flags(sys::AV_DISPOSITION_DEFAULT | sys::AV_DISPOSITION_FORCED),
            "default, forced"
        );
        assert_eq!(flags(0), "none");
    }

    #[test]
    fn chapter_at_time() {
        let s = Specs {
            chapters: vec![(0.0, Some("Intro".into())), (300.0, None)],
            ..Default::default()
        };
        assert_eq!(s.chapter_at(10.0), "1 of 2 · Intro");
        assert_eq!(s.chapter_at(301.0), "2 of 2");
        assert_eq!(Specs::default().chapter_at(5.0), "No chapters");
    }
}
