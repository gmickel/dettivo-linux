//! Resolves an evidence route's `ref` against the repository, one rule
//! per kind: a `#[test]` function by name under its crate (or a Qt test
//! under `qt/`), a contract fixture path, a pipeline name, a scenario id,
//! a visual surface with an optional state, a pack step, a bench step, a
//! docs page, a script or a human receipt. Every rule answers with what
//! it found or why it did not; a route that resolves exists, which is
//! not the same as having passed.

use std::path::{Path, PathBuf};

/// Root Markdown files a `docs` route may name beside `docs/`.
pub const ROOT_DOCS: &[&str] = &[
    "README.md",
    "CONTRIBUTING.md",
    "NOTICE.md",
    "STRATEGY.md",
    "CHANGELOG.md",
];

/// The repository facts the rules read once.
pub struct Resolver {
    root: PathBuf,
    /// Each visual surface with its states, from the manifest.
    surfaces: Vec<(String, Vec<String>)>,
    pack_steps: Vec<String>,
}

impl Resolver {
    /// Reads the visual manifest and the pack list under `root`.
    pub fn new(root: &Path) -> Result<Self, String> {
        let manifest = root.join("qa/visual/manifest.toml");
        let surfaces = match std::fs::read_to_string(&manifest) {
            Ok(text) => surfaces(&text),
            Err(_) => Vec::new(),
        };
        let mut pack_steps: Vec<String> = crate::pack::packs()
            .iter()
            .flat_map(|p| p.steps.iter().map(move |s| format!("{}/{}", p.name, s.id)))
            .collect();
        pack_steps.extend(
            crate::pack::release::STEP_IDS
                .iter()
                .map(|s| format!("release/{s}")),
        );
        Ok(Self {
            root: root.to_path_buf(),
            surfaces,
            pack_steps,
        })
    }

    /// Resolves one reference; `Ok` carries what was found.
    pub fn resolve(&self, kind: &str, reference: &str) -> Result<String, String> {
        match kind {
            "unit" => self.unit(reference),
            "contract" => self.existing(
                reference,
                &[
                    "crates/dettivo-proto/fixtures/",
                    "crates/dettivo-rest/fixtures/",
                ],
            ),
            "pipeline" => name_in(reference, crate::pipeline::PIPELINES, "pipeline"),
            "drive" => match crate::scenarios::by_id(reference) {
                Some(s) => Ok(format!("scenario {}: {}", s.id(), s.summary())),
                None => Err("no scenario with that id (`dettivo-qa list` names them)".into()),
            },
            "visual" => {
                let (surface, state) = match reference.split_once('/') {
                    Some((s, st)) => (s, Some(st)),
                    None => (reference, None),
                };
                let Some((_, states)) = self.surfaces.iter().find(|(s, _)| s == surface) else {
                    return Err(format!(
                        "no surface named {surface:?} in qa/visual/manifest.toml"
                    ));
                };
                match state {
                    None => Ok(format!("surface {surface} in qa/visual/manifest.toml")),
                    Some(st) if states.iter().any(|s| s == st) => Ok(format!(
                        "surface {surface}, state {st} in qa/visual/manifest.toml"
                    )),
                    Some(st) => Err(format!(
                        "surface {surface} has no state {st:?} in qa/visual/manifest.toml; its states are {}",
                        states.join(", ")
                    )),
                }
            }
            "pack" => {
                if self.pack_steps.iter().any(|s| s == reference) {
                    Ok(format!("pack step {reference}"))
                } else {
                    Err("no such pack step (`dettivo-qa pack list` names them)".into())
                }
            }
            "bench" => {
                if crate::bench::STEPS.contains(&reference) {
                    Ok(format!("bench step {reference}"))
                } else {
                    name_in(reference, crate::bench::meetings::ROWS, "bench step")
                }
            }
            "docs" => {
                if ROOT_DOCS.contains(&reference) {
                    self.existing(reference, &[""])
                } else {
                    self.existing(reference, &["docs/"])
                }
            }
            "script" => {
                let path = self.existing(reference, &["scripts/"])?;
                let full = self.root.join(reference);
                let executable = std::fs::metadata(&full)
                    .map(|m| {
                        use std::os::unix::fs::PermissionsExt;
                        m.permissions().mode() & 0o111 != 0
                    })
                    .unwrap_or(false);
                if executable {
                    Ok(path)
                } else {
                    Err("the script is not executable".into())
                }
            }
            "human" => self.existing(reference, &[""]).map(|found| {
                format!("receipt {found}: a route a person walks, not proof that they did")
            }),
            other => Err(format!("unknown kind {other:?}")),
        }
    }

    fn existing(&self, reference: &str, prefixes: &[&str]) -> Result<String, String> {
        if reference.contains("..") || reference.starts_with('/') {
            return Err("a reference is a path relative to the repository root".into());
        }
        if !prefixes
            .iter()
            .any(|p| reference.starts_with(p) || reference == p.trim_end_matches('/'))
        {
            return Err(format!(
                "the path must be under {}",
                prefixes
                    .iter()
                    .map(|p| if p.is_empty() {
                        "the repository root"
                    } else {
                        p
                    })
                    .collect::<Vec<_>>()
                    .join(" or ")
            ));
        }
        let full = self.root.join(reference);
        if full.is_file() {
            Ok(format!("file {reference}"))
        } else if full.is_dir() {
            let n = std::fs::read_dir(&full).map(|d| d.count()).unwrap_or(0);
            if n == 0 {
                Err(format!("{reference} is an empty directory"))
            } else {
                Ok(format!("directory {reference} ({n} entries)"))
            }
        } else {
            Err(format!("{reference} does not exist"))
        }
    }

    /// `<crate>::<fn>` under `crates/` or `tools/` (a function carrying
    /// `#[test]` or `#[tokio::test]`), or `qt::<name>`.
    fn unit(&self, reference: &str) -> Result<String, String> {
        let Some((scope, name)) = reference.split_once("::") else {
            return Err("a unit ref is `<crate>::<test function>` or `qt::<test name>`".into());
        };
        if name.is_empty()
            || !name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            return Err(format!("{name:?} is not a test name"));
        }
        if scope == "qt" {
            return self.qt_test(name);
        }
        let dir = ["crates", "tools"]
            .iter()
            .map(|d| self.root.join(d).join(scope))
            .find(|d| d.is_dir())
            .ok_or_else(|| format!("no crate named {scope:?} under crates/ or tools/"))?;
        let mut plain = None;
        for file in files(&dir, &["rs"]) {
            let Ok(text) = std::fs::read_to_string(&file) else {
                continue;
            };
            match test_fn(&text, name) {
                Some(true) => {
                    return Ok(format!("fn {name} in {}", relative(&self.root, &file)));
                }
                Some(false) => plain.get_or_insert_with(|| relative(&self.root, &file)),
                None => continue,
            };
        }
        Err(match plain {
            Some(file) => format!("`fn {name}` in {file} is not a test (no #[test] above it)"),
            None => format!("no `fn {name}` under {}", relative(&self.root, &dir)),
        })
    }

    fn qt_test(&self, name: &str) -> Result<String, String> {
        let dir = self.root.join("qt");
        let call = format!("{name}(");
        let ctest = format!("NAME {name}");
        for file in files(&dir, &["qml", "cpp", "h", "txt"]) {
            let Ok(text) = std::fs::read_to_string(&file) else {
                continue;
            };
            let found = if file.extension().is_some_and(|e| e == "txt") {
                text.split_whitespace()
                    .collect::<Vec<_>>()
                    .windows(2)
                    .any(|w| w[0] == "NAME" && w[1] == name)
                    || text.contains(&ctest)
            } else {
                text.match_indices(&call).any(|(i, _)| {
                    let before = text[..i].chars().next_back();
                    !before.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
                })
            };
            if found {
                return Ok(format!("{name} in {}", relative(&self.root, &file)));
            }
        }
        Err(format!("no test named {name:?} under qt/"))
    }
}

/// Whether `text` declares `fn name` as a test: `Some(true)` when the
/// declaration carries `#[test]` or `#[tokio::test]` among the
/// attributes directly above it, `Some(false)` when it is an ordinary
/// function, `None` when the file does not declare it. A declaration
/// inside a comment does not count.
fn test_fn(text: &str, name: &str) -> Option<bool> {
    let lines: Vec<&str> = text.lines().collect();
    let paren = format!("fn {name}(");
    let generic = format!("fn {name}<");
    let mut plain = false;
    for (i, line) in lines.iter().enumerate() {
        let t = line.trim_start();
        if t.starts_with("//") || !(t.contains(&paren) || t.contains(&generic)) {
            continue;
        }
        let attrs = lines[..i]
            .iter()
            .rev()
            .map(|l| l.trim())
            .take_while(|l| l.starts_with("#[") || l.starts_with("///") || l.is_empty());
        if attrs
            .into_iter()
            .any(|l| l == "#[test]" || l.starts_with("#[tokio::test"))
        {
            return Some(true);
        }
        plain = true;
    }
    plain.then_some(false)
}

fn name_in(reference: &str, names: &[&str], what: &str) -> Result<String, String> {
    if names.contains(&reference) {
        Ok(format!("{what} {reference}"))
    } else {
        Err(format!(
            "no {what} named {reference:?}; the names are {}",
            names.join(", ")
        ))
    }
}

/// `[[surface]]` names in the visual manifest, each with its
/// `[[surface.state]]` names.
pub fn surfaces(manifest: &str) -> Vec<(String, Vec<String>)> {
    let mut out: Vec<(String, Vec<String>)> = Vec::new();
    #[derive(PartialEq)]
    enum In {
        Surface,
        State,
        Other,
    }
    let mut at = In::Other;
    for line in manifest.lines() {
        let t = line.trim();
        if t == "[[surface]]" {
            at = In::Surface;
            continue;
        }
        if t == "[[surface.state]]" {
            at = In::State;
            continue;
        }
        if t.starts_with('[') {
            at = In::Other;
            continue;
        }
        let Some(value) = t
            .strip_prefix("name")
            .and_then(|rest| rest.trim_start().strip_prefix('='))
        else {
            continue;
        };
        let value = value.trim().trim_matches('"').to_string();
        match at {
            In::Surface => out.push((value, Vec::new())),
            In::State => {
                if let Some((_, states)) = out.last_mut() {
                    states.push(value);
                }
            }
            In::Other => {}
        }
        at = In::Other;
    }
    out
}

/// `[[surface]]` names in the visual manifest.
pub fn surface_names(manifest: &str) -> Vec<String> {
    surfaces(manifest).into_iter().map(|(s, _)| s).collect()
}

fn files(dir: &Path, extensions: &[&str]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if name != "target" && name != "build" && !name.starts_with('.') {
                    stack.push(path);
                }
            } else if path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| extensions.contains(&e))
            {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_resolves_against_the_repository() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let r = Resolver::new(&root).unwrap();
        assert!(r.resolve("drive", "osd_dictation").is_ok());
        assert!(r.resolve("drive", "settings_roundtrip.models").is_ok());
        assert!(r.resolve("drive", "nope").is_err());
        assert!(r.resolve("visual", "osd").is_ok());
        assert!(
            r.resolve("visual", "nope")
                .unwrap_err()
                .contains("manifest")
        );
        assert!(r.resolve("pack", "dictation/whisper_wer").is_ok());
        assert!(r.resolve("pack", "release/version_match").is_ok());
        assert!(r.resolve("pack", "dictation/nope").is_err());
        assert!(r.resolve("bench", "startup").is_ok());
        assert!(r.resolve("pipeline", "import-merge").is_ok());
        assert!(
            r.resolve("pipeline", "x")
                .unwrap_err()
                .contains("parakeet-alignment")
        );
        assert!(
            r.resolve("contract", "crates/dettivo-proto/fixtures/system")
                .is_ok()
        );
        assert!(
            r.resolve("contract", "docs/qa.md")
                .unwrap_err()
                .contains("must be under")
        );
        assert!(r.resolve("docs", "docs/qa.md").is_ok());
        assert!(r.resolve("docs", "README.md").is_ok());
        assert!(r.resolve("docs", "scripts/run-step.sh").is_err());
        assert!(r.resolve("script", "scripts/run-step.sh").is_ok());
        assert!(r.resolve("script", "scripts/nope.sh").is_err());
        assert!(
            r.resolve(
                "unit",
                "dettivo-qa::every_kind_resolves_against_the_repository"
            )
            .is_ok()
        );
        assert!(
            r.resolve("unit", "dettivo-qa::no_such_test_anywhere")
                .is_err()
        );
        assert!(
            r.resolve("unit", "nocrate::x")
                .unwrap_err()
                .contains("no crate")
        );
        assert!(r.resolve("unit", "bare").is_err());
        assert!(r.resolve("human", "../etc/passwd").is_err());
        assert!(r.resolve("magic", "x").is_err());
    }

    #[test]
    fn surface_names_come_from_the_manifest_tables() {
        let text = "[defaults]\nthreshold = 0.5\n\n[[surface]]\nname = \"osd\"\nbinary = \"x\"\n[[surface.state]]\nname = \"listening\"\n\n[[surface]]\nname = \"panel\"\n";
        assert_eq!(surface_names(text), ["osd", "panel"]);
    }
}
