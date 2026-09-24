//! Emit the packaged configuration reference without a running daemon.

use std::io::{self, Write};

fn main() -> io::Result<()> {
    io::stdout()
        .lock()
        .write_all(dettivo_core::config::DEFAULT_TOML.as_bytes())
}
