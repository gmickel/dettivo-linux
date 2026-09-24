//! The seed as the History list shows it (fn-41 R2): the row count comes
//! from the seeded daemon's `transcripts.list` with the kinds the list
//! requests, never from a literal, and the wait knows the fold: the
//! list keeps only the delegates near the screen, so a seed longer than
//! the view shows fewer rows with the last one past the list's bottom
//! edge.

use std::time::{Duration, Instant};

use serde_json::json;

use super::Context;
use super::daemon::DaemonHandle;
use crate::driver::{App, Driver, DriverError, Element};

/// The kinds the History list asks for (`qt/host/app/history_model.cpp`).
const LISTED_KINDS: [&str; 2] = ["dictation", "meeting"];

/// The seed as the list shows it: the row titles in list order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seed {
    /// The titles, newest first, as `transcripts.list` answers them.
    pub titles: Vec<String>,
}

impl Seed {
    /// The row count.
    pub fn len(&self) -> usize {
        self.titles.len()
    }

    /// True when the seed lists nothing.
    pub fn is_empty(&self) -> bool {
        self.titles.is_empty()
    }

    /// True when the rows on screen (after `skip` rows that are not the
    /// seed's, the re-run's) are the seed's leading rows in order; a row
    /// shows its title whole or elided to a prefix.
    pub fn leads(&self, rows: &[&Element], skip: usize) -> bool {
        rows.iter().skip(skip).enumerate().all(|(i, row)| {
            let shown = row.name.trim_end_matches('…').trim_end();
            self.titles
                .get(i)
                .is_some_and(|title| !shown.is_empty() && title.starts_with(shown))
        })
    }
}

/// The rows the seed puts in the list, read from the seeded daemon with
/// the request the list itself makes, so a seed that grows (fn-33 and
/// fn-34 added meetings) moves the expectation with it.
pub fn seeded_rows(daemon: &DaemonHandle) -> Result<Seed, String> {
    let listed = daemon.call(
        "transcripts.list",
        json!({"kinds": LISTED_KINDS, "limit": 50, "cursor": null}),
    )?;
    let items = listed["items"]
        .as_array()
        .ok_or("transcripts.list over the seed answered no items")?;
    if items.is_empty() {
        return Err(
            "transcripts.list over the seed answered no rows; is DETTIVO_E2E_SEED on?".into(),
        );
    }
    Ok(Seed {
        titles: items
            .iter()
            .map(|i| i["title"].as_str().unwrap_or_default().to_string())
            .collect(),
    })
}

/// The rows a search for `query` finds over the seed, with the request
/// the search field makes; the hit count the field shows follows it.
pub fn seeded_hits(daemon: &DaemonHandle, query: &str) -> Result<usize, String> {
    let found = daemon.call(
        "transcripts.search",
        json!({"query": query, "kinds": LISTED_KINDS, "limit": 50}),
    )?;
    let items = found["items"]
        .as_array()
        .ok_or("transcripts.search over the seed answered no items")?;
    if items.is_empty() {
        return Err(format!(
            "transcripts.search over the seed finds nothing for {query:?}"
        ));
    }
    Ok(items.len())
}

/// Focuses the search field, types `query` and submits it once the field
/// carries the whole query: the first keystrokes after a click can land
/// before the field has focus, so the value is read back (where the
/// driver exposes it) and a short query is cleared and typed again, three
/// attempts at most. Unreadable values fail verification without submitting.
pub fn type_query(
    driver: &mut dyn Driver,
    app: &App,
    ctx: &mut Context<'_>,
    query: &str,
) -> Result<(), String> {
    let mut read = String::new();
    for attempt in 0..3 {
        if attempt > 0 && !read.is_empty() {
            if let Ok(clear) = driver.wait_for_label(app, "Clear search", Duration::from_secs(2)) {
                driver
                    .click(app, &clear)
                    .map_err(|e| format!("clear the search field: {e}"))?;
            }
        }
        let field = driver
            .wait_for_label(app, "Search history", ctx.timeout)
            .map_err(|e| format!("the search field: {e}"))?;
        driver
            .click(app, &field)
            .map_err(|e| format!("focus the search field: {e}"))?;
        driver
            .type_text(app, query)
            .map_err(|e| format!("type the query: {e}"))?;
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            match driver.read_value(app, &field) {
                Ok(value) if value != field.name => read = value,
                observation => {
                    if Instant::now() >= deadline {
                        return Err(format!(
                            "cannot verify the search field value: {observation:?}"
                        ));
                    }
                    std::thread::sleep(Duration::from_millis(100));
                    continue;
                }
            }
            if read == query || Instant::now() >= deadline {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        if read == query {
            return driver
                .type_text(app, "\n")
                .map_err(|e| format!("submit the query: {e}"));
        }
    }
    Err(format!(
        "the search field carries {read:?} after three attempts to type {query:?}"
    ))
}

/// The rows of the list on screen.
pub fn rows(tree: &[Element]) -> Vec<&Element> {
    tree.iter().filter(|e| e.role == "list item").collect()
}

/// Whether the list is full past its fold: `Some(true)` when its last
/// row on screen ends below the list's own bottom edge, so more rows
/// sit under it; `None` when the driver gives the list no bounds
/// (a driver that bounds only what it can act on), so the fold is judged
/// by the rows alone.
pub fn is_full(tree: &[Element]) -> Option<bool> {
    let (_, list_y, _, list_h) = tree
        .iter()
        .find(|e| e.role == "list")
        .and_then(|e| e.bounds)?;
    Some(
        rows(tree)
            .last()
            .and_then(|row| row.bounds)
            .is_some_and(|(_, y, _, h)| y + h > list_y + list_h),
    )
}

/// The first row on screen.
pub fn top_row(tree: &[Element]) -> Option<Element> {
    rows(tree).first().map(|e| (*e).clone())
}

/// True when a row joined the list above `before`: the row that was
/// first is no longer first, or sits lower than it did.
fn top_moved(tree: &[Element], before: &Element) -> bool {
    let Some(top) = top_row(tree) else {
        return false;
    };
    if top.id != before.id {
        return true;
    }
    match (top.bounds, before.bounds) {
        (Some((_, y, _, _)), Some((_, was, _, _))) => y > was,
        _ => false,
    }
}

/// Whether a snapshot shows the seed plus `extra` rows: the rows on
/// screen are the seed's leading rows in order (after the extra row, which
/// pushed the old top row down), and either every row is on screen or the
/// list is full past its fold, judged from the list's own bounds. A driver
/// that gives the list no bounds cannot prove the fold, so fewer rows
/// than the seed never pass on it.
pub fn shows(tree: &[Element], seed: &Seed, extra: Option<&Element>) -> bool {
    let rows = rows(tree);
    let expected = seed.len() + usize::from(extra.is_some());
    let leading = seed.leads(&rows, usize::from(extra.is_some()));
    let joined = extra.is_none_or(|before| top_moved(tree, before));
    let whole = rows.len() == expected;
    let folded = !rows.is_empty() && rows.len() < expected && is_full(tree) == Some(true);
    leading && joined && (whole || folded)
}

/// Waits until the list `shows` the seed plus `extra` rows. A mismatch at
/// the timeout names the count shown and the count the seed answered, and
/// files the tree beside the evidence.
pub fn wait(
    driver: &mut dyn Driver,
    app: &App,
    ctx: &mut Context<'_>,
    seed: &Seed,
    extra: Option<&Element>,
) -> Result<Vec<Element>, String> {
    let seeded = seed.len();
    let expected = seeded + usize::from(extra.is_some());
    let timeout = ctx.timeout;
    let deadline = Instant::now() + timeout;
    loop {
        let tree = match driver.snapshot(app) {
            Ok(tree) => tree,
            Err(DriverError::NotFound(_)) => Vec::new(),
            Err(e) => return Err(format!("snapshot: {e}")),
        };
        if shows(&tree, seed, extra) {
            return Ok(tree);
        }
        if Instant::now() > deadline {
            let shown = rows(&tree).len();
            let leading = seed.leads(&rows(&tree), usize::from(extra.is_some()));
            let file = format!("tree-rows-{expected}.json");
            if let Ok(text) = serde_json::to_string_pretty(&tree) {
                let _ = std::fs::write(ctx.evidence_dir.join(&file), text);
                ctx.evidence.push(file);
            }
            let names: Vec<String> = rows(&tree)
                .iter()
                .map(|e| format!("{:?}", e.name))
                .collect();
            return Err(format!(
                "the list shows {shown} rows after {timeout:?}, expected {expected} (the seed's transcripts.list over the profile answered {seeded} rows{}; the list is {}): {}",
                if extra.is_some() {
                    " plus one re-run"
                } else {
                    ""
                },
                match (is_full(&tree), leading) {
                    (Some(true), true) => "full past its fold",
                    (Some(true), false) => "full past its fold but not the seed's leading rows",
                    (Some(false), true) => "not full",
                    (Some(false), false) => "not full and not the seed's leading rows",
                    (None, true) => "unbounded, so the fold cannot be proven",
                    (None, false) => "unbounded and not the seed's leading rows",
                },
                names.join(", ")
            ));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn element(role: &str, name: &str, id: &str, bounds: (i32, i32, i32, i32)) -> Element {
        Element {
            index: 0,
            id: id.into(),
            role: role.into(),
            name: name.into(),
            value: None,
            bounds: Some(bounds),
            focusable: true,
            focused: false,
            enabled: true,
            parent: None,
            native_id: None,
        }
    }

    #[test]
    fn a_list_is_full_when_its_last_row_ends_past_its_bottom_edge() {
        let list = element("list", "Dictations", "l", (360, 208, 400, 670));
        let a = element("list item", "Dictation", "a", (360, 236, 400, 60));
        let b = element("list item", "Design call", "b", (360, 1188, 400, 60));
        assert_eq!(is_full(&[list.clone(), a.clone(), b.clone()]), Some(true));
        assert_eq!(is_full(&[list.clone(), a.clone()]), Some(false));
        assert_eq!(is_full(&[a.clone(), b.clone()]), None, "no list bounds");
        assert_eq!(rows(&[list.clone(), a.clone(), b.clone()]).len(), 2);

        // The rows on screen are the seed's leading rows, elided or whole.
        let seed = Seed {
            titles: vec![
                "Dictation".into(),
                "Design call".into(),
                "Roadmap review".into(),
            ],
        };
        let elided = element("list item", "Design ca…", "b", (360, 296, 400, 60));
        assert!(seed.leads(&[&a, &b], 0));
        assert!(seed.leads(&[&a, &elided], 0));
        assert!(!seed.leads(&[&b, &a], 0), "out of order");
        let rerun = element("list item", "Re-run", "z", (360, 236, 400, 60));
        assert!(seed.leads(&[&rerun, &a, &b], 1));
        assert!(!seed.leads(&[&rerun, &a, &b], 0));
        assert_eq!(seed.len(), 3);
        assert!(!seed.is_empty());

        // A re-run row on top pushes the old first row down, or replaces it.
        let pushed = element("list item", "Dictation", "a", (360, 324, 400, 60));
        assert!(top_moved(&[list.clone(), pushed], &a));
        let other = element("list item", "Re-run", "z", (360, 236, 400, 60));
        assert!(top_moved(&[list.clone(), other, a.clone()], &a));
        assert!(!top_moved(&[list, a.clone()], &a));
    }

    #[test]
    fn the_rows_shown_are_judged_by_identity_and_order_in_every_branch() {
        let seed = Seed {
            titles: vec!["Dictation".into(), "Design call".into()],
        };
        let list = element("list", "Dictations", "l", (360, 208, 400, 670));
        let a = element("list item", "Dictation", "a", (360, 236, 400, 60));
        let b = element("list item", "Design call", "b", (360, 296, 400, 60));
        let wrong = element("list item", "Roadmap review", "c", (360, 296, 400, 60));
        assert!(shows(&[list.clone(), a.clone(), b.clone()], &seed, None));
        // The right count with a wrong row, or the seed's rows reordered.
        assert!(!shows(&[list.clone(), a.clone(), wrong], &seed, None));
        assert!(!shows(&[list.clone(), b.clone(), a.clone()], &seed, None));
        // Fewer rows pass only when the list is provably full past its fold.
        let low = element("list item", "Dictation", "a", (360, 1188, 400, 60));
        assert!(shows(&[list.clone(), low.clone()], &seed, None));
        assert!(!shows(&[list.clone(), a.clone()], &seed, None), "not full");
        assert!(!shows(&[low], &seed, None), "no list bounds");
        // A re-run on top must be joined by the pushed-down seed rows.
        let rerun = element("list item", "Re-run", "z", (360, 236, 400, 60));
        let pushed_a = element("list item", "Dictation", "a", (360, 296, 400, 60));
        let pushed_b = element("list item", "Design call", "b", (360, 356, 400, 60));
        assert!(shows(
            &[
                list.clone(),
                rerun.clone(),
                pushed_a.clone(),
                pushed_b.clone()
            ],
            &seed,
            Some(&a)
        ));
        assert!(!shows(
            &[list.clone(), rerun.clone(), pushed_b, pushed_a],
            &seed,
            Some(&a)
        ));
        assert!(
            !shows(&[list, a.clone(), b, rerun], &seed, Some(&a)),
            "the old top did not move"
        );
    }
}
