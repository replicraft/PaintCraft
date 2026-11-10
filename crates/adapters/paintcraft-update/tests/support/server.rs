//! A minimal HTTP/1.1 server on 127.0.0.1 with scripted replies and a request log.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

#[derive(Clone)]
pub struct Reply {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    /// Announce this Content-Length instead of the body's length.
    pub announce: Option<usize>,
}

impl Reply {
    pub fn ok(body: impl Into<Vec<u8>>) -> Self {
        Self {
            status: 200,
            headers: Vec::new(),
            body: body.into(),
            announce: None,
        }
    }

    pub fn redirect(location: &str) -> Self {
        Self {
            status: 302,
            headers: vec![("Location".into(), location.into())],
            body: Vec::new(),
            announce: None,
        }
    }

    pub fn status(status: u16) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: Vec::new(),
            announce: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Request {
    pub path: String,
    pub headers: HashMap<String, String>,
}

pub struct Server {
    pub base: String,
    routes: Arc<Mutex<HashMap<String, Reply>>>,
    log: Arc<Mutex<Vec<Request>>>,
}

impl Server {
    pub fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let base = format!(
            "http://127.0.0.1:{}",
            listener.local_addr().expect("addr").port()
        );
        let routes: Arc<Mutex<HashMap<String, Reply>>> = Arc::default();
        let log: Arc<Mutex<Vec<Request>>> = Arc::default();
        let (r, l) = (routes.clone(), log.clone());
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let (r, l) = (r.clone(), l.clone());
                thread::spawn(move || serve(stream, &r, &l));
            }
        });
        Self { base, routes, log }
    }

    pub fn route(&self, path: &str, reply: Reply) {
        self.routes
            .lock()
            .expect("routes")
            .insert(path.to_owned(), reply);
    }

    pub fn requests(&self) -> Vec<Request> {
        self.log.lock().expect("log").clone()
    }
}

fn serve(stream: TcpStream, routes: &Mutex<HashMap<String, Reply>>, log: &Mutex<Vec<Request>>) {
    let mut reader = BufReader::new(stream.try_clone().expect("clone"));
    let mut line = String::new();
    if reader.read_line(&mut line).unwrap_or(0) == 0 {
        return;
    }
    let path = line.split_whitespace().nth(1).unwrap_or("/").to_owned();
    let mut headers = HashMap::new();
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).unwrap_or(0) == 0 || header.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            headers.insert(name.trim().to_lowercase(), value.trim().to_owned());
        }
    }
    log.lock().expect("log").push(Request {
        path: path.clone(),
        headers,
    });
    let reply = routes
        .lock()
        .expect("routes")
        .get(&path)
        .cloned()
        .unwrap_or_else(|| Reply::status(404));
    let mut out = stream;
    let _ = write!(
        out,
        "HTTP/1.1 {} X\r\nContent-Length: {}\r\nConnection: close\r\n",
        reply.status,
        reply.announce.unwrap_or(reply.body.len())
    );
    for (name, value) in &reply.headers {
        let _ = write!(out, "{name}: {value}\r\n");
    }
    let _ = out.write_all(b"\r\n");
    let _ = out.write_all(&reply.body);
    let _ = out.flush();
}
