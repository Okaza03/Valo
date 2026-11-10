//! The editor's other tabs (files, subtitles, episodes) and its extra
//! actions (TMDB matching, deleting a title), each on its own endpoint.
use crate::api::Episode;
use crate::api::Item;
use crate::api::Kind;

/// The video file a subtitle search is for: the movie's, or for a series the
/// episode typed in ("S01E02"), else its first episode with a file.
pub(crate) fn video_name(item: &Item, episode: &str) -> String {
    let file = |f: &Option<crate::api::FileInfo>| {
        f.as_ref()
            .map(|f| {
                let name = f.name.clone().unwrap_or_default();
                format!("{name} {}", f.quality.clone().unwrap_or_default())
            })
            .filter(|n| !n.trim().is_empty())
    };
    if item.kind != Kind::Series {
        return file(&item.file).unwrap_or_default();
    }
    let code = episode.trim().to_lowercase();
    let eps = || {
        item.seasons
            .iter()
            .flat_map(|s| s.episodes.iter().map(move |e| (s.number, e)))
    };
    eps()
        .find(|(s, e)| format!("s{s:02}e{:02}", e.number) == code)
        .and_then(|(_, e)| file(&e.file))
        .or_else(|| eps().find_map(|(_, e)| file(&e.file)))
        .unwrap_or_default()
}

/// An episode's code as the subtitle endpoints take it: "S01E02".
pub(crate) fn episode_code(season: u32, episode: u32) -> String {
    format!("S{season:02}E{episode:02}")
}

/// The season and episode of a code like "S01E02" (any case).
pub(crate) fn parse_code(code: &str) -> Option<(u32, u32)> {
    let code = code.trim().to_lowercase();
    let (s, e) = code.strip_prefix('s')?.split_once('e')?;
    Some((s.parse().ok()?, e.parse().ok()?))
}

fn words(s: &str) -> Vec<String> {
    s.to_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_owned)
        .collect()
}

/// "bluray", "web", … for a release name: subtitles from the same kind of
/// source share the cut and the frame rate, others are often seconds off.
fn source_of(words: &[String]) -> Option<&'static str> {
    words.iter().find_map(|w| match w.as_str() {
        "bluray" | "bdrip" | "brrip" | "bdremux" | "remux" | "uhd" | "bd" => Some("bluray"),
        "web" | "webdl" | "webrip" | "amzn" | "nf" | "dsnp" | "hmax" | "atvp" => Some("web"),
        "hdrip" | "fhdrip" => Some("hdrip"),
        "dvdrip" | "dvd" => Some("dvd"),
        "hdtv" => Some("hdtv"),
        _ => None,
    })
}

const EDITIONS: [&str; 7] = [
    "bw",
    "black",
    "criterion",
    "extended",
    "directors",
    "uncut",
    "imax",
];

/// How well a subtitle's release fits the video file; higher is better,
/// 4 or more counts as a match.
pub(crate) fn release_score(video: &str, release: &str) -> i32 {
    let (v, r) = (words(video), words(release));
    if v.is_empty() || r.is_empty() {
        return 0;
    }
    let mut score = 0;
    // the release group: "…x265-YAWNTiC.mkv"
    let stem = video.rsplit_once('.').map_or(video, |(s, _)| s);
    if let Some((_, group)) = stem
        .split_whitespace()
        .next()
        .unwrap_or(stem)
        .rsplit_once('-')
    {
        let g = group.to_lowercase();
        if g.len() > 1 && r.contains(&g) {
            score += 8;
        }
    }
    match (source_of(&v), source_of(&r)) {
        (Some(a), Some(b)) if a == b => score += 4,
        (Some(_), Some(_)) => score -= 4,
        _ => {}
    }
    for res in ["2160p", "1080p", "720p", "480p"] {
        let res = res.to_owned();
        if v.contains(&res) && r.contains(&res) {
            score += 1;
        }
    }
    for e in EDITIONS {
        let e = e.to_owned();
        if r.contains(&e) && !v.contains(&e) {
            score -= 3;
        }
    }
    score
}

/// An episode's id as the server knows it (ours carries "episode:").
pub(crate) fn episode_id(ep: &Episode) -> &str {
    ep.id.strip_prefix("episode:").unwrap_or(&ep.id)
}

#[cfg(test)]
mod tests {
    use super::{episode_code, parse_code, release_score};

    #[test]
    fn episode_codes() {
        assert_eq!(episode_code(1, 2), "S01E02");
        assert_eq!(episode_code(12, 104), "S12E104");
        assert_eq!(parse_code("S01E02"), Some((1, 2)));
        assert_eq!(parse_code(" s3e10 "), Some((3, 10)));
        assert_eq!(parse_code(""), None);
        assert_eq!(parse_code("Pilot"), None);
    }

    #[test]
    fn release_matching() {
        let video = "Parasite.2019.1080p.BluRay.x265-YAWNTiC.mkv";
        let group = release_score(video, "Parasite.2019.1080p.BluRay.x265-YAWNTiC_eng");
        let bluray = release_score(video, "Parasite (2019) Bluray-1080p");
        let rip = release_score(video, "PARASITE.2019.1080p.FHDRip.H264.AAC-NonDRM");
        let bw = release_score(video, "Parasite.2019.BW.PROPER.1080p.BluRay.x264-REGRET");
        assert!(
            group > bluray && bluray > rip && bluray > bw,
            "{group} {bluray} {rip} {bw}"
        );
        assert!(group >= 4 && bluray >= 4 && rip < 4 && bw < 4);
        // a file renamed without its release: only the resolution helps
        let plain = "Parasite.2019.2160p.mkv 2160p";
        assert!(
            release_score(plain, "Parasite 2019 2160p UHD Bluray (4k)")
                > release_score(plain, "PARASITE.2019.1080p.FHDRip.H264.AAC-NonDRM")
        );
        assert_eq!(release_score("", "anything"), 0);
    }
}
