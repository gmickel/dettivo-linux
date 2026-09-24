//! The driver interface every scenario is written against (R2). Two
//! implementations exist: `cua` over the cua-driver command line tool, and
//! the in-repo `atspi` driver. A scenario sees only this module.

pub mod atspi;
pub mod bounded;
pub mod cua;
mod cua_answer;
mod x11;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// A launched application under a driver.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct App {
    /// The process id the driver addresses.
    pub pid: u32,
    /// The driver's window handle, when it has one (X11 window id).
    pub window: Option<u64>,
}

/// One node of the accessibility tree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Element {
    /// The driver's index for the element within the last snapshot; the
    /// order of a walk, so it changes when the tree changes.
    pub index: u32,
    /// A stable identity across snapshots: the AT-SPI object path for the
    /// fallback driver, cua-driver's element id or a label key otherwise.
    pub id: String,
    /// Observed AT-SPI object path when a driver uses changing action
    /// tokens as `id`. Keyboard coverage uses this stable identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_id: Option<String>,
    /// The AT-SPI role name.
    pub role: String,
    /// The accessible name.
    pub name: String,
    /// The value or text, when the element exposes one.
    pub value: Option<String>,
    /// Screen bounds `(x, y, width, height)` when known.
    pub bounds: Option<(i32, i32, i32, i32)>,
    /// Whether the element can take keyboard focus (AT-SPI `focusable`).
    #[serde(default)]
    pub focusable: bool,
    /// Whether the element holds keyboard focus in this snapshot.
    #[serde(default)]
    pub focused: bool,
    /// Whether the element is enabled (AT-SPI `enabled`); a driver that
    /// cannot tell says so with `true`.
    #[serde(default = "enabled_default")]
    pub enabled: bool,
    /// The index of the element's parent in the same snapshot, when the
    /// driver walks a tree (the fallback driver does; cua-driver's list
    /// is flat).
    #[serde(default)]
    pub parent: Option<u32>,
}

fn enabled_default() -> bool {
    true
}

/// How a scenario launches a binary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Launch {
    /// The executable.
    pub program: PathBuf,
    /// Arguments.
    pub args: Vec<String>,
    /// Environment additions (the profile supplies the rest).
    pub env: BTreeMap<String, String>,
}

/// Why a driver call failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DriverError {
    /// A required piece of the desktop is missing; names it.
    Missing(String),
    /// The element was not found within the wait.
    NotFound(String),
    /// The driver tool or bus answered with an error.
    Failed(String),
}

impl std::fmt::Display for DriverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing(m) => write!(f, "missing: {m}"),
            Self::NotFound(m) => write!(f, "not found: {m}"),
            Self::Failed(m) => write!(f, "driver failed: {m}"),
        }
    }
}

impl std::error::Error for DriverError {}

/// The calls a scenario may make. Every method is synchronous; a driver
/// that needs an async runtime owns one internally.
pub trait Driver {
    /// The driver's name for receipts (`cua` or `atspi`).
    fn name(&self) -> &'static str;
    /// Checks the desktop pieces the driver needs, naming the missing one.
    fn preflight(&mut self) -> Result<(), DriverError>;
    /// Launches a binary and waits until it has a window.
    fn launch(&mut self, launch: &Launch, timeout: Duration) -> Result<App, DriverError>;
    /// The whole accessibility tree of the app, flattened.
    fn snapshot(&mut self, app: &App) -> Result<Vec<Element>, DriverError>;
    /// Finds an element by exact accessible name.
    fn find(&mut self, app: &App, name: &str) -> Result<Element, DriverError> {
        self.snapshot(app)?
            .into_iter()
            .find(|e| e.name == name)
            .ok_or_else(|| DriverError::NotFound(format!("element named {name:?}")))
    }
    /// Waits until an element with the accessible name appears.
    fn wait_for_label(
        &mut self,
        app: &App,
        name: &str,
        timeout: Duration,
    ) -> Result<Element, DriverError> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            match self.find(app, name) {
                Ok(e) => return Ok(e),
                Err(DriverError::NotFound(_)) if std::time::Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(100));
                }
                Err(e) => return Err(e),
            }
        }
    }
    /// Clicks an element.
    fn click(&mut self, app: &App, element: &Element) -> Result<(), DriverError>;
    /// Types text into the focused element of the app.
    fn type_text(&mut self, app: &App, text: &str) -> Result<(), DriverError>;
    /// Presses one key or chord by name: `Tab`, `Shift+Tab`, `Escape`,
    /// `Super+F`, or a printable character such as `/` and `?`.
    fn press_key(&mut self, app: &App, key: &str) -> Result<(), DriverError>;
    /// Reads an element's value or text.
    fn read_value(&mut self, app: &App, element: &Element) -> Result<String, DriverError>;
    /// Writes a PNG screenshot of the app's window to `path`.
    fn screenshot(&mut self, app: &App, path: &Path) -> Result<(), DriverError>;
    /// Terminates the app.
    fn close(&mut self, app: &App) -> Result<(), DriverError>;
}

/// Builds a driver by name.
pub fn by_name(name: &str) -> Result<Box<dyn Driver>, DriverError> {
    match name {
        "cua" => Ok(Box::new(cua::CuaDriver::new())),
        "atspi" => Ok(Box::new(atspi::AtspiDriver::new())),
        other => Err(DriverError::Failed(format!(
            "unknown driver {other:?}; use cua or atspi"
        ))),
    }
}

/// Writes a PNG from packed RGB rows without an image crate: a minimal
/// encoder (uncompressed deflate blocks) that every viewer reads.
pub fn write_png(path: &Path, width: u32, height: u32, rgb: &[u8]) -> std::io::Result<()> {
    fn crc32(data: &[u8]) -> u32 {
        let mut crc = 0xFFFF_FFFFu32;
        for &b in data {
            crc ^= u32::from(b);
            for _ in 0..8 {
                crc = if crc & 1 == 1 {
                    0xEDB8_8320 ^ (crc >> 1)
                } else {
                    crc >> 1
                };
            }
        }
        !crc
    }
    fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let mut body = Vec::with_capacity(4 + data.len());
        body.extend_from_slice(kind);
        body.extend_from_slice(data);
        out.extend_from_slice(&body);
        out.extend_from_slice(&crc32(&body).to_be_bytes());
    }
    let row_len = width as usize * 3;
    let expected = row_len * height as usize;
    if rgb.len() != expected {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!(
                "rgb buffer holds {} bytes, {width}x{height} needs {expected}",
                rgb.len()
            ),
        ));
    }
    let mut raw = Vec::with_capacity((row_len + 1) * height as usize);
    for y in 0..height as usize {
        raw.push(0);
        raw.extend_from_slice(&rgb[y * row_len..(y + 1) * row_len]);
    }
    let mut zlib = vec![0x78, 0x01];
    let mut a: u32 = 1;
    let mut b: u32 = 0;
    for &byte in &raw {
        a = (a + u32::from(byte)) % 65521;
        b = (b + a) % 65521;
    }
    let blocks: Vec<&[u8]> = raw.chunks(65535).collect();
    for (i, block) in blocks.iter().enumerate() {
        let last = i + 1 == blocks.len();
        zlib.push(u8::from(last));
        let len = block.len() as u16;
        zlib.extend_from_slice(&len.to_le_bytes());
        zlib.extend_from_slice(&(!len).to_le_bytes());
        zlib.extend_from_slice(block);
    }
    if blocks.is_empty() {
        zlib.extend_from_slice(&[1, 0, 0, 0xFF, 0xFF]);
    }
    zlib.extend_from_slice(&((b << 16) | a).to_be_bytes());
    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &zlib);
    chunk(&mut out, b"IEND", &[]);
    std::fs::write(path, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_writer_produces_a_signature_and_chunks() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x.png");
        write_png(
            &path,
            2,
            2,
            &[255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255],
        )
        .unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(
            &bytes[..8],
            &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]
        );
        assert!(bytes.windows(4).any(|w| w == b"IHDR"));
        assert!(bytes.windows(4).any(|w| w == b"IEND"));
    }

    #[test]
    fn unknown_driver_names_are_refused() {
        assert!(by_name("nope").is_err());
    }
}
