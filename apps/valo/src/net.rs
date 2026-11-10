//! HTTP for the whole app, on ureq 3, in the shape the app was written
//! for (ureq 2's): requests are built with `set`/`query`/`timeout` and an
//! answer of 400 or more comes back as [`Error::Status`] with the response,
//! so callers can match on the code and still read what the server said.
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fmt;
use std::io::Read;
use std::time::Duration;

/// Settings for an [`Agent`]; `None` leaves ureq's default.
#[derive(Default)]
pub struct Builder {
    connect: Option<Duration>,
    read: Option<Duration>,
    timeout: Option<Duration>,
    idle_per_host: Option<usize>,
    user_agent: Option<String>,
}

impl Builder {
    pub fn new() -> Self {
        Self::default()
    }
    /// Time to connect.
    pub fn timeout_connect(mut self, d: Duration) -> Self {
        self.connect = Some(d);
        self
    }
    /// Time from sending the request to the response's headers.
    pub fn timeout_read(mut self, d: Duration) -> Self {
        self.read = Some(d);
        self
    }
    /// Time for the whole request, body included.
    pub fn timeout(mut self, d: Duration) -> Self {
        self.timeout = Some(d);
        self
    }
    pub fn max_idle_connections_per_host(mut self, n: usize) -> Self {
        self.idle_per_host = Some(n);
        self
    }
    pub fn user_agent(mut self, ua: &str) -> Self {
        self.user_agent = Some(ua.to_owned());
        self
    }
    pub fn build(self) -> Agent {
        let mut c = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_connect(self.connect)
            .timeout_recv_response(self.read)
            .timeout_global(self.timeout);
        if let Some(n) = self.idle_per_host {
            c = c.max_idle_connections_per_host(n);
        }
        if let Some(ua) = self.user_agent {
            c = c.user_agent(ua);
        }
        Agent(ureq::Agent::new_with_config(c.build()))
    }
}

/// Shares connections between requests; cheap to clone.
#[derive(Clone)]
pub struct Agent(ureq::Agent);

impl Agent {
    pub fn get(&self, url: &str) -> Request {
        self.request("GET", url)
    }
    pub fn post(&self, url: &str) -> Request {
        self.request("POST", url)
    }
    pub fn delete(&self, url: &str) -> Request {
        self.request("DELETE", url)
    }
    pub fn request(&self, method: &str, url: &str) -> Request {
        Request {
            agent: self.0.clone(),
            method: method.to_owned(),
            url: url.to_owned(),
            headers: Vec::new(),
            query: Vec::new(),
            timeout: None,
        }
    }
}

#[must_use]
pub struct Request {
    agent: ureq::Agent,
    method: String,
    url: String,
    headers: Vec<(String, String)>,
    query: Vec<(String, String)>,
    timeout: Option<Duration>,
}

impl Request {
    /// Set a header.
    pub fn set(mut self, name: &str, value: &str) -> Self {
        self.headers.retain(|(n, _)| !n.eq_ignore_ascii_case(name));
        self.headers.push((name.to_owned(), value.to_owned()));
        self
    }
    /// Add `name=value` to the URL's query.
    pub fn query(mut self, name: &str, value: &str) -> Self {
        self.query.push((name.to_owned(), value.to_owned()));
        self
    }
    /// Time for this whole request, body included.
    pub fn timeout(mut self, d: Duration) -> Self {
        self.timeout = Some(d);
        self
    }
    pub fn call(self) -> Result<Response, Error> {
        self.run(())
    }
    pub fn send_json(self, body: impl Serialize) -> Result<Response, Error> {
        let data = serde_json::to_vec(&body).map_err(|e| Error::Transport(e.to_string().into()))?;
        let r = if self.has("Content-Type") {
            self
        } else {
            self.set("Content-Type", "application/json; charset=utf-8")
        };
        r.run(data)
    }
    pub fn send_bytes(self, body: &[u8]) -> Result<Response, Error> {
        self.run(body.to_vec())
    }

    fn has(&self, name: &str) -> bool {
        self.headers
            .iter()
            .any(|(n, _)| n.eq_ignore_ascii_case(name))
    }

    fn run(self, body: impl ureq::AsSendBody) -> Result<Response, Error> {
        let mut url = self.url;
        for (i, (k, v)) in self.query.iter().enumerate() {
            url.push(if i == 0 && !url.contains('?') {
                '?'
            } else {
                '&'
            });
            url.push_str(&form_encode(k));
            url.push('=');
            url.push_str(&form_encode(v));
        }
        let mut b = ureq::http::Request::builder()
            .method(self.method.as_str())
            .uri(url.as_str());
        for (n, v) in &self.headers {
            b = b.header(n.as_str(), v.as_str());
        }
        let req = b
            .body(body)
            .map_err(|e| Error::Transport(e.to_string().into()))?;
        let req = match self.timeout {
            Some(d) => self
                .agent
                .configure_request(req)
                .timeout_global(Some(d))
                .build(),
            None => req,
        };
        let r = self
            .agent
            .run(req)
            .map_err(|e| Error::Transport(e.into()))?;
        let r = Response(r);
        match r.status() {
            code @ 400.. => Err(Error::Status(code, Box::new(r))),
            _ => Ok(r),
        }
    }
}

/// `application/x-www-form-urlencoded`, as a query value.
fn form_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub struct Response(ureq::http::Response<ureq::Body>);

impl fmt::Debug for Response {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Response({})", self.status())
    }
}

/// No size limit on what's read: a library or an update can be big.
const NO_LIMIT: u64 = u64::MAX;

impl Response {
    pub fn status(&self) -> u16 {
        self.0.status().as_u16()
    }
    /// "Not Found" for 404, and so on.
    pub fn status_text(&self) -> &str {
        self.0.status().canonical_reason().unwrap_or("")
    }
    pub fn header(&self, name: &str) -> Option<&str> {
        self.0.headers().get(name).and_then(|v| v.to_str().ok())
    }
    /// The media type without its parameters ("text/html"), or "text/plain".
    pub fn content_type(&self) -> &str {
        self.header("Content-Type")
            .and_then(|c| c.split(';').next())
            .map(str::trim)
            .filter(|c| !c.is_empty())
            .unwrap_or("text/plain")
    }
    pub fn into_json<T: DeserializeOwned>(mut self) -> std::io::Result<T> {
        self.0
            .body_mut()
            .with_config()
            .limit(NO_LIMIT)
            .read_json()
            .map_err(std::io::Error::other)
    }
    pub fn into_string(mut self) -> std::io::Result<String> {
        self.0
            .body_mut()
            .with_config()
            .limit(NO_LIMIT)
            .read_to_string()
            .map_err(std::io::Error::other)
    }
    pub fn into_reader(self) -> impl Read + Send + 'static {
        self.0
            .into_body()
            .into_with_config()
            .limit(NO_LIMIT)
            .reader()
    }
}

/// Why a request didn't work.
#[derive(Debug)]
pub enum Error {
    /// The server answered with this code (400 or more).
    Status(u16, Box<Response>),
    /// No answer: the address, the network, TLS, a timeout…
    Transport(Transport),
}

#[derive(Debug)]
pub struct Transport(String);

impl From<ureq::Error> for Transport {
    fn from(e: ureq::Error) -> Self {
        Transport(e.to_string())
    }
}

impl From<String> for Transport {
    fn from(s: String) -> Self {
        Transport(s)
    }
}

impl fmt::Display for Transport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Status(code, r) => write!(f, "server answered {code} {}", r.status_text()),
            Error::Transport(t) => t.fmt(f),
        }
    }
}

impl std::error::Error for Error {}

#[cfg(test)]
mod tests {
    #[test]
    fn encodes_query_values() {
        assert_eq!(
            super::form_encode("Jo Ann & co/ø"),
            "Jo+Ann+%26+co%2F%C3%B8"
        );
    }
}
