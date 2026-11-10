//! Full-window playback of a queue (an episode run, a movie, or local files).
use crate::addons::Lookup;
use crate::api::SubtitleTrack;
use crate::player::Source;
use crate::PathBuf;

pub struct PlayItem {
    pub title: String,
    /// Under the title: "2020 · 2h 0m", "S01E01 · Pilot".
    pub subtitle: String,
    /// An episode of a series (the next button says "Next Episode").
    pub episode: bool,
    pub source: Source,
    /// The video's file name, when the server sent it (a stream URL doesn't say).
    pub file_name: Option<String>,
    /// Progress key; None for local files.
    pub key: Option<String>,
    /// Resume position in seconds.
    pub start: f64,
    /// Subtitle files on the server.
    pub subtitles: Vec<SubtitleTrack>,
    /// Ask the admin's subtitle addons for more; None once they've answered.
    pub addons: Option<Lookup>,
}

impl PlayItem {
    pub fn local(path: PathBuf) -> Self {
        let source = Source::File(path);
        Self {
            title: source.name(),
            subtitle: String::new(),
            episode: false,
            source,
            file_name: None,
            key: None,
            start: 0.0,
            subtitles: Vec::new(),
            addons: None,
        }
    }
}
