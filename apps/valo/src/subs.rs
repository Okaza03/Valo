//! Subtitle files (SRT, WebVTT, ASS/SSA): fetching them and finding the
//! line to show at a given time.
use std::io::Read;
use std::time::Duration;

use anyhow::{anyhow, Result};

use crate::addons::lang_code;
use crate::api::SubtitleTrack;

/// One line (or a few) on screen from `start` to `end`, in seconds.
#[derive(Clone, Debug, PartialEq)]
pub struct Cue {
    pub start: f64,
    pub end: f64,
    pub text: String,
}

/// A parsed file, cues in start order.
pub struct Subs {
    cues: Vec<Cue>,
}

impl Subs {
    pub fn parse(text: &str, format: &str) -> Self {
        let text = text
            .trim_start_matches('\u{feff}')
            .replace("\r\n", "\n")
            .replace('\r', "\n");
        let mut cues = match format {
            "ass" | "ssa" => parse_ass(&text),
            _ => parse_srt_vtt(&text),
        };
        cues.retain(|c| c.end > c.start && !c.text.trim().is_empty());
        cues.sort_by(|a, b| a.start.total_cmp(&b.start));
        Self { cues }
    }

    pub fn is_empty(&self) -> bool {
        self.cues.is_empty()
    }

    /// The text to show at `t`; overlapping cues are stacked, earliest on top.
    pub fn at(&self, t: f64) -> Option<String> {
        // cues starting after t can't be on screen
        let n = self.cues.partition_point(|c| c.start <= t);
        let lines: Vec<&str> = self.cues[..n]
            .iter()
            .rev()
            .take(32) // a long cue started well before t is rare; don't scan the whole file
            .filter(|c| t < c.end)
            .map(|c| c.text.as_str())
            .collect();
        (!lines.is_empty()).then(|| lines.into_iter().rev().collect::<Vec<_>>().join("\n"))
    }
}

/// "srt", "vtt", "ass", "ssa": as the server says, else from the URL.
pub fn format_of(track: &SubtitleTrack) -> String {
    let f = track.format.clone().unwrap_or_else(|| {
        let path = track.url.split(['?', '#']).next().unwrap_or_default();
        let name = path.rsplit('/').next().unwrap_or_default();
        name.rsplit_once('.')
            .map(|(_, e)| e.to_owned())
            .unwrap_or_default()
    });
    f.trim().to_lowercase()
}

/// Download and parse a subtitle file. `auth` goes along as the
/// Authorization header (the stream's, so only to our own server).
pub fn load(track: &SubtitleTrack, auth: Option<&str>) -> Result<Subs> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(5))
        .timeout_read(Duration::from_secs(30))
        .user_agent(concat!("Valo/", env!("CARGO_PKG_VERSION")))
        .build();
    let mut req = agent.get(&track.url);
    if let Some(a) = auth {
        req = req.set("Authorization", a);
    }
    let resp = req.call().map_err(|e| match e {
        ureq::Error::Status(code, _) => anyhow!("the server answered {code}"),
        e => anyhow!("{e}"),
    })?;
    let mut buf = Vec::new();
    resp.into_reader().take(16 << 20).read_to_end(&mut buf)?;
    let subs = Subs::parse(&decode(&buf), &format_of(track));
    if subs.is_empty() {
        return Err(anyhow!("the file has no lines in it"));
    }
    Ok(subs)
}

/// UTF-8 (with or without BOM), UTF-16 with BOM, else Windows-1252.
fn decode(b: &[u8]) -> String {
    if let Some(rest) = b.strip_prefix(&[0xFF, 0xFE]) {
        return utf16(rest, u16::from_le_bytes);
    }
    if let Some(rest) = b.strip_prefix(&[0xFE, 0xFF]) {
        return utf16(rest, u16::from_be_bytes);
    }
    match std::str::from_utf8(b) {
        Ok(s) => s.to_owned(),
        Err(_) => b.iter().map(|&c| cp1252(c)).collect(),
    }
}

fn utf16(b: &[u8], f: fn([u8; 2]) -> u16) -> String {
    let units: Vec<u16> = b.as_chunks::<2>().0.iter().map(|c| f(*c)).collect();
    String::from_utf16_lossy(&units)
}

fn cp1252(c: u8) -> char {
    const HIGH: [char; 32] = [
        '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8d}', 'Ž',
        '\u{8f}', '\u{90}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9d}',
        'ž', 'Ÿ',
    ];
    match c {
        0x80..=0x9F => HIGH[(c - 0x80) as usize],
        _ => c as char,
    }
}

/// "00:01:02,500", "01:02.500", "1:02:03.5" -> seconds.
fn timestamp(s: &str) -> Option<f64> {
    let s = s.trim().replace(',', ".");
    let parts: Vec<&str> = s.split(':').collect();
    let (h, m, sec) = match parts[..] {
        [h, m, s] => (h.parse::<f64>().ok()?, m.parse::<f64>().ok()?, s),
        [m, s] => (0.0, m.parse::<f64>().ok()?, s),
        _ => return None,
    };
    Some(h * 3600.0 + m * 60.0 + sec.parse::<f64>().ok()?)
}

/// SRT and WebVTT: blocks with a "start --> end" line, the text under it.
fn parse_srt_vtt(text: &str) -> Vec<Cue> {
    let mut cues = Vec::new();
    for block in text.split("\n\n") {
        let mut lines = block.lines().skip_while(|l| !l.contains("-->"));
        let Some(times) = lines.next() else { continue };
        let Some((a, b)) = times.split_once("-->") else {
            continue;
        };
        // WebVTT puts cue settings after the end time ("00:01.000 align:start")
        let b = b.split_whitespace().next().unwrap_or_default();
        let (Some(start), Some(end)) = (timestamp(a), timestamp(b)) else {
            continue;
        };
        let body: Vec<String> = lines
            .map(strip_tags)
            .map(|l| l.trim().to_owned())
            .filter(|l| !l.is_empty())
            .collect();
        cues.push(Cue {
            start,
            end,
            text: body.join("\n"),
        });
    }
    cues
}

/// `<i>`, `<font color=…>`, `<c.yellow>`, `{\an8}` and the like go; the text stays.
fn strip_tags(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        let close = match c {
            '<' => '>',
            '{' if chars.peek() == Some(&'\\') => '}',
            _ => {
                out.push(c);
                continue;
            }
        };
        // only drop it if it's closed, so a lone "<" stays
        let rest: String = chars.clone().collect();
        match rest.find(close) {
            Some(i) => {
                for _ in 0..rest[..=i].chars().count() {
                    chars.next();
                }
            }
            None => out.push(c),
        }
    }
    out.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
}

/// ASS/SSA: "Dialogue:" lines under [Events], laid out by its Format line.
fn parse_ass(text: &str) -> Vec<Cue> {
    let mut fields: Vec<String> = Vec::new();
    let mut cues = Vec::new();
    let mut in_events = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_events = line.eq_ignore_ascii_case("[events]");
            continue;
        }
        if !in_events {
            continue;
        }
        if let Some(f) = line.strip_prefix("Format:") {
            fields = f.split(',').map(|x| x.trim().to_lowercase()).collect();
            continue;
        }
        let Some(d) = line.strip_prefix("Dialogue:") else {
            continue;
        };
        if fields.is_empty() {
            fields = [
                "layer", "start", "end", "style", "name", "marginl", "marginr", "marginv",
                "effect", "text",
            ]
            .map(String::from)
            .to_vec();
        }
        // the text is last and may hold commas
        let parts: Vec<&str> = d.splitn(fields.len(), ',').collect();
        let get = |name: &str| {
            fields
                .iter()
                .position(|f| f == name)
                .and_then(|i| parts.get(i))
        };
        let (Some(a), Some(b), Some(t)) = (get("start"), get("end"), get("text")) else {
            continue;
        };
        let (Some(start), Some(end)) = (timestamp(a), timestamp(b)) else {
            continue;
        };
        let t = strip_tags(t)
            .replace("\\N", "\n")
            .replace("\\n", "\n")
            .replace("\\h", " ");
        let t: Vec<&str> = t.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
        cues.push(Cue {
            start,
            end,
            text: t.join("\n"),
        });
    }
    cues
}

/// Which track to turn on: one in the language picked last time, the SDH
/// kind if that's what was picked, and not one that's only forced parts.
pub fn pick(tracks: &[SubtitleTrack], language: Option<&str>, sdh: bool) -> Option<usize> {
    let want = lang_code(language?);
    let score = |t: &SubtitleTrack| (!t.forced as u8) * 2 + (t.sdh == sdh) as u8;
    tracks
        .iter()
        .enumerate()
        .filter(|(_, t)| lang_code(&t.language) == want)
        // the server's own files before an addon's, then the best fit
        .max_by_key(|(i, t)| (!t.external, score(t), std::cmp::Reverse(*i)))
        .map(|(i, _)| i)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn srt() {
        let s = Subs::parse(
            "\u{feff}1\r\n00:00:02,000 --> 00:00:07,000\r\n<i>Downloaded</i> from\r\nYTS\r\n\r\n2\r\n00:01:00,500 --> 00:01:02,000\r\n{\\an8}Up top\r\n",
            "srt",
        );
        assert_eq!(s.at(1.0), None);
        assert_eq!(s.at(3.0).as_deref(), Some("Downloaded from\nYTS"));
        assert_eq!(s.at(61.0).as_deref(), Some("Up top"));
        assert_eq!(s.at(62.5), None);
    }

    #[test]
    fn vtt() {
        let s = Subs::parse(
            "WEBVTT\n\nNOTE hi\n\nintro\n00:01.000 --> 00:02.500 align:start\n<c.yellow>Hello</c> &amp; bye\n",
            "vtt",
        );
        assert_eq!(s.at(2.0).as_deref(), Some("Hello & bye"));
    }

    #[test]
    fn ass() {
        let s = Subs::parse(
            "[Script Info]\nTitle: x\n\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\nDialogue: 0,0:00:01.00,0:00:03.00,Default,,0,0,0,,{\\i1}Hi,{\\i0} there\\Nyou\n",
            "ass",
        );
        assert_eq!(s.at(2.0).as_deref(), Some("Hi, there\nyou"));
    }

    #[test]
    fn overlapping() {
        let s = Subs::parse(
            "1\n00:00:01,000 --> 00:00:05,000\nA\n\n2\n00:00:02,000 --> 00:00:03,000\nB\n",
            "srt",
        );
        assert_eq!(s.at(2.5).as_deref(), Some("A\nB"));
        assert_eq!(s.at(4.0).as_deref(), Some("A"));
    }

    #[test]
    fn latin1() {
        assert_eq!(decode(b"Caf\xe9 \x93hi\x94"), "Café “hi”");
    }

    #[test]
    fn picking() {
        let t = |language: &str, forced, sdh| SubtitleTrack {
            language: language.into(),
            forced,
            sdh,
            url: "x".into(),
            ..Default::default()
        };
        let tracks = [
            t("en", true, false),
            t("en", false, true),
            t("en", false, false),
            t("nb", false, false),
        ];
        assert_eq!(pick(&tracks, None, false), None);
        assert_eq!(pick(&tracks, Some("en"), false), Some(2));
        assert_eq!(pick(&tracks, Some("en"), true), Some(1));
        assert_eq!(pick(&tracks, Some("nb"), true), Some(3));
        assert_eq!(pick(&tracks, Some("de"), false), None);

        // an addon's track comes after the server's file, whatever the order
        let addon = SubtitleTrack {
            external: true,
            ..t("eng", false, false)
        };
        let tracks = [addon, t("English", false, true)];
        assert_eq!(pick(&tracks, Some("en"), false), Some(1));
    }
}
