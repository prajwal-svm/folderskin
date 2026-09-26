//! A stand-in for every provider at once, on this computer: it answers each request the way the
//! provider's documentation says the real one does, and keeps what it was sent so a test can
//! check the method, the path, the content type and every field.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

/// A request as the stand-in saw it.
#[derive(Clone, Debug)]
pub struct Seen {
    pub method: String,
    /// The path and query, "/v1/images/edits".
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

/// One part of a multipart/form-data body.
#[derive(Clone, Debug)]
pub struct Part {
    pub name: String,
    pub file_name: Option<String>,
    pub content_type: Option<String>,
    pub bytes: Vec<u8>,
}

impl Part {
    pub fn text(&self) -> &str {
        std::str::from_utf8(&self.bytes).expect("a text field is UTF-8")
    }
}

impl Seen {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    /// The media type the request said it carries, without its parameters.
    pub fn content_type(&self) -> &str {
        self.header("content-type")
            .unwrap_or("")
            .split(';')
            .next()
            .unwrap_or("")
            .trim()
    }

    pub fn json(&self) -> serde_json::Value {
        assert_eq!(self.content_type(), "application/json", "{self:?}");
        serde_json::from_slice(&self.body).expect("the body is JSON")
    }

    /// The parts of a multipart/form-data body, in the order they came.
    pub fn form(&self) -> Vec<Part> {
        assert_eq!(
            self.content_type(),
            "multipart/form-data",
            "{:?}",
            self.headers
        );
        let boundary = self
            .header("content-type")
            .and_then(|v| v.split("boundary=").nth(1))
            .expect("a multipart body names its boundary")
            .trim_matches('"')
            .to_string();
        let delimiter = format!("--{boundary}").into_bytes();
        let mut parts = Vec::new();
        let mut rest: &[u8] = &self.body;
        // Everything before the first delimiter is a preamble; each part runs to the next one.
        let Some(start) = find(rest, &delimiter) else {
            return parts;
        };
        rest = &rest[start + delimiter.len()..];
        while !rest.starts_with(b"--") {
            let rest_after_crlf = rest.strip_prefix(b"\r\n").unwrap_or(rest);
            let end = find(rest_after_crlf, &delimiter).expect("every part is closed");
            let part = &rest_after_crlf[..end];
            let part = part.strip_suffix(b"\r\n").unwrap_or(part);
            let split = find(part, b"\r\n\r\n").expect("a part has headers");
            let head = std::str::from_utf8(&part[..split]).unwrap();
            let bytes = part[split + 4..].to_vec();
            let mut name = String::new();
            let mut file_name = None;
            let mut content_type = None;
            for line in head.split("\r\n") {
                let (key, value) = line.split_once(':').unwrap_or((line, ""));
                if key.eq_ignore_ascii_case("content-disposition") {
                    for piece in value.split(';').map(str::trim) {
                        if let Some(v) = piece.strip_prefix("name=") {
                            name = v.trim_matches('"').to_string();
                        } else if let Some(v) = piece.strip_prefix("filename=") {
                            file_name = Some(v.trim_matches('"').to_string());
                        }
                    }
                } else if key.eq_ignore_ascii_case("content-type") {
                    content_type = Some(value.trim().to_string());
                }
            }
            parts.push(Part {
                name,
                file_name,
                content_type,
                bytes,
            });
            rest = &rest_after_crlf[end + delimiter.len()..];
        }
        parts
    }

    /// A form's text field by name.
    pub fn field(&self, name: &str) -> Option<String> {
        self.form()
            .into_iter()
            .find(|p| p.name == name && p.file_name.is_none())
            .map(|p| p.text().to_string())
    }
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// An answer: status, headers, body.
pub struct Answer {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Answer {
    pub fn json(status: u16, body: serde_json::Value) -> Answer {
        Answer {
            status,
            headers: vec![("Content-Type".into(), "application/json".into())],
            body: body.to_string().into_bytes(),
        }
    }

    pub fn bytes(content_type: &str, body: Vec<u8>) -> Answer {
        Answer {
            status: 200,
            headers: vec![("Content-Type".into(), content_type.into())],
            body,
        }
    }

    pub fn header(mut self, name: &str, value: &str) -> Answer {
        self.headers.push((name.into(), value.into()));
        self
    }
}

/// Every request the stand-in has seen, in order.
pub type Log = Arc<Mutex<Vec<Seen>>>;

/// Starts the stand-in on a free port. `answer` is given each request and the stand-in's own
/// address ("http://127.0.0.1:1234"), for answers that link back to it.
pub fn serve(answer: impl Fn(&Seen, &str) -> Answer + Send + 'static) -> (String, Log) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    let log: Log = Arc::new(Mutex::new(Vec::new()));
    let (seen, here) = (Arc::clone(&log), base.clone());
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { return };
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            if reader.read_line(&mut line).is_err() || line.is_empty() {
                continue;
            }
            let mut words = line.split_whitespace();
            let (method, path) = (
                words.next().unwrap_or("").to_string(),
                words.next().unwrap_or("").to_string(),
            );
            let mut headers = Vec::new();
            loop {
                let mut header = String::new();
                reader.read_line(&mut header).unwrap();
                let header = header.trim_end().to_string();
                if header.is_empty() {
                    break;
                }
                if let Some((k, v)) = header.split_once(':') {
                    headers.push((k.trim().to_string(), v.trim().to_string()));
                }
            }
            let chunked = headers.iter().any(|(k, v)| {
                k.eq_ignore_ascii_case("transfer-encoding") && v.eq_ignore_ascii_case("chunked")
            });
            let body = if chunked {
                read_chunked(&mut reader)
            } else {
                let length = headers
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
                    .and_then(|(_, v)| v.parse::<usize>().ok())
                    .unwrap_or(0);
                let mut body = vec![0; length];
                reader.read_exact(&mut body).unwrap();
                body
            };
            let request = Seen {
                method,
                path,
                headers,
                body,
            };
            let reply = answer(&request, &here);
            seen.lock().unwrap().push(request);
            let mut head = format!(
                "HTTP/1.1 {} Stand-in\r\nConnection: close\r\nContent-Length: {}\r\n",
                reply.status,
                reply.body.len()
            );
            for (k, v) in &reply.headers {
                head.push_str(&format!("{k}: {v}\r\n"));
            }
            head.push_str("\r\n");
            let mut stream = stream;
            let _ = stream.write_all(head.as_bytes());
            let _ = stream.write_all(&reply.body);
        }
    });
    (base, log)
}

fn read_chunked(reader: &mut impl BufRead) -> Vec<u8> {
    let mut body = Vec::new();
    loop {
        let mut size = String::new();
        reader.read_line(&mut size).unwrap();
        let size = usize::from_str_radix(size.trim(), 16).unwrap_or(0);
        let mut chunk = vec![0; size + 2];
        reader.read_exact(&mut chunk).unwrap();
        if size == 0 {
            return body;
        }
        body.extend_from_slice(&chunk[..size]);
    }
}

/// Runs a future on a runtime of its own, as the app's and the command line's are.
pub fn run<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future)
}
