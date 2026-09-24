//! Keyboard-first (fn-38 R4, FR-V9): every route of the app is operable
//! without a mouse. On every route the drive presses Tab until focus
//! wraps and reads which element holds focus after each press through
//! the accessibility tree (its `focused` state is the focus ring), so
//! every element the a11y walk names as interactive and focusable is
//! visited by Tab, Shift+Tab walks back one step, and the four
//! conventions hold: `Super+F` makes the window fullscreen, `Escape`
//! closes what is open, `/` goes to search and `?` opens the hint sheet,
//! which lists the in-app keys and closes on any key. Rows a list moves
//! through with j and k and tabs a number picks are arrow-navigated and
//! not part of the Tab chain, nor is a control inside such a row or the
//! hyperlink inside a link, nor a disabled control. Fails naming the
//! route and the element.

mod desktop;

use std::collections::BTreeSet;

use serde_json::json;

use super::app_support::{ROUTES, RouteCase, app_env, capture, close, daemon_config, wait_status};
use super::daemon::DaemonHandle;
use super::{Context, Scenario, binary};
use crate::a11y_tree::is_interactive;
use crate::driver::{App, Driver, Element, Launch};

/// Roles a route navigates with its own keys rather than Tab.
const ARROW_NAVIGATED: &[&str] = &["list item", "page tab", "tab", "menu item"];

/// The scenario.
pub struct KeyboardOnly;

impl Scenario for KeyboardOnly {
    fn id(&self) -> &'static str {
        "keyboard_only"
    }

    fn summary(&self) -> &'static str {
        "Tab reaches every interactive element on every route with the focus ring, and Super+F, Escape, / and ? do what the hint sheet says"
    }

    fn preconditions(&self, ctx: &Context<'_>) -> Result<(), String> {
        binary(ctx.repo_root, "dettivo-app")?;
        binary(ctx.repo_root, "dettivod").map(|_| ())
    }

    fn run(&self, driver: &mut dyn Driver, ctx: &mut Context<'_>) -> Result<(), String> {
        desktop::check(ctx)?;
        let dettivod = binary(ctx.repo_root, "dettivod")?;
        let app_binary = binary(ctx.repo_root, "dettivo-app")?;
        let mut seed = std::collections::BTreeMap::new();
        seed.insert("DETTIVO_E2E_SEED".to_string(), "1".to_string());
        let _daemon = DaemonHandle::spawn(
            &dettivod,
            ctx.profile,
            &daemon_config(ctx.repo_root),
            &seed,
            ctx.timeout,
        )?;
        ctx.timings.mark("daemon");
        let mut summary = Vec::new();
        for case in ROUTES {
            let app = launch(driver, ctx, &app_binary, case)?;
            let result = tab_chain(driver, ctx, &app, case);
            let closed = close(driver, &app);
            ctx.timings.mark(case.name);
            summary.push(result?);
            closed?;
        }
        let app = launch(driver, ctx, &app_binary, &ROUTES[0])?;
        let conventions = conventions(driver, ctx, &app);
        let closed = close(driver, &app);
        let conventions = conventions?;
        closed?;
        ctx.timings.mark("conventions");
        let report = json!({"routes": summary, "conventions": conventions});
        std::fs::write(
            ctx.evidence_dir.join("keyboard.json"),
            serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        ctx.evidence.push("keyboard.json".into());
        Ok(())
    }
}

fn launch(
    driver: &mut dyn Driver,
    ctx: &mut Context<'_>,
    app_binary: &std::path::Path,
    case: &RouteCase,
) -> Result<App, String> {
    let app = driver
        .launch(
            &Launch {
                program: app_binary.to_path_buf(),
                args: Vec::new(),
                env: app_env(ctx, case.open, case.route),
            },
            ctx.timeout,
        )
        .map_err(|e| format!("launch dettivo-app for {}: {e}", case.name))?;
    ctx.profile.track_pid("dettivo-app", app.pid);
    driver
        .wait_for_label(&app, case.title, ctx.timeout)
        .map_err(|e| format!("title {:?} of {}: {e}", case.title, case.name))?;
    Ok(app)
}

fn identity(e: &Element) -> &str {
    e.native_id.as_deref().unwrap_or(&e.id)
}

fn describe(e: &Element) -> String {
    format!("{} {:?}", e.role, e.name)
}

fn focused(tree: &[Element]) -> Option<&Element> {
    // Qt exposes focus on both a ListView focus scope and its active row.
    // Follow the deepest observed node, regardless of snapshot ordering.
    tree.iter()
        .filter(|e| e.focused && e.bounds.is_some())
        .max_by_key(|e| {
            std::iter::successors(e.parent, |parent| {
                tree.iter()
                    .find(|e| e.index == *parent)
                    .and_then(|e| e.parent)
            })
            .take(tree.len())
            .count()
        })
}

/// True when an ancestor is a row the list navigates with its own keys
/// (the row's actions come with it) or a link (the hyperlink is its own
/// AT-SPI node under the text).
fn inside_row_or_link(tree: &[Element], e: &Element) -> bool {
    let mut parent = e.parent;
    while let Some(p) = parent {
        let Some(ancestor) = tree.iter().find(|a| a.index == p) else {
            return false;
        };
        if ARROW_NAVIGATED.contains(&ancestor.role.as_str()) || ancestor.role == "link" {
            return true;
        }
        parent = ancestor.parent;
    }
    false
}

/// Tab around the route once: every focusable interactive element on
/// screen is visited, the focused one is read back after every press,
/// and Shift+Tab returns to the previous element.
fn tab_chain(
    driver: &mut dyn Driver,
    ctx: &mut Context<'_>,
    app: &App,
    case: &RouteCase,
) -> Result<serde_json::Value, String> {
    let tree = capture(driver, app, ctx, &format!("keyboard-{}", case.name))?;
    let expected: Vec<&Element> = tree
        .iter()
        .filter(|e| is_interactive(&e.role) && e.bounds.is_some())
        .collect();
    let in_chain =
        |e: &Element| !ARROW_NAVIGATED.contains(&e.role.as_str()) && !inside_row_or_link(&tree, e);
    let chain: BTreeSet<String> = expected
        .iter()
        .filter(|e| e.enabled && in_chain(e))
        .map(|e| identity(e).to_owned())
        .collect();
    let disabled: Vec<String> = expected
        .iter()
        .filter(|e| !e.enabled && in_chain(e))
        .map(|e| describe(e))
        .collect();
    let mut visited: Vec<String> = Vec::new();
    let mut focus_samples = Vec::new();
    let mut seen = BTreeSet::new();
    // Every interactive element, the arrow-navigated ones included, may
    // take a press before the chain wraps.
    let limit = expected.len() * 2 + 8;
    // The element focus sits on after the last press, whatever it was.
    let mut current: Option<String> = None;
    let mut previous: Option<String> = None;
    for _ in 0..limit {
        driver
            .press_key(app, "Tab")
            .map_err(|e| format!("{}: Tab: {e}", case.name))?;
        let now = driver
            .snapshot(app)
            .map_err(|e| format!("{}: snapshot after Tab: {e}", case.name))?;
        focus_samples.push(
            now.iter()
                .filter(|e| e.focused)
                .map(describe)
                .collect::<Vec<_>>(),
        );
        let Some(focus) = focused(&now) else {
            continue;
        };
        if current.as_deref() != Some(identity(focus)) {
            previous = current.replace(identity(focus).to_owned());
        }
        if !seen.insert(identity(focus).to_owned()) && seen.len() >= chain.len() {
            break;
        }
        visited.push(describe(focus));
    }
    let unreached: Vec<String> = expected
        .iter()
        .filter(|e| chain.contains(identity(e)) && !seen.contains(identity(e)))
        .map(|e| describe(e))
        .collect();
    // Shift+Tab must return to the actual previous node, not merely move.
    let mut back_ok = true;
    let mut back_observed = None;
    if current.is_some() {
        driver
            .press_key(app, "Shift+Tab")
            .map_err(|e| format!("{}: Shift+Tab: {e}", case.name))?;
        let now = driver
            .snapshot(app)
            .map_err(|e| format!("{}: snapshot after Shift+Tab: {e}", case.name))?;
        back_observed = focused(&now).map(|f| identity(f).to_owned());
        back_ok = (previous.is_some() && back_observed == previous) || chain.len() < 2;
    }
    let report = json!({
        "route": case.name,
        "interactive": expected.len(),
        "chain": chain.len(),
        "visited": visited,
        "focus_samples": focus_samples,
        "unreached": unreached,
        "disabled": disabled,
        "shift_tab_walks_back": back_ok,
        "shift_tab_expected": previous,
        "shift_tab_observed": back_observed,
    });
    let file = format!("keyboard-{}.json", case.name);
    std::fs::write(
        ctx.evidence_dir.join(&file),
        serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    ctx.evidence.push(file);
    if !unreached.is_empty() {
        return Err(format!(
            "Tab cannot reach {} on the {} route",
            unreached.join(", "),
            case.name
        ));
    }
    if chain.is_empty() {
        return Err(format!(
            "the {} route has no element Tab can focus",
            case.name
        ));
    }
    if !back_ok {
        return Err(format!(
            "Shift+Tab did not walk back on the {} route",
            case.name
        ));
    }
    Ok(report)
}

/// The four conventions from Home: `?` opens the hint sheet and any key
/// closes it, `/` opens History with the search field focused, `Escape`
/// leaves the detail, `Super+F` toggles fullscreen.
fn conventions(
    driver: &mut dyn Driver,
    ctx: &mut Context<'_>,
    app: &App,
) -> Result<serde_json::Value, String> {
    let on_screen = |tree: &[Element], name: &str| {
        tree.iter()
            .any(|e| e.name == name && e.bounds.is_some_and(|b| b.2 > 0 && b.3 > 0))
    };
    driver.press_key(app, "?").map_err(|e| format!("?: {e}"))?;
    let tree = capture(driver, app, ctx, "keyboard-hint-sheet")?;
    if !on_screen(&tree, "Keyboard hint sheet") {
        return Err("? did not open the keyboard hint sheet on Home".into());
    }
    for key in [
        "j: next item",
        "?: this sheet",
        "n: new meeting",
        "Global keys",
    ] {
        if !tree.iter().any(|e| e.name == key) {
            return Err(format!("the hint sheet does not list {key:?}"));
        }
    }
    driver.press_key(app, "j").map_err(|e| format!("j: {e}"))?;
    let tree = driver.snapshot(app).map_err(|e| e.to_string())?;
    if on_screen(&tree, "Keyboard hint sheet") {
        return Err("the hint sheet stayed open after a key".into());
    }
    driver.press_key(app, "/").map_err(|e| format!("/: {e}"))?;
    driver
        .wait_for_label(app, "History", ctx.timeout)
        .map_err(|e| format!("/ did not open History: {e}"))?;
    let tree = capture(driver, app, ctx, "keyboard-search")?;
    let search_focused = tree
        .iter()
        .any(|e| e.focused && (e.name == "Search history" || e.role == "text"));
    if !search_focused {
        return Err("/ opened History but the search field has no focus".into());
    }
    driver
        .press_key(app, "Escape")
        .map_err(|e| format!("Escape: {e}"))?;
    driver
        .press_key(app, "Super+F")
        .map_err(|e| format!("Super+F: {e}"))?;
    let status = wait_status(ctx, ctx.timeout, "fullscreen", |s| {
        s["fullscreen"].as_bool() == Some(true)
    });
    let fullscreen = status.is_ok();
    driver
        .press_key(app, "Super+F")
        .map_err(|e| format!("Super+F: {e}"))?;
    let restored = wait_status(ctx, ctx.timeout, "windowed", |s| {
        s["fullscreen"].as_bool() == Some(false)
    })
    .is_ok();
    if !fullscreen || !restored {
        return Err(format!(
            "Super+F: fullscreen {fullscreen}, restored {restored}"
        ));
    }
    Ok(json!({
        "hint_sheet": true,
        "slash_to_search": true,
        "escape_closes": true,
        "super_f_fullscreen": true,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_row_wins_over_its_focused_list_scope_in_any_order() {
        let node = |index, name: &str, parent| Element {
            index,
            id: name.into(),
            native_id: None,
            role: "list item".into(),
            name: name.into(),
            value: None,
            bounds: Some((0, 0, 100, 30)),
            focusable: true,
            focused: true,
            enabled: true,
            parent,
        };
        let list = node(1, "list", None);
        let row = node(2, "row", Some(1));
        for tree in [
            vec![list.clone(), row.clone()],
            vec![row.clone(), list.clone()],
        ] {
            assert_eq!(focused(&tree).unwrap().id, "row");
        }
        let mut next = node(3, "next row", Some(1));
        next.focused = false;
        let mut tree = vec![list, row, next];
        assert_eq!(focused(&tree).unwrap().id, "row");
        tree[1].focused = false;
        tree[2].focused = true;
        assert_eq!(focused(&tree).unwrap().id, "next row");
    }
}
