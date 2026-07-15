//! Just enough std-only HTTP/1.1 to serve the embedded analysis GUI to a
//! local browser: one request per connection, static bodies, JSON, and
//! Server-Sent Events. Not a general web server — it never needs to be.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::TcpStream;

/// Largest accepted request body (PGN import is the biggest payload).
const MAX_BODY: usize = 1 << 20;

pub struct Request {
    pub method: String,
    pub path: String,
    pub query: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Request {
    /// A query parameter, percent-decoded.
    pub fn param(&self, key: &str) -> Option<&str> {
        self.query
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// A numeric query parameter, falling back to `default` when absent or
    /// malformed, clamped to `[lo, hi]`.
    pub fn num<T>(&self, key: &str, default: T, lo: T, hi: T) -> T
    where
        T: std::str::FromStr + PartialOrd,
    {
        let v = self.param(key).and_then(|v| v.parse().ok()).unwrap_or(default);
        if v < lo {
            lo
        } else if v > hi {
            hi
        } else {
            v
        }
    }
}

/// Read one request: request line, headers (only Content-Length matters), body.
pub fn read_request(reader: &mut BufReader<TcpStream>) -> io::Result<Request> {
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_string();
    let target = parts.next().unwrap_or("/").to_string();
    if method.is_empty() {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "empty request"));
    }

    let mut content_length = 0usize;
    loop {
        let mut h = String::new();
        if reader.read_line(&mut h)? == 0 {
            break;
        }
        let h = h.trim();
        if h.is_empty() {
            break;
        }
        if let Some((name, value)) = h.split_once(':')
            && name.eq_ignore_ascii_case("content-length")
        {
            content_length = value.trim().parse().unwrap_or(0);
        }
    }
    if content_length > MAX_BODY {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "body too large"));
    }
    let mut body = vec![0u8; content_length];
    reader.read_exact(&mut body)?;

    let (path, query) = match target.split_once('?') {
        Some((p, q)) => (p, q),
        None => (target.as_str(), ""),
    };
    let query = query
        .split('&')
        .filter(|s| !s.is_empty())
        .map(|pair| match pair.split_once('=') {
            Some((k, v)) => (percent_decode(k), percent_decode(v)),
            None => (percent_decode(pair), String::new()),
        })
        .collect();

    Ok(Request {
        method,
        path: percent_decode(path),
        query,
        body,
    })
}

/// Decode `%XX` escapes (the frontend always encodes with
/// `encodeURIComponent`, which never produces `+` for spaces).
pub fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let (Some(hi), Some(lo)) = (hex(b[i + 1]), hex(b[i + 2]))
        {
            out.push(hi << 4 | lo);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

pub fn respond(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    body: &[u8],
) -> io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(body)
}

pub fn respond_json(stream: &mut TcpStream, json: &str) -> io::Result<()> {
    respond(stream, "200 OK", "application/json", json.as_bytes())
}

pub fn respond_bad_request(stream: &mut TcpStream, msg: &str) -> io::Result<()> {
    let body = format!("{{\"error\":{}}}", jstr(msg));
    respond(stream, "400 Bad Request", "application/json", body.as_bytes())
}

/// Send the response head for a Server-Sent Events stream.
pub fn start_sse(stream: &mut TcpStream) -> io::Result<()> {
    stream.write_all(
        b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
    )
}

/// Emit one SSE event. `data` must not contain raw newlines (ours is JSON).
pub fn sse_event(stream: &mut TcpStream, event: &str, data: &str) -> io::Result<()> {
    write!(stream, "event: {event}\ndata: {data}\n\n")
}

/// A JSON string literal (quoted, escaped).
pub fn jstr(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A JSON array from already-encoded element strings.
pub fn jarr<I: IntoIterator<Item = String>>(items: I) -> String {
    let mut out = String::from("[");
    for (i, item) in items.into_iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&item);
    }
    out.push(']');
    out
}

/// `null` or a JSON number.
pub fn jopt(v: Option<i32>) -> String {
    match v {
        Some(n) => n.to_string(),
        None => "null".to_string(),
    }
}
