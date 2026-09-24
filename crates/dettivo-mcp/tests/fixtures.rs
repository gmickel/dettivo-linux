//! The checked-in fixtures under `fixtures/tools/` are the nineteen tool
//! definitions as `tools/list` reports them, one file per tool, so a
//! change to a name, a description or a schema shows up as a fixture
//! diff. `FN18_WRITE_FIXTURES=1` rewrites them from the catalog.

use std::path::Path;

use dettivo_mcp::tools;
use serde_json::Value;

fn root() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/tools"))
}

#[test]
fn every_tool_definition_matches_its_fixture() {
    let write = std::env::var_os("FN18_WRITE_FIXTURES").is_some();
    let catalog = tools::catalog();
    let mut expected_files: Vec<String> = Vec::new();
    for tool in &catalog {
        let path = root().join(format!("{}.json", tool.name));
        expected_files.push(format!("{}.json", tool.name));
        let definition = tool.definition();
        if write {
            std::fs::create_dir_all(root()).unwrap();
            std::fs::write(
                &path,
                format!("{}\n", serde_json::to_string_pretty(&definition).unwrap()),
            )
            .unwrap();
        }
        let text =
            std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let on_disk: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(on_disk, definition, "{}", path.display());
    }
    let mut on_disk: Vec<String> = std::fs::read_dir(root())
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    on_disk.sort();
    expected_files.sort();
    assert_eq!(on_disk, expected_files, "a fixture without a tool");
}
