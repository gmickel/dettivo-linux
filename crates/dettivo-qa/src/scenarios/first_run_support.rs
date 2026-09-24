//! What the first-run drives share (fn-21, ADR 0024): a profile-local
//! models directory with the real tiny.en linked in (so a download never
//! touches the machine's models), a fixture model server with a small
//! test model and the catalogue that points at it, the Hyprland
//! configuration files that make a profile look provisioned, the golden
//! snippet from the `dettivo` command line, and the state file's
//! completion mark.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use super::{Context, binary};
use crate::profile::Profile;

/// A minimal HTTP file server on loopback (`GET`, `Range` honoured) for
/// the catalogue downloads of a drive; `DETTIVO_MODEL_SERVER` is not needed
/// because the catalogue names the server's URL outright.
pub struct ModelServer {
    /// `http://127.0.0.1:<port>`.
    pub url: String,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl ModelServer {
    /// Serves `dir`.
    pub fn serve(dir: PathBuf) -> Result<Self, String> {
        let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| format!("bind: {e}"))?;
        let port = listener.local_addr().map_err(|e| e.to_string())?.port();
        listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        let stop = Arc::new(AtomicBool::new(false));
        let s = stop.clone();
        let thread = std::thread::spawn(move || {
            while !s.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let _ = stream.set_nonblocking(false);
                        answer(stream, &dir);
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    Err(_) => break,
                }
            }
        });
        Ok(Self {
            url: format!("http://127.0.0.1:{port}"),
            stop,
            thread: Some(thread),
        })
    }
}

impl Drop for ModelServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn answer(mut stream: TcpStream, dir: &Path) {
    let mut reader = BufReader::new(stream.try_clone().expect("clone"));
    let mut request = String::new();
    if reader.read_line(&mut request).is_err() {
        return;
    }
    let mut range: Option<u64> = None;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).is_err() || line.trim().is_empty() {
            break;
        }
        if let Some(value) = line.strip_prefix("Range: bytes=") {
            range = value.trim().trim_end_matches('-').parse().ok();
        }
    }
    let path = request
        .split_whitespace()
        .nth(1)
        .unwrap_or("/")
        .trim_start_matches('/');
    let Ok(body) = std::fs::read(dir.join(path)) else {
        let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n");
        return;
    };
    let total = body.len() as u64;
    let (status, from) = match range {
        Some(from) if from < total => ("206 Partial Content", from as usize),
        _ => ("200 OK", 0),
    };
    let slice = &body[from..];
    let mut head = format!(
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\n",
        slice.len()
    );
    if from > 0 {
        head.push_str(&format!(
            "Content-Range: bytes {from}-{}/{total}\r\n",
            total - 1
        ));
    }
    head.push_str("\r\n");
    let _ = stream.write_all(head.as_bytes());
    // Chunks with a pause between them, so the progress is visible.
    for chunk in slice.chunks(256 * 1024) {
        if stream.write_all(chunk).is_err() {
            return;
        }
        std::thread::sleep(Duration::from_millis(15));
    }
}

/// `sha256sum` of a file, as the catalogue pins it.
pub fn sha256(path: &Path) -> Result<String, String> {
    let out = Command::new("sha256sum")
        .arg(path)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("sha256sum: {e}"))?;
    String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .next()
        .map(str::to_string)
        .ok_or_else(|| "sha256sum printed nothing".to_string())
}

/// The directory the machine's tiny.en lives in, for the fixture server
/// to serve from: the served bytes are never copied.
pub fn real_tiny_dir(profile: &Profile) -> Result<PathBuf, String> {
    let tiny = profile.root.join("data/dettivo/models/whisper/tiny.en");
    if !tiny.join("ggml-tiny.en.bin").is_file() {
        return Err(
            "the local tiny.en model is missing (scripts/models/fetch-test-model.sh)".into(),
        );
    }
    std::fs::canonicalize(&tiny).map_err(|e| e.to_string())
}

/// Stages the download catalogue against the profile-owned model tree.
/// The downloadable test model starts absent; Profile owns cleanup and retention.
pub fn stage_models(profile: &Profile, server_url: &str) -> Result<(PathBuf, PathBuf), String> {
    let tiny = real_tiny_dir(profile)?;
    let models = profile.models_dir();
    let file = tiny.join("ggml-tiny.en.bin");
    let sha = sha256(&file)?;
    let tiny_sha = sha.clone();
    let size = std::fs::metadata(&file).map(|m| m.len()).unwrap_or(0);
    let catalogue = format!(
        r#"version = 21

[[models]]
provider = "whisper"
id = "tiny.en"
display_name = "Tiny (English)"
kind = "stt"
file_name = "ggml-tiny.en.bin"
size_bytes = {tiny_size}
url = "{url}/ggml-tiny.en.bin"
sha256 = "{tiny_sha}"
license = "MIT"
redistribution = "OpenAI Whisper weights converted by whisper.cpp"
languages = ["en"]
english_only = true

[[models]]
provider = "whisper"
id = "test"
display_name = "Test"
kind = "stt"
file_name = "ggml-test.bin"
size_bytes = {size}
url = "{url}/ggml-tiny.en.bin"
sha256 = "{sha}"
license = "MIT"
redistribution = "a drive fixture"
languages = ["en"]
english_only = true
default = true
recommended_for = "English · the drive's small test model"
"#,
        tiny_size = size,
        url = server_url,
    );
    let catalogue_path = profile.root.join("catalogue.toml");
    std::fs::write(&catalogue_path, catalogue).map_err(|e| e.to_string())?;
    Ok((models, catalogue_path))
}

/// The profile's `config.toml` for a first-run drive: the engines beside
/// the build, tiny.en selected, the models directory and catalogue of
/// `stage_models` when given, no daemon hotkey backend.
pub fn write_config(ctx: &Context<'_>, catalogue: Option<&Path>) -> Result<(), String> {
    let bin_dir = binary(ctx.repo_root, "dettivod")
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_default();
    let mut text = format!(
        "[hotkeys]\nbackend = \"none\"\n[engines]\ndirectory = \"{}\"\n[speech]\nmodel = \"tiny.en\"\n[dictation]\nlanguage = \"en\"\n",
        bin_dir.display()
    );
    if let Some(catalogue) = catalogue {
        text.push_str(&format!(
            "[models]\ncatalogue_file = \"{}\"\nverify_on_start = false\n",
            catalogue.display()
        ));
    }
    let file = ctx.profile.root.join("cfg/dettivo/config.toml");
    std::fs::create_dir_all(file.parent().unwrap_or(&ctx.profile.root))
        .map_err(|e| e.to_string())?;
    std::fs::write(&file, text).map_err(|e| format!("config: {e}"))
}

/// The snippet `dettivo setup hyprland --stdout` prints against the
/// profile's daemon: the golden the Keys step must show.
pub fn golden_snippet(
    ctx: &Context<'_>,
    extra: &BTreeMap<String, String>,
) -> Result<String, String> {
    let cli = binary(ctx.repo_root, "dettivo")?;
    let mut env = ctx.profile.env();
    env.extend(extra.clone());
    let out = crate::profile::command(cli, &env)
        .args(["setup", "hyprland", "--stdout"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map_err(|e| format!("dettivo setup: {e}"))?;
    if !out.status.success() {
        return Err(format!("dettivo setup exited {:?}", out.status.code()));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Writes the files a provisioned Hyprland profile carries: the snippet
/// and a main configuration that sources it.
pub fn provision_hyprland(profile: &Profile, snippet: &str) -> Result<(), String> {
    let hypr = profile.root.join("cfg/hypr");
    std::fs::create_dir_all(&hypr).map_err(|e| e.to_string())?;
    std::fs::write(hypr.join("dettivo.conf"), snippet).map_err(|e| e.to_string())?;
    std::fs::write(
        hypr.join("hyprland.conf"),
        "# the user's own configuration\nsource = ~/.config/hypr/dettivo.conf\n",
    )
    .map_err(|e| e.to_string())
}

/// The `[first_run]` table of the profile's state file, once the app has
/// written it (the app saves on quit).
pub fn wait_first_run_state(
    profile: &Profile,
    timeout: Duration,
    complete: bool,
) -> Result<toml::Table, String> {
    let path = profile.root.join("state/dettivo/state.toml");
    let deadline = Instant::now() + timeout;
    loop {
        if let Ok(text) = std::fs::read_to_string(&path) {
            if let Ok(doc) = text.parse::<toml::Table>() {
                let table = doc
                    .get("first_run")
                    .and_then(|v| v.as_table())
                    .cloned()
                    .unwrap_or_default();
                let done = table
                    .get("completed_at")
                    .and_then(|v| v.as_str())
                    .is_some_and(|s| !s.is_empty());
                if done == complete {
                    return Ok(table);
                }
            }
        }
        if Instant::now() > deadline {
            return Err(format!(
                "{} never carried first_run.completed_at {} within {timeout:?}",
                path.display(),
                if complete { "set" } else { "empty" }
            ));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Reads the whole answer of one JSON line to a socket, for the drives
/// that talk to the daemon beside the driver.
pub fn read_all(stream: &mut TcpStream) -> String {
    let mut s = String::new();
    let _ = stream.read_to_string(&mut s);
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn staging_uses_the_profile_model_tree_and_honours_retention() {
        for keep in [false, true] {
            let real = tempfile::tempdir().unwrap();
            let store = tempfile::tempdir().unwrap();
            let tiny = real.path().join("whisper/tiny.en");
            std::fs::create_dir_all(&tiny).unwrap();
            std::fs::write(tiny.join("ggml-tiny.en.bin"), b"model").unwrap();
            let source = crate::profile::ModelSource {
                real: real.path().into(),
                store: store.path().into(),
            };
            let mut profile = Profile::create("firststage", Some(&source)).unwrap();
            let (models, catalogue) = stage_models(&profile, "http://127.0.0.1:1").unwrap();
            assert_eq!(models, profile.models_dir());
            assert!(catalogue.is_file());
            assert!(!models.join("whisper/test/ggml-test.bin").exists());
            let private = std::fs::canonicalize(&models).unwrap();
            let root = profile.root.clone();
            if keep {
                profile.keep();
            }
            drop(profile);
            assert_eq!(private.exists(), keep);
            if keep {
                std::fs::remove_dir_all(&root).unwrap();
            }
            assert!(tiny.join("ggml-tiny.en.bin").exists());
        }
    }

    #[test]
    fn the_model_server_answers_whole_files_and_ranges() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.bin"), b"0123456789").unwrap();
        let server = ModelServer::serve(dir.path().to_path_buf()).unwrap();
        let get = |range: Option<&str>| {
            let addr = server.url.trim_start_matches("http://").to_string();
            let mut stream = TcpStream::connect(addr).unwrap();
            let mut req = String::from("GET /a.bin HTTP/1.1\r\nHost: x\r\n");
            if let Some(r) = range {
                req.push_str(&format!("Range: bytes={r}-\r\n"));
            }
            req.push_str("\r\n");
            stream.write_all(req.as_bytes()).unwrap();
            read_all(&mut stream)
        };
        let whole = get(None);
        assert!(whole.starts_with("HTTP/1.1 200 OK"), "{whole}");
        assert!(whole.ends_with("0123456789"), "{whole}");
        let tail = get(Some("7"));
        assert!(tail.starts_with("HTTP/1.1 206"), "{tail}");
        assert!(tail.ends_with("789"), "{tail}");
        assert!(get(Some("99")).starts_with("HTTP/1.1 200 OK"));
    }
}
