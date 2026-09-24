//! `lint-scenarios` (R2): a scenario file may only use the driver
//! interface. Any mention of a driver implementation fails the lint with
//! the file and line named.

use std::path::Path;

/// Tokens a scenario may not contain.
pub const FORBIDDEN: &[&str] = &[
    "CuaDriver",
    "AtspiDriver",
    "cua-driver",
    "atspi::",
    "x11rb",
    "driver::cua",
    "driver::atspi",
];

/// Lints every `.rs` under `scenarios_dir` except `mod.rs`.
pub fn run(scenarios_dir: &Path) -> Result<usize, Vec<String>> {
    let mut findings = Vec::new();
    let mut checked = 0;
    let mut files: Vec<_> = std::fs::read_dir(scenarios_dir)
        .map_err(|e| vec![format!("{}: {e}", scenarios_dir.display())])?
        .map(|entry| entry.map(|e| e.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| vec![format!("{}: {e}", scenarios_dir.display())])?
        .into_iter()
        .filter(|p| {
            p.extension().is_some_and(|x| x == "rs") && p.file_name().is_some_and(|n| n != "mod.rs")
        })
        .collect();
    files.sort();
    for path in files {
        checked += 1;
        let text =
            std::fs::read_to_string(&path).map_err(|e| vec![format!("{}: {e}", path.display())])?;
        for (n, line) in text.lines().enumerate() {
            for token in FORBIDDEN {
                if line.contains(token) {
                    findings.push(format!(
                        "{}:{}: scenario names a driver implementation ({token}); use the Driver interface",
                        path.display(),
                        n + 1
                    ));
                }
            }
        }
    }
    if findings.is_empty() {
        Ok(checked)
    } else {
        Err(findings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shipped_pack_is_clean_and_a_driver_mention_is_caught() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/scenarios");
        assert!(run(&dir).unwrap() >= 1);
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("bad.rs"),
            "use crate::driver::cua::CuaDriver;\n",
        )
        .unwrap();
        let err = run(tmp.path()).unwrap_err();
        assert!(err[0].contains("bad.rs:1"), "{err:?}");
    }
}
