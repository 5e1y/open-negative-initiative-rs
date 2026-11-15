//! A bare HTTP/1.1 client: what a browser or a foreign program sends, outside ONI's own client.

use std::io::{Read, Write};
use std::net::TcpStream;

pub struct Answer {
    pub status: u16,
    pub head: String,
    pub body: String,
}

/// One request; `headers` are whole lines such as `Origin: https://example.com`.
pub fn request(method: &str, url: &str, headers: &[&str], body: &str) -> Answer {
    let url = url::Url::parse(url).unwrap();
    let mut stream = TcpStream::connect((url.host_str().unwrap(), url.port().unwrap())).unwrap();
    let target = match url.query() {
        Some(query) => format!("{}?{query}", url.path()),
        None => url.path().to_string(),
    };
    let mut text = format!("{method} {target} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nContent-Length: {}\r\n", body.len());
    for header in headers {
        text.push_str(&format!("{header}\r\n"));
    }
    text.push_str("\r\n");
    text.push_str(body);
    stream.write_all(text.as_bytes()).unwrap();
    let mut answer = String::new();
    stream.read_to_string(&mut answer).unwrap();
    let (head, body) = answer.split_once("\r\n\r\n").unwrap_or((&answer, ""));
    let status = head.split(' ').nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    Answer { status, head: head.to_string(), body: body.to_string() }
}

pub fn get(url: &str) -> Answer {
    request("GET", url, &[], "")
}
