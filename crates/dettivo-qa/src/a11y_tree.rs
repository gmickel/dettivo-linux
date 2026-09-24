//! The accessibility walk (fn-35 R6, NFR-9): every interactive element on
//! a route carries a role and a non-empty accessible name, so a drive on
//! either driver and a screen reader find it by name. The walk runs over
//! the `tree-<route>.json` a step captured; static text and images are
//! exempt, and a control the driver reports at zero size (a hidden
//! action, a collapsed row's button) is not interactive to anyone. A
//! control whose geometry the driver could not read stays in the count:
//! unknown geometry is not proof that it is hidden. The result is one
//! `a11y-<route>.json` and a coverage figure the pack report carries per
//! surface.

use serde::{Deserialize, Serialize};

use crate::driver::Element;

/// The AT-SPI roles a user acts on. A role Qt exposes for a control the
/// design system draws (`toggle button` for a switch, `page tab` for a
/// segment, `list item` for a row a click opens) counts as interactive.
pub const INTERACTIVE_ROLES: &[&str] = &[
    "push button",
    "button",
    "toggle button",
    "check box",
    "radio button",
    "text",
    "password text",
    "editable text",
    "combo box",
    "slider",
    "spin button",
    "page tab",
    "tab",
    "list item",
    "menu item",
    "link",
];

/// One interactive element without a name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Offender {
    /// The element's role.
    pub role: String,
    /// The driver's stable id (an AT-SPI path, a cua-driver id).
    pub id: String,
    /// The element's index in the walk, so a person finds it in the tree.
    pub index: u32,
    /// Its bounds, when known.
    pub bounds: Option<(i32, i32, i32, i32)>,
}

/// What the walk found on one route.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Coverage {
    /// Interactive elements on screen.
    pub interactive: usize,
    /// Of those, the ones with a non-empty name.
    pub named: usize,
    /// `named / interactive`, `1.0` for a route with no control.
    pub coverage: f64,
    /// Every interactive element without a name.
    pub offenders: Vec<Offender>,
}

impl Coverage {
    /// True when every interactive element carries a name.
    pub fn complete(&self) -> bool {
        self.offenders.is_empty()
    }

    /// The failure sentence, naming every offender by role, index and
    /// bounds; `None` when the walk is clean.
    pub fn failure(&self, route: &str) -> Option<String> {
        if self.complete() {
            return None;
        }
        let list: Vec<String> = self
            .offenders
            .iter()
            .map(|o| match o.bounds {
                Some((x, y, w, h)) => format!("{} #{} at {x},{y} {w}x{h}", o.role, o.index),
                None => format!("{} #{}", o.role, o.index),
            })
            .collect();
        Some(format!(
            "unnamed interactive element on the {route} route: {}",
            list.join("; ")
        ))
    }
}

/// True for a role a user acts on.
pub fn is_interactive(role: &str) -> bool {
    INTERACTIVE_ROLES.contains(&role)
}

/// True for an element the driver reports at zero size: hidden, so no
/// one can act on it. Unknown geometry (`None`) is not hidden.
pub fn is_hidden(bounds: Option<(i32, i32, i32, i32)>) -> bool {
    matches!(bounds, Some((_, _, w, h)) if w <= 0 || h <= 0)
}

/// Walks one tree.
pub fn check(elements: &[Element]) -> Coverage {
    let mut interactive = 0;
    let mut named = 0;
    let mut offenders = Vec::new();
    for e in elements {
        if !is_interactive(&e.role) || is_hidden(e.bounds) {
            continue;
        }
        interactive += 1;
        if e.name.trim().is_empty() {
            offenders.push(Offender {
                role: e.role.clone(),
                id: e.id.clone(),
                index: e.index,
                bounds: e.bounds,
            });
        } else {
            named += 1;
        }
    }
    let coverage = if interactive == 0 {
        1.0
    } else {
        named as f64 / interactive as f64
    };
    Coverage {
        interactive,
        named,
        coverage,
        offenders,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn element(
        index: u32,
        role: &str,
        name: &str,
        bounds: Option<(i32, i32, i32, i32)>,
    ) -> Element {
        Element {
            index,
            id: format!("/a/{index}"),
            role: role.into(),
            name: name.into(),
            value: None,
            bounds,
            focusable: false,
            focused: false,
            enabled: true,
            parent: None,
            native_id: None,
        }
    }

    const ON: Option<(i32, i32, i32, i32)> = Some((10, 20, 100, 30));

    #[test]
    fn a_planted_unnamed_button_fails_by_name_and_hidden_controls_do_not_count() {
        let tree = vec![
            element(0, "label", "", ON),
            element(1, "graphic", "", ON),
            element(2, "push button", "Continue", ON),
            element(3, "text", "Try it field", ON),
            element(4, "push button", "", Some((0, 0, 0, 0))),
            element(5, "push button", "  ", ON),
            element(6, "list item", "Row", ON),
        ];
        let c = check(&tree);
        assert_eq!((c.interactive, c.named), (4, 3));
        assert_eq!(c.offenders.len(), 1);
        assert_eq!(c.offenders[0].index, 5);
        assert!(!c.complete());
        let why = c.failure("keys").unwrap();
        assert!(
            why.contains("keys route") && why.contains("push button #5 at 10,20 100x30"),
            "{why}"
        );
        let clean = check(&tree[..4]);
        assert!(clean.complete());
        assert_eq!(clean.coverage, 1.0);
        assert!(clean.failure("keys").is_none());
        let empty = check(&[element(0, "label", "Ready.", ON)]);
        assert_eq!((empty.interactive, empty.coverage), (0, 1.0));
    }

    #[test]
    fn a_control_whose_geometry_failed_to_read_still_has_to_carry_a_name() {
        // The driver could not read the extents: `None`. That is not a
        // hidden control, so an unnamed one is an offender and a named
        // one counts.
        let tree = vec![
            element(0, "push button", "", None),
            element(1, "push button", "Continue", None),
            element(2, "check box", "Enabled", Some((5, 5, 0, 12))),
        ];
        let c = check(&tree);
        assert_eq!((c.interactive, c.named), (2, 1));
        assert_eq!(c.offenders.len(), 1);
        assert_eq!(c.offenders[0].index, 0);
        assert!(c.failure("home").unwrap().contains("push button #0"));
        assert!(is_hidden(Some((0, 0, 0, 0))) && !is_hidden(None) && !is_hidden(ON));
    }
}
