//! The least of HTTP/1.1 the simulated service needs: one request a connection, its body by its
//! Content-Length, and an answer that closes the connection.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::time::Duration;

/// The largest body read; a Kit change of 200 entries takes about 6 KB.
const BODY: usize = 1 << 20;

pub struct Request {
    pub method: String,
    pub path: String,
    pub query: Vec<(String, String)>,
    /// Header names in lower case.
    headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Request {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(n, _)| n == name).map(|(_, v)| v.as_str())
    }

    pub fn param(&self, name: &str) -> Option<&str> {
        self.query.iter().find(|(n, _)| n == name).map(|(_, v)| v.as_str())
    }

    /// The body's form fields, as a token or revocation sends them.
    pub fn form(&self) -> Vec<(String, String)> {
        url::form_urlencoded::parse(&self.body).into_owned().collect()
    }
}

pub fn read(stream: &TcpStream) -> std::io::Result<Request> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let mut words = line.split_whitespace();
    let (method, target) = (words.next().unwrap_or_default().to_string(), words.next().unwrap_or_default().to_string());
    let mut headers = Vec::new();
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 || line.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            headers.push((name.trim().to_ascii_lowercase(), value.trim().to_string()));
        }
    }
    let length = headers.iter().find(|(n, _)| n == "content-length").and_then(|(_, v)| v.parse::<usize>().ok()).unwrap_or(0);
    let mut body = vec![0u8; length.min(BODY)];
    reader.read_exact(&mut body)?;
    let (path, query) = target.split_once('?').unwrap_or((&target, ""));
    let query = url::form_urlencoded::parse(query.as_bytes()).into_owned().collect();
    Ok(Request { method, path: path.to_string(), query, headers, body })
}

pub struct Response {
    status: u16,
    headers: Vec<(&'static str, String)>,
    body: Vec<u8>,
}

impl Response {
    pub fn new(status: u16, kind: &str, body: impl Into<Vec<u8>>) -> Response {
        Response { status, headers: vec![("Content-Type", kind.to_string())], body: body.into() }
    }

    pub fn json(status: u16, value: &serde_json::Value) -> Response {
        Response::new(status, "application/json", value.to_string())
    }

    /// The contract's error shape, `{"error": …}`.
    pub fn error(status: u16, reason: &str) -> Response {
        Response::json(status, &serde_json::json!({ "error": reason }))
    }

    pub fn html(status: u16, body: String) -> Response {
        Response::new(status, "text/html; charset=utf-8", body)
    }

    pub fn redirect(location: String) -> Response {
        Response::new(302, "text/plain; charset=utf-8", "").with("Location", location)
    }

    pub fn empty(status: u16) -> Response {
        Response { status, headers: Vec::new(), body: Vec::new() }
    }

    pub fn with(mut self, name: &'static str, value: String) -> Response {
        self.headers.push((name, value));
        self
    }

    pub fn write(&self, stream: &mut TcpStream) -> std::io::Result<()> {
        let mut head = format!("HTTP/1.1 {} {}\r\n", self.status, reason(self.status));
        for (name, value) in &self.headers {
            head.push_str(&format!("{name}: {value}\r\n"));
        }
        head.push_str(&format!("Content-Length: {}\r\nConnection: close\r\n\r\n", self.body.len()));
        stream.write_all(head.as_bytes())?;
        stream.write_all(&self.body)?;
        stream.flush()
    }
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        302 => "Found",
        304 => "Not Modified",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        429 => "Too Many Requests",
        503 => "Service Unavailable",
        _ => "Unknown",
    }
}

/// Text made safe inside HTML.
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            other => out.push(other),
        }
    }
    out
}
