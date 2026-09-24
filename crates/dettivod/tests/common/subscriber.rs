//! A raw connection that subscribes to the event stream with a buffer of
//! its own and collects the notifications, asserting the subscription id
//! on each; the dictation tests read the state sequence and the overflow
//! through it.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::Daemon;

/// A raw connection that subscribes and collects notifications.
pub struct Subscriber {
    reader: BufReader<UnixStream>,
    stream: UnixStream,
    id: String,
}

impl Subscriber {
    pub fn open(daemon: &Daemon, topics: &[&str], buffer: u32) -> Self {
        let stream = UnixStream::connect(daemon.tree.socket()).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(30)))
            .unwrap();
        let mut s = Self {
            reader: BufReader::new(stream.try_clone().unwrap()),
            stream,
            id: String::new(),
        };
        let req = json!({"jsonrpc": "2.0", "id": "s", "method": "events.subscribe", "params": {"topics": topics, "buffer": buffer}});
        s.stream.write_all(format!("{req}\n").as_bytes()).unwrap();
        let mut line = String::new();
        s.reader.read_line(&mut line).unwrap();
        let v: Value = serde_json::from_str(line.trim_end()).unwrap();
        s.id = v["result"]["subscription_id"].as_str().unwrap().to_string();
        assert_eq!(v["result"]["buffer"], buffer);
        s
    }

    /// Reads notifications until `stop` matches one or the timeout passes.
    pub fn collect(&mut self, stop: impl Fn(&Value) -> bool, timeout: Duration) -> Vec<Value> {
        let deadline = Instant::now() + timeout;
        let mut out = Vec::new();
        while Instant::now() < deadline {
            let mut line = String::new();
            match self.reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {
                    let v: Value = serde_json::from_str(line.trim_end()).unwrap();
                    assert_eq!(v["method"], "events.notify");
                    assert_eq!(v["params"]["subscription_id"], self.id);
                    let done = stop(&v["params"]);
                    out.push(v["params"].clone());
                    if done {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        out
    }
}
