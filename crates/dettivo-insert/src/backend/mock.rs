//! The QA mock backend (`DETTIVO_MOCK_INSERT`): appends the text to
//! `$XDG_STATE_HOME/dettivo/qa/inserted.txt` and reports backend `mock`,
//! so a drive can read back what would have been typed.

use std::io::Write;
use std::path::PathBuf;

use super::{Availability, Backend, Ctx, Failure, Kind, Performed};
use crate::session::Session;
use crate::settings::PasteKeys;

/// The mock: one instance stands in for the keystroke backends, a second
/// (`mock_clipboard`) for clipboard-only mode.
pub struct MockBackend {
    /// The file the text is appended to.
    pub inserted_file: PathBuf,
    /// Which kind of backend this instance plays.
    pub kind: Kind,
}

impl Backend for MockBackend {
    fn name(&self) -> &'static str {
        match self.kind {
            Kind::ClipboardOnly => "mock_clipboard",
            _ => "mock",
        }
    }

    fn kind(&self) -> Kind {
        self.kind
    }

    fn availability(&self, _session: &Session) -> Availability {
        Availability::Available
    }

    fn insert(&self, text: &str, ctx: &Ctx<'_>) -> Result<Performed, Failure> {
        // The keystroke mock stands in for a typing backend, so it runs the
        // origin check a typing backend runs; the clipboard mock types
        // nothing and needs none.
        if self.kind != Kind::ClipboardOnly {
            ctx.recheck()?;
        }
        if let Some(parent) = self.inserted_file.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| Failure::Before(format!("{}: {e}", parent.display())))?;
        }
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.inserted_file)
            .map_err(|e| Failure::Before(format!("{}: {e}", self.inserted_file.display())))?;
        file.write_all(text.as_bytes())
            .and_then(|()| file.write_all(b"\n"))
            .map_err(|e| Failure::During(format!("{}: {e}", self.inserted_file.display())))?;
        Ok(match self.kind {
            Kind::ClipboardOnly => Performed::copied(),
            _ => Performed::typed(text.chars().count()),
        })
    }

    fn paste_keystroke(&self, _keys: PasteKeys, _ctx: &Ctx<'_>) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Settings;

    #[test]
    fn mock_appends_lines_to_the_inserted_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("qa/inserted.txt");
        let backend = MockBackend {
            inserted_file: file.clone(),
            kind: Kind::Keystroke,
        };
        let session = Session::default();
        let settings = Settings::default();
        let ctx = Ctx {
            session: &session,
            settings: &settings,
            app_id: "x",
            keystroke: None,
            recheck: None,
        };
        backend.insert("one", &ctx).unwrap();
        backend.insert("two", &ctx).unwrap();
        assert_eq!(std::fs::read_to_string(file).unwrap(), "one\ntwo\n");
    }
}
