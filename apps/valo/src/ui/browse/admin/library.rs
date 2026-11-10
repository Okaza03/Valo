//! Admin › Library: edit a title's details and artwork by hand, or fetch
//! them from IMDb and check them before saving.
use crate::api::Item;
use crate::api::Kind;
use anyhow::{Context, Result};
use serde_json::{json, Map, Value};
use std::path::Path;

pub(crate) mod extras;

/// The editor's fields, as text.
#[derive(Clone, Default, PartialEq)]
pub(crate) struct Form {
    pub(crate) title: String,
    pub(crate) year: String,
    pub(crate) overview: String,
    /// Comma-separated.
    pub(crate) genres: String,
    pub(crate) rating: String,
    /// Director (movies) or creators, comma-separated (series).
    pub(crate) maker: String,
    pub(crate) released: String,
    pub(crate) end_year: String,
    pub(crate) network: String,
    pub(crate) status: String,
    pub(crate) score: String,
    pub(crate) imdb_id: String,
    pub(crate) poster: String,
    pub(crate) backdrop: String,
    pub(crate) thumb: String,
    pub(crate) logo: String,
}

impl Form {
    pub(crate) fn from_item(i: &Item) -> Self {
        let opt = |o: &Option<String>| o.clone().unwrap_or_default();
        let num = |o: Option<u32>| o.map(|n| n.to_string()).unwrap_or_default();
        Self {
            title: i.title.clone(),
            year: num(i.year),
            overview: i.overview.clone(),
            genres: i.genres.join(", "),
            rating: opt(&i.rating),
            maker: match i.kind {
                Kind::Movie => opt(&i.director),
                Kind::Series => i.creators.join(", "),
            },
            released: opt(&i.released),
            end_year: num(i.end_year),
            network: opt(&i.network),
            status: opt(&i.status),
            score: i.score.map(|s| format!("{s:.1}")).unwrap_or_default(),
            imdb_id: opt(&i.imdb_id),
            poster: opt(&i.poster),
            backdrop: opt(&i.backdrop),
            thumb: opt(&i.thumb),
            logo: opt(&i.logo),
        }
    }

    /// Fill in what a lookup found; empty answers leave a field alone.
    pub(crate) fn merge(&mut self, found: &Form) {
        for (to, from) in self.fields_mut().into_iter().zip(found.fields()) {
            if !from.trim().is_empty() {
                *to = from.to_owned();
            }
        }
    }

    fn fields(&self) -> [&String; 16] {
        [
            &self.title,
            &self.year,
            &self.overview,
            &self.genres,
            &self.rating,
            &self.maker,
            &self.released,
            &self.end_year,
            &self.network,
            &self.status,
            &self.score,
            &self.imdb_id,
            &self.poster,
            &self.backdrop,
            &self.thumb,
            &self.logo,
        ]
    }

    pub(crate) fn fields_mut(&mut self) -> [&mut String; 16] {
        [
            &mut self.title,
            &mut self.year,
            &mut self.overview,
            &mut self.genres,
            &mut self.rating,
            &mut self.maker,
            &mut self.released,
            &mut self.end_year,
            &mut self.network,
            &mut self.status,
            &mut self.score,
            &mut self.imdb_id,
            &mut self.poster,
            &mut self.backdrop,
            &mut self.thumb,
            &mut self.logo,
        ]
    }

    /// The fields that differ from `base`, in the library's names.
    /// A cleared field is sent as `null`.
    pub(crate) fn changes(&self, base: &Form, kind: Kind) -> Result<Map<String, Value>, String> {
        let mut out = Map::new();
        let text = |s: &str| {
            let s = s.trim();
            if s.is_empty() {
                Value::Null
            } else {
                json!(s)
            }
        };
        let list = |s: &str| {
            json!(s
                .split(',')
                .map(str::trim)
                .filter(|g| !g.is_empty())
                .collect::<Vec<_>>())
        };
        let int = |s: &str, name: &str| -> Result<Value, String> {
            let s = s.trim();
            if s.is_empty() {
                return Ok(Value::Null);
            }
            s.parse::<u32>()
                .map(|n| json!(n))
                .map_err(|_| format!("{name} must be a number"))
        };
        if self.title != base.title {
            if self.title.trim().is_empty() {
                return Err("The title can't be empty".into());
            }
            out.insert("title".into(), text(&self.title));
        }
        if self.year != base.year {
            out.insert("year".into(), int(&self.year, "Year")?);
        }
        if self.overview != base.overview {
            out.insert("overview".into(), json!(self.overview.trim()));
        }
        if self.genres != base.genres {
            out.insert("genres".into(), list(&self.genres));
        }
        if self.rating != base.rating {
            out.insert("rating".into(), text(&self.rating));
        }
        if self.maker != base.maker {
            match kind {
                Kind::Movie => out.insert("director".into(), text(&self.maker)),
                Kind::Series => out.insert("creators".into(), list(&self.maker)),
            };
        }
        if self.released != base.released {
            out.insert("released".into(), text(&self.released));
        }
        if self.end_year != base.end_year {
            out.insert("end_year".into(), int(&self.end_year, "End year")?);
        }
        for (name, a, b) in [
            ("network", &self.network, &base.network),
            ("status", &self.status, &base.status),
            ("imdb_id", &self.imdb_id, &base.imdb_id),
            ("poster", &self.poster, &base.poster),
            ("backdrop", &self.backdrop, &base.backdrop),
            ("thumb", &self.thumb, &base.thumb),
            ("logo", &self.logo, &base.logo),
        ] {
            if a != b {
                out.insert(name.into(), text(a));
            }
        }
        if self.score != base.score {
            let s = self.score.trim().replace(',', ".");
            let v = if s.is_empty() {
                Value::Null
            } else {
                let f: f32 = s
                    .parse()
                    .map_err(|_| "Score must be a number, like 7.4".to_owned())?;
                json!(f)
            };
            out.insert("score".into(), v);
        }
        Ok(out)
    }
}

/// The server says the title (or some of its episodes) has no subtitles.
pub(crate) fn no_subs(i: &Item) -> bool {
    i.has_subtitles == Some(false) || i.missing_subtitles.is_some_and(|n| n > 0)
}

/// What a title is missing, for the list and the editor.
pub(crate) fn gaps(i: &Item) -> Vec<&'static str> {
    let mut v = Vec::new();
    if i.poster.is_none() {
        v.push("poster");
    }
    if i.backdrop.is_none() {
        v.push("backdrop");
    }
    if i.overview.trim().is_empty() {
        v.push("overview");
    }
    if i.year.is_none() {
        v.push("year");
    }
    if i.genres.is_empty() {
        v.push("genres");
    }
    v
}

pub(crate) fn read_image(path: &Path) -> Result<(Vec<u8>, &'static str)> {
    let data = std::fs::read(path).with_context(|| format!("can't read {}", path.display()))?;
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_lowercase();
    let mime = match ext.as_str() {
        "png" => "image/png",
        "webp" => "image/webp",
        _ => "image/jpeg",
    };
    Ok((data, mime))
}
