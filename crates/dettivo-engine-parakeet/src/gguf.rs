//! Just enough GGUF to identify a model before parakeet.cpp loads it: the
//! magic, and the `general.name` key (`nvidia/parakeet-tdt-0.6b-v2`), which
//! decides the languages the engine accepts.

use std::io::{BufReader, Read};
use std::path::Path;

/// What the header said, or why it could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    /// `general.name`, when present.
    pub name: Option<String>,
}

/// Reads the header; a file without the GGUF magic is an error naming
/// what was found.
pub fn read_header(path: &Path) -> Result<Header, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut r = BufReader::new(file);
    let mut magic = [0u8; 4];
    r.read_exact(&mut magic)
        .map_err(|_| "file is shorter than a GGUF header".to_string())?;
    if &magic != b"GGUF" {
        return Err(format!(
            "not a GGUF file (magic {:?})",
            String::from_utf8_lossy(&magic)
        ));
    }
    let version = u32_of(&mut r)?;
    if !(2..=3).contains(&version) {
        return Err(format!("GGUF version {version} is not supported"));
    }
    let _tensors = u64_of(&mut r)?;
    let kv_count = u64_of(&mut r)?;
    let mut name = None;
    for _ in 0..kv_count.min(4096) {
        let key = string_of(&mut r)?;
        let kind = u32_of(&mut r)?;
        let value = value_of(&mut r, kind)?;
        if key == "general.name" {
            name = value;
        }
    }
    Ok(Header { name })
}

fn u32_of(r: &mut impl Read) -> Result<u32, String> {
    let mut b = [0u8; 4];
    r.read_exact(&mut b).map_err(|e| e.to_string())?;
    Ok(u32::from_le_bytes(b))
}

fn u64_of(r: &mut impl Read) -> Result<u64, String> {
    let mut b = [0u8; 8];
    r.read_exact(&mut b).map_err(|e| e.to_string())?;
    Ok(u64::from_le_bytes(b))
}

fn string_of(r: &mut impl Read) -> Result<String, String> {
    let len = u64_of(r)?;
    if len > 1 << 20 {
        return Err("GGUF string longer than 1 MiB".into());
    }
    let mut buf = vec![0u8; len as usize];
    r.read_exact(&mut buf).map_err(|e| e.to_string())?;
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

/// Skips a value of `kind`, returning it only when it is a string.
/// The longest array a header may carry; a real model's vocabularies stay
/// well under it, and a crafted count above it is refused before any read.
const MAX_ARRAY_LEN: u64 = 1 << 22;
/// How deep arrays may nest; GGUF writers never nest, so one level inside
/// an array is already generous.
const MAX_ARRAY_DEPTH: u32 = 2;

fn value_of(r: &mut impl Read, kind: u32) -> Result<Option<String>, String> {
    value_at(r, kind, 0)
}

fn value_at(r: &mut impl Read, kind: u32, depth: u32) -> Result<Option<String>, String> {
    let fixed = |r: &mut dyn Read, n: usize| -> Result<(), String> {
        let mut buf = vec![0u8; n];
        r.read_exact(&mut buf).map_err(|e| e.to_string())
    };
    match kind {
        0 | 1 | 7 => fixed(r, 1).map(|_| None),
        2 | 3 => fixed(r, 2).map(|_| None),
        4..=6 => fixed(r, 4).map(|_| None),
        10..=12 => fixed(r, 8).map(|_| None),
        8 => string_of(r).map(Some),
        9 => {
            if depth >= MAX_ARRAY_DEPTH {
                return Err(format!(
                    "GGUF array nests deeper than {MAX_ARRAY_DEPTH} levels"
                ));
            }
            let elem = u32_of(r)?;
            let n = u64_of(r)?;
            if n > MAX_ARRAY_LEN {
                return Err(format!(
                    "GGUF array of {n} elements exceeds the {MAX_ARRAY_LEN} the header reader accepts"
                ));
            }
            for _ in 0..n {
                value_at(r, elem, depth + 1)?;
            }
            Ok(None)
        }
        other => Err(format!("GGUF value type {other} is unknown")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gguf(kvs: &[(&str, u32, Vec<u8>)]) -> Vec<u8> {
        let mut out = b"GGUF".to_vec();
        out.extend_from_slice(&3u32.to_le_bytes());
        out.extend_from_slice(&0u64.to_le_bytes());
        out.extend_from_slice(&(kvs.len() as u64).to_le_bytes());
        for (k, kind, v) in kvs {
            out.extend_from_slice(&(k.len() as u64).to_le_bytes());
            out.extend_from_slice(k.as_bytes());
            out.extend_from_slice(&kind.to_le_bytes());
            out.extend_from_slice(v);
        }
        out
    }

    fn s(text: &str) -> Vec<u8> {
        let mut v = (text.len() as u64).to_le_bytes().to_vec();
        v.extend_from_slice(text.as_bytes());
        v
    }

    #[test]
    fn the_name_is_read_past_other_value_kinds() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("m.gguf");
        let mut array = 4u32.to_le_bytes().to_vec();
        array.extend_from_slice(&2u64.to_le_bytes());
        array.extend_from_slice(&7u32.to_le_bytes());
        array.extend_from_slice(&9u32.to_le_bytes());
        std::fs::write(
            &path,
            gguf(&[
                ("general.architecture", 8, s("parakeet")),
                ("parakeet.encoder.n_layers", 4, 24u32.to_le_bytes().to_vec()),
                ("parakeet.tdt.durations", 9, array),
                (
                    "parakeet.preprocessor.preemph",
                    6,
                    0.97f32.to_le_bytes().to_vec(),
                ),
                ("general.name", 8, s("nvidia/parakeet-tdt-0.6b-v2")),
            ]),
        )
        .unwrap();
        let header = read_header(&path).unwrap();
        assert_eq!(header.name.as_deref(), Some("nvidia/parakeet-tdt-0.6b-v2"));
    }

    #[test]
    fn a_file_without_the_magic_names_what_it_found() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.gguf");
        std::fs::write(&path, b"RIFF....").unwrap();
        let err = read_header(&path).unwrap_err();
        assert!(
            err.contains("not a GGUF file") && err.contains("RIFF"),
            "{err}"
        );
        std::fs::write(&path, b"GG").unwrap();
        assert!(read_header(&path).unwrap_err().contains("shorter"));
    }

    #[test]
    fn a_crafted_array_count_is_refused_before_it_is_read() {
        let mut v = Vec::new();
        v.extend_from_slice(&4u32.to_le_bytes()); // element kind u32
        v.extend_from_slice(&u64::MAX.to_le_bytes()); // absurd count
        let err = value_of(&mut v.as_slice(), 9).unwrap_err();
        assert!(err.contains("exceeds"), "{err}");
        let mut nested = Vec::new();
        for _ in 0..3 {
            nested.extend_from_slice(&9u32.to_le_bytes());
            nested.extend_from_slice(&1u64.to_le_bytes());
        }
        let err = value_of(&mut nested.as_slice(), 9).unwrap_err();
        assert!(err.contains("nests deeper"), "{err}");
    }
}
