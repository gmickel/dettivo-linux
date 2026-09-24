//! A local HTTP server the provider tests answer from, so no test ever
//! reaches the network: it binds a loopback port, serves one canned
//! response per path, records every request body it was sent and stops
//! when it is dropped.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// One route: the path, the status and the JSON body. A 3xx status makes
/// the body the `Location` the response points at.
pub type Route = (String, u16, String);

/// A loopback HTTP server with a fixed set of routes.
#[derive(Debug)]
pub struct MockServer {
    port: u16,
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
    requests: Arc<Mutex<Vec<(String, String)>>>,
}

impl MockServer {
    /// Starts a server on a free loopback port serving `routes`; a path
    /// with no route answers 404.
    pub fn start(routes: Vec<Route>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
        let port = listener.local_addr().expect("addr").port();
        listener
            .set_nonblocking(true)
            .expect("non-blocking listener");
        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        let requests = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::clone(&requests);
        let handle = std::thread::spawn(move || {
            while !flag.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => serve(stream, &routes, &seen),
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(std::time::Duration::from_millis(5));
                    }
                    Err(_) => break,
                }
            }
        });
        Self {
            port,
            stop,
            handle: Some(handle),
            requests,
        }
    }

    /// The base URL clients point at.
    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// Every request received so far: the path and the body.
    pub fn requests(&self) -> Vec<(String, String)> {
        self.requests
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }
}

impl Drop for MockServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn serve(mut stream: TcpStream, routes: &[Route], seen: &Mutex<Vec<(String, String)>>) {
    let _ = stream.set_nonblocking(false);
    let mut reader = BufReader::new(match stream.try_clone() {
        Ok(s) => s,
        Err(_) => return,
    });
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() {
        return;
    }
    let path = request_line
        .split_whitespace()
        .nth(1)
        .unwrap_or("/")
        .to_string();
    let mut length = 0usize;
    loop {
        let mut header = String::new();
        match reader.read_line(&mut header) {
            Ok(0) => break,
            Ok(_) => {
                let trimmed = header.trim_end();
                if trimmed.is_empty() {
                    break;
                }
                if let Some(value) = trimmed.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = value.trim().parse().unwrap_or(0);
                }
            }
            Err(_) => break,
        }
    }
    let mut received = vec![0u8; length];
    if length > 0 {
        use std::io::Read;
        let _ = reader.read_exact(&mut received);
    }
    seen.lock().unwrap_or_else(|p| p.into_inner()).push((
        path.clone(),
        String::from_utf8_lossy(&received).into_owned(),
    ));
    let (status, body) = routes
        .iter()
        .find(|(route, _, _)| *route == path)
        .map(|(_, status, body)| (*status, body.clone()))
        .unwrap_or((404, "{}".to_string()));
    let response = if (300..400).contains(&status) {
        format!(
            "HTTP/1.1 {status} Redirect\r\nLocation: {body}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        )
    } else {
        format!(
            "HTTP/1.1 {status} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
    };
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_server_answers_its_routes_and_404s_the_rest() {
        let server = MockServer::start(vec![("/ok".into(), 200, r#"{"ok":true}"#.into())]);
        let client = reqwest::blocking::Client::new();
        let response = client.get(format!("{}/ok", server.url())).send().unwrap();
        assert!(response.status().is_success());
        assert_eq!(response.text().unwrap(), r#"{"ok":true}"#);
        let missing = client.get(format!("{}/nope", server.url())).send().unwrap();
        assert_eq!(missing.status().as_u16(), 404);
        let posted = client
            .post(format!("{}/ok", server.url()))
            .body("payload")
            .send()
            .unwrap();
        assert!(posted.status().is_success());
        let seen = server.requests();
        assert_eq!(seen.len(), 3);
        assert_eq!(seen[2], ("/ok".to_string(), "payload".to_string()));
    }
}
