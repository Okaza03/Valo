//! Everything a person has been in, from Wikidata (free, no key): movies and
//! series they acted in or directed, with IMDb ids so the app can tell which
//! are on the server, and a photo when Wikidata has one.
use std::collections::HashMap;
use std::time::Duration;

use anyhow::{anyhow, Result};
use serde_json::Value;

use crate::api::Kind;

/// One of their titles.
#[derive(Clone, Debug)]
pub struct Credit {
    pub imdb: String,
    pub title: String,
    pub year: Option<u32>,
    pub kind: Kind,
}

#[derive(Clone, Debug, Default)]
pub struct Filmography {
    /// A photo of them, from Wikimedia Commons.
    pub photo: Option<String>,
    /// Newest first.
    pub credits: Vec<Credit>,
}

/// Photos for these people, by name (only those Wikidata has one for).
/// For a name several people share, the best-known one's.
pub fn photos(names: &[String]) -> Result<HashMap<String, String>> {
    let values: Vec<String> = names
        .iter()
        .map(|n| format!("\"{}\"@en", n.trim().replace(['\\', '"'], "")))
        .collect();
    let query = format!(
        r#"SELECT ?name ?img ?links WHERE {{
  VALUES ?name {{ {} }}
  ?p rdfs:label ?name; wdt:P31 wd:Q5; wdt:P18 ?img; wikibase:sitelinks ?links.
}}"#,
        values.join(" ")
    );
    let v = sparql(&query)?;
    let mut best: HashMap<String, (u64, String)> = HashMap::new();
    for b in v
        .pointer("/results/bindings")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let get = |k: &str| b.pointer(&format!("/{k}/value")).and_then(Value::as_str);
        let (Some(name), Some(img)) = (get("name"), get("img")) else {
            continue;
        };
        let links = get("links").and_then(|l| l.parse().ok()).unwrap_or(0);
        if best.get(name).is_none_or(|(n, _)| links > *n) {
            best.insert(name.to_owned(), (links, commons(img)));
        }
    }
    Ok(best.into_iter().map(|(k, (_, u))| (k, u)).collect())
}

/// A Commons file link, as a 400-pixel-wide image.
fn commons(url: &str) -> String {
    let file = url.rsplit('/').next().unwrap_or_default();
    format!("https://commons.wikimedia.org/wiki/Special:FilePath/{file}?width=400")
}

fn sparql(query: &str) -> Result<Value> {
    Ok(ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(5))
        .timeout(Duration::from_secs(40))
        .user_agent(concat!(
            "Valo/",
            env!("CARGO_PKG_VERSION"),
            " (https://github.com/Okaza03/Valo)"
        ))
        .build()
        .get("https://query.wikidata.org/sparql")
        .query("query", query)
        .set("Accept", "application/sparql-results+json")
        .call()
        .map_err(|e| anyhow!("Wikidata: {e}"))?
        .into_json()?)
}

/// Look `name` up. Among people with that exact name, the one with the most
/// titles wins (the actor, not a namesake).
pub fn lookup(name: &str) -> Result<Filmography> {
    let name = name.trim().replace(['\\', '"'], "");
    let query = format!(
        r#"SELECT ?p (SAMPLE(?img) AS ?photo) ?imdb (SAMPLE(?t) AS ?title) (MIN(?d) AS ?date) (MAX(?s) AS ?series) WHERE {{
  ?p rdfs:label "{name}"@en; wdt:P31 wd:Q5.
  OPTIONAL {{ ?p wdt:P18 ?img }}
  ?work wdt:P161|wdt:P57 ?p; wdt:P345 ?imdb.
  FILTER(STRSTARTS(?imdb, "tt"))
  FILTER NOT EXISTS {{ ?work wdt:P31 wd:Q21191270 }}
  OPTIONAL {{ ?work wdt:P577|wdt:P580 ?d }}
  OPTIONAL {{ ?work rdfs:label ?t FILTER(LANG(?t) = "en") }}
  BIND(EXISTS {{ ?work wdt:P31/wdt:P279* wd:Q5398426 }} AS ?s)
}} GROUP BY ?p ?imdb"#
    );
    Ok(parse(&sparql(&query)?))
}

fn parse(v: &Value) -> Filmography {
    let rows = v
        .pointer("/results/bindings")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let get = |b: &Value, k: &str| {
        b.get(k)
            .and_then(|x| x.get("value"))
            .and_then(Value::as_str)
            .map(str::to_owned)
    };
    // people with this name, by how many titles they have
    let mut people: HashMap<String, usize> = HashMap::new();
    for b in &rows {
        if let Some(p) = get(b, "p") {
            *people.entry(p).or_default() += 1;
        }
    }
    let Some(who) = people.into_iter().max_by_key(|(_, n)| *n).map(|(p, _)| p) else {
        return Filmography::default();
    };
    let mine: Vec<&Value> = rows
        .iter()
        .filter(|b| get(b, "p").as_deref() == Some(who.as_str()))
        .collect();
    let photo = mine
        .iter()
        .find_map(|b| get(b, "photo"))
        .map(|u| commons(&u));
    let mut credits: Vec<Credit> = mine
        .iter()
        .filter_map(|b| {
            Some(Credit {
                imdb: get(b, "imdb")?,
                title: get(b, "title")?,
                year: get(b, "date").and_then(|d| d.get(..4)?.parse().ok()),
                kind: if get(b, "series").as_deref() == Some("true") {
                    Kind::Series
                } else {
                    Kind::Movie
                },
            })
        })
        .collect();
    credits.sort_by_key(|c| std::cmp::Reverse(c.year));
    Filmography { photo, credits }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_the_person_with_most_titles() {
        let b = |p: &str, imdb: &str, t: &str, d: &str, s: &str| {
            serde_json::json!({
                "p": {"value": p}, "imdb": {"value": imdb}, "title": {"value": t},
                "date": {"value": d}, "series": {"value": s},
                "photo": {"value": "http://commons.wikimedia.org/wiki/Special:FilePath/Bryan%20Cranston.jpg"}
            })
        };
        let v = serde_json::json!({"results": {"bindings": [
            b("Q1", "tt0903747", "Breaking Bad", "2008-01-20T00:00:00Z", "true"),
            b("Q1", "tt9243946", "El Camino", "2019-10-11T00:00:00Z", "false"),
            b("Q2", "tt0000001", "A namesake's film", "1990-01-01T00:00:00Z", "false"),
        ]}});
        let f = parse(&v);
        assert_eq!(f.credits.len(), 2);
        assert_eq!(f.credits[0].title, "El Camino"); // newest first
        assert_eq!(f.credits[1].kind, Kind::Series);
        assert_eq!(f.credits[1].year, Some(2008));
        assert!(f.photo.unwrap().ends_with("Bryan%20Cranston.jpg?width=400"));
    }
}
