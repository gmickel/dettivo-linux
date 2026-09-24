//! The per-route checks a pack runs after every scenario step (fn-35
//! R6): each `tree-<route>.json` the step captured goes through the
//! negative text scan with the full class list and the accessibility
//! walk, the walk's result lands beside the tree as `a11y-<route>.json`,
//! and a finding fails the step naming the route, the class and the
//! element. The scenario's own assertions run first; the pack's scan
//! catches a tree the scenario captured without judging it.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::a11y_tree::{self, Coverage};
use crate::driver::Element;
use crate::negative_text;

/// One developer-text finding on a route.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextFinding {
    /// The class (`negative_text::CLASSES`).
    pub class: String,
    /// The element's accessible name.
    pub name: String,
}

/// The checks over one route's tree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RouteScan {
    /// The route, from the tree's file name (`tree-<route>.json`).
    pub route: String,
    /// Developer text on the route.
    pub negative_text: Vec<TextFinding>,
    /// The accessibility walk.
    pub a11y: Coverage,
}

/// The checks over every tree a step captured.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct StepScan {
    /// One entry per route, in file-name order.
    pub routes: Vec<RouteScan>,
}

impl StepScan {
    /// Every route's failure sentence, or `None` when the step is clean.
    pub fn failure(&self) -> Option<String> {
        let mut lines = Vec::new();
        for r in &self.routes {
            if !r.negative_text.is_empty() {
                let list: Vec<String> = r
                    .negative_text
                    .iter()
                    .map(|f| format!("{} in {:?}", f.class, f.name))
                    .collect();
                lines.push(format!(
                    "developer text on the {} route: {}",
                    r.route,
                    list.join("; ")
                ));
            }
            if let Some(why) = r.a11y.failure(&r.route) {
                lines.push(why);
            }
        }
        (!lines.is_empty()).then(|| lines.join("\n"))
    }

    /// Interactive and named elements over every route.
    pub fn totals(&self) -> (usize, usize) {
        self.routes.iter().fold((0, 0), |(i, n), r| {
            (i + r.a11y.interactive, n + r.a11y.named)
        })
    }
}

/// The route a tree file names: `tree-<route>.json`, or `window` for
/// the plain `tree.json` a scenario with one screen writes.
fn route_of(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_str()?;
    if name == "tree.json" {
        return Some("window".into());
    }
    let route = name.strip_prefix("tree-")?.strip_suffix(".json")?;
    (!route.is_empty()).then(|| route.to_string())
}

/// The finding for a tree that cannot be judged: unreadable, or empty.
fn unjudged(route: &str, class: &str, detail: String) -> RouteScan {
    RouteScan {
        route: route.to_string(),
        negative_text: vec![TextFinding {
            class: class.into(),
            name: detail,
        }],
        a11y: a11y_tree::check(&[]),
    }
}

/// Scans one tree.
pub fn scan_tree(route: &str, elements: &[Element]) -> RouteScan {
    RouteScan {
        route: route.to_string(),
        negative_text: negative_text::findings_outside_profile(elements)
            .into_iter()
            .map(|f| TextFinding {
                class: f.class.to_string(),
                name: f.name,
            })
            .collect(),
        a11y: a11y_tree::check(elements),
    }
}

/// Scans every `tree-<route>.json` (and `tree.json`) under `dir` and
/// writes `a11y-<route>.json` beside each; a tree that does not parse
/// and a tree with no element at all are findings of their own, so a
/// truncated or empty capture never passes silently.
pub fn scan_dir(dir: &Path) -> std::io::Result<StepScan> {
    let mut trees: Vec<_> = std::fs::read_dir(dir)?
        .flatten()
        .map(|e| e.path())
        .filter(|p| route_of(p).is_some())
        .collect();
    trees.sort();
    let mut scan = StepScan::default();
    for path in trees {
        let route = route_of(&path).unwrap_or_default();
        let elements: Vec<Element> = match std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|t| serde_json::from_str(&t).map_err(|e| e.to_string()))
        {
            Ok(v) => v,
            Err(e) => {
                scan.routes.push(unjudged(
                    &route,
                    "unreadable tree",
                    format!("{}: {e}", path.display()),
                ));
                continue;
            }
        };
        if elements.is_empty() {
            scan.routes.push(unjudged(
                &route,
                "empty tree",
                format!("{}: no element was captured", path.display()),
            ));
            continue;
        }
        let result = scan_tree(&route, &elements);
        std::fs::write(
            dir.join(format!("a11y-{route}.json")),
            serde_json::to_string_pretty(&result.a11y).map_err(std::io::Error::other)?,
        )?;
        scan.routes.push(result);
    }
    Ok(scan)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn element(index: u32, role: &str, name: &str) -> Element {
        Element {
            index,
            id: format!("/e/{index}"),
            role: role.into(),
            name: name.into(),
            value: None,
            bounds: Some((0, 0, 10, 10)),
            focusable: false,
            focused: false,
            enabled: true,
            parent: None,
            native_id: None,
        }
    }

    fn write_tree(dir: &Path, route: &str, tree: &[Element]) {
        std::fs::write(
            dir.join(format!("tree-{route}.json")),
            serde_json::to_string(tree).unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn a_planted_parity_gap_and_an_unnamed_button_fail_their_routes_by_name() {
        let dir = tempfile::tempdir().unwrap();
        write_tree(
            dir.path(),
            "home",
            &[
                element(0, "heading", "Home"),
                element(1, "push button", "History"),
                element(2, "text", "Search history"),
            ],
        );
        write_tree(
            dir.path(),
            "keys",
            &[
                element(0, "heading", "Keys"),
                element(1, "label", "parity_gap: hotkeys.snippet"),
                element(2, "push button", ""),
            ],
        );
        write_tree(
            dir.path(),
            "profile",
            &[element(
                0,
                "label",
                "/tmp/dqhistory-x/data/dettivo/history.db",
            )],
        );
        std::fs::write(dir.path().join("screenshot-home.png"), b"png").unwrap();
        let scan = scan_dir(dir.path()).unwrap();
        let routes: Vec<&str> = scan.routes.iter().map(|r| r.route.as_str()).collect();
        assert_eq!(routes, ["home", "keys", "profile"]);
        assert!(scan.routes[0].negative_text.is_empty());
        assert!(scan.routes[0].a11y.complete());
        assert_eq!(scan.routes[0].a11y.interactive, 2);
        assert_eq!(scan.routes[1].negative_text[0].class, "internal identifier");
        assert_eq!(scan.routes[1].a11y.offenders.len(), 1);
        assert!(
            scan.routes[2].negative_text.is_empty(),
            "the profile root is set aside"
        );
        let why = scan.failure().unwrap();
        assert!(
            why.contains("developer text on the keys route: internal identifier in \"parity_gap: hotkeys.snippet\""),
            "{why}"
        );
        assert!(
            why.contains("unnamed interactive element on the keys route: push button #2"),
            "{why}"
        );
        assert!(dir.path().join("a11y-keys.json").is_file());
        assert!(dir.path().join("a11y-home.json").is_file());
        assert_eq!(scan.totals(), (3, 2));
        let clean = StepScan {
            routes: vec![scan.routes[0].clone()],
        };
        assert!(clean.failure().is_none());
    }

    #[test]
    fn an_unreadable_or_empty_tree_is_a_finding_and_the_default_tree_is_scanned() {
        let dir = tempfile::tempdir().unwrap();
        assert!(scan_dir(dir.path()).unwrap().routes.is_empty());
        std::fs::write(dir.path().join("tree-broken.json"), "[{").unwrap();
        std::fs::write(dir.path().join("tree-blank.json"), "[]").unwrap();
        std::fs::write(
            dir.path().join("tree.json"),
            serde_json::to_string(&[element(0, "push button", "")]).unwrap(),
        )
        .unwrap();
        let scan = scan_dir(dir.path()).unwrap();
        let routes: Vec<(&str, &str)> = scan
            .routes
            .iter()
            .map(|r| {
                (
                    r.route.as_str(),
                    r.negative_text
                        .first()
                        .map(|f| f.class.as_str())
                        .unwrap_or(""),
                )
            })
            .collect();
        assert_eq!(
            routes,
            [
                ("blank", "empty tree"),
                ("broken", "unreadable tree"),
                ("window", "")
            ]
        );
        let why = scan.failure().unwrap();
        assert!(
            why.contains("broken route") && why.contains("blank route"),
            "{why}"
        );
        assert!(
            why.contains("unnamed interactive element on the window route"),
            "{why}"
        );
        assert!(dir.path().join("a11y-window.json").is_file());
    }
}
