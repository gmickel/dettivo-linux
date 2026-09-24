//! A minimal ZIP writer and reader for the archive export: entries are
//! stored uncompressed (audio is already compact, JSON is small), so the
//! format needs no compression library and any unzip tool opens the file.

/// Builds an archive in memory.
#[derive(Debug, Default)]
pub struct Writer {
    buf: Vec<u8>,
    entries: Vec<Entry>,
}

#[derive(Debug)]
struct Entry {
    name: String,
    crc: u32,
    size: u32,
    offset: u32,
}

impl Writer {
    /// An empty archive.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds one stored file.
    pub fn add(&mut self, name: &str, data: &[u8]) {
        let offset = self.buf.len() as u32;
        let crc = crc32(data);
        let size = data.len() as u32;
        self.buf.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        self.buf.extend_from_slice(&20u16.to_le_bytes()); // version needed
        self.buf.extend_from_slice(&0x0800u16.to_le_bytes()); // utf-8 names
        self.buf.extend_from_slice(&0u16.to_le_bytes()); // stored
        self.buf.extend_from_slice(&0u16.to_le_bytes()); // time
        self.buf.extend_from_slice(&0x21u16.to_le_bytes()); // date 1980-01-01
        self.buf.extend_from_slice(&crc.to_le_bytes());
        self.buf.extend_from_slice(&size.to_le_bytes());
        self.buf.extend_from_slice(&size.to_le_bytes());
        self.buf
            .extend_from_slice(&(name.len() as u16).to_le_bytes());
        self.buf.extend_from_slice(&0u16.to_le_bytes());
        self.buf.extend_from_slice(name.as_bytes());
        self.buf.extend_from_slice(data);
        self.entries.push(Entry {
            name: name.to_string(),
            crc,
            size,
            offset,
        });
    }

    /// Writes the central directory and returns the archive bytes.
    pub fn finish(mut self) -> Vec<u8> {
        let start = self.buf.len() as u32;
        for e in &self.entries {
            self.buf.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
            self.buf.extend_from_slice(&20u16.to_le_bytes()); // made by
            self.buf.extend_from_slice(&20u16.to_le_bytes()); // needed
            self.buf.extend_from_slice(&0x0800u16.to_le_bytes());
            self.buf.extend_from_slice(&0u16.to_le_bytes());
            self.buf.extend_from_slice(&0u16.to_le_bytes());
            self.buf.extend_from_slice(&0x21u16.to_le_bytes());
            self.buf.extend_from_slice(&e.crc.to_le_bytes());
            self.buf.extend_from_slice(&e.size.to_le_bytes());
            self.buf.extend_from_slice(&e.size.to_le_bytes());
            self.buf
                .extend_from_slice(&(e.name.len() as u16).to_le_bytes());
            self.buf.extend_from_slice(&0u16.to_le_bytes()); // extra
            self.buf.extend_from_slice(&0u16.to_le_bytes()); // comment
            self.buf.extend_from_slice(&0u16.to_le_bytes()); // disk
            self.buf.extend_from_slice(&0u16.to_le_bytes()); // internal attrs
            self.buf.extend_from_slice(&0u32.to_le_bytes()); // external attrs
            self.buf.extend_from_slice(&e.offset.to_le_bytes());
            self.buf.extend_from_slice(e.name.as_bytes());
        }
        let end = self.buf.len() as u32;
        self.buf.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
        self.buf.extend_from_slice(&0u16.to_le_bytes());
        self.buf.extend_from_slice(&0u16.to_le_bytes());
        self.buf
            .extend_from_slice(&(self.entries.len() as u16).to_le_bytes());
        self.buf
            .extend_from_slice(&(self.entries.len() as u16).to_le_bytes());
        self.buf.extend_from_slice(&(end - start).to_le_bytes());
        self.buf.extend_from_slice(&start.to_le_bytes());
        self.buf.extend_from_slice(&0u16.to_le_bytes());
        self.buf
    }
}

fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
    bytes
        .get(at..at + 2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    bytes
        .get(at..at + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

/// Reads every entry of a stored archive: `(name, bytes)`. A compressed
/// entry or a broken directory is an error naming the entry.
pub fn read(bytes: &[u8]) -> Result<Vec<(String, Vec<u8>)>, String> {
    let eocd = (0..bytes.len().saturating_sub(21))
        .rev()
        .find(|&i| u32_at(bytes, i) == Some(0x0605_4b50))
        .ok_or("not a zip archive (no end of central directory)")?;
    let count = u16_at(bytes, eocd + 10).ok_or("truncated archive")? as usize;
    let mut at = u32_at(bytes, eocd + 16).ok_or("truncated archive")? as usize;
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        if u32_at(bytes, at) != Some(0x0201_4b50) {
            return Err("broken central directory".into());
        }
        let method = u16_at(bytes, at + 10).ok_or("truncated archive")?;
        let crc = u32_at(bytes, at + 16).ok_or("truncated archive")?;
        let size = u32_at(bytes, at + 24).ok_or("truncated archive")? as usize;
        let name_len = u16_at(bytes, at + 28).ok_or("truncated archive")? as usize;
        let extra_len = u16_at(bytes, at + 30).ok_or("truncated archive")? as usize;
        let comment_len = u16_at(bytes, at + 32).ok_or("truncated archive")? as usize;
        let offset = u32_at(bytes, at + 42).ok_or("truncated archive")? as usize;
        let name = std::str::from_utf8(
            bytes
                .get(at + 46..at + 46 + name_len)
                .ok_or("truncated archive")?,
        )
        .map_err(|_| "entry name is not UTF-8")?
        .to_string();
        if method != 0 {
            return Err(format!("{name}: compressed entries are not supported"));
        }
        if u32_at(bytes, offset) != Some(0x0403_4b50) {
            return Err(format!("{name}: broken local header"));
        }
        let local_name = u16_at(bytes, offset + 26).ok_or("truncated archive")? as usize;
        let local_extra = u16_at(bytes, offset + 28).ok_or("truncated archive")? as usize;
        // Offsets from the archive are untrusted: every sum is checked so a
        // crafted entry fails as truncated instead of wrapping around.
        let start = offset
            .checked_add(30 + local_name + local_extra)
            .ok_or_else(|| format!("{name}: truncated data"))?;
        let end = start
            .checked_add(size)
            .ok_or_else(|| format!("{name}: truncated data"))?;
        let data = bytes
            .get(start..end)
            .ok_or_else(|| format!("{name}: truncated data"))?
            .to_vec();
        if crc32(&data) != crc {
            return Err(format!("{name}: checksum mismatch"));
        }
        out.push((name, data));
        at = at
            .checked_add(46 + name_len + extra_len + comment_len)
            .ok_or("truncated archive")?;
    }
    Ok(out)
}

/// CRC-32 (IEEE), as ZIP uses it.
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archives_round_trip_and_checksums_are_standard() {
        assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
        let mut w = Writer::new();
        w.add("items.json", b"{\"items\":[]}");
        w.add("dictations/a/microphone.wav", &[0u8; 64]);
        let bytes = w.finish();
        let entries = read(&bytes).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].0, "items.json");
        assert_eq!(entries[0].1, b"{\"items\":[]}");
        assert_eq!(entries[1].1.len(), 64);
        assert!(read(b"nope").is_err());
        let mut broken = bytes.clone();
        broken[40] ^= 0xff; // the first data byte of items.json
        assert!(read(&broken).unwrap_err().contains("checksum"));
    }
}
