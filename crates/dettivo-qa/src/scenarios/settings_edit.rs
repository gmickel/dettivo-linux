//! The edit of one settings key through its control (fn-26 R1): a
//! field is typed into, a switch clicked, a segment or a combo box
//! picked, and the file is waited on to carry the value. The round
//! trip's key loop (`settings_roundtrip`) calls `edit` once per key.

use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::app_support::app_command;
use super::settings_roundtrip::{KeyOutcome, Run};
use super::settings_support::{matches, new_elements, tabs_after, typed_value};
use crate::driver::Element;

impl Run<'_, '_> {
    /// Edits `key` through its control and waits for the file to carry it.
    pub(super) fn edit(
        &mut self,
        section: &str,
        key: &str,
        kind: &str,
    ) -> Result<KeyOutcome, String> {
        let (before, _) = self.get(key)?;
        self.open(section, key)?;
        let label = match key {
            "dictation.vocabulary" => "Vocabulary term",
            "polish.transforms" => "Remove fillers",
            _ => key,
        };
        let element = self
            .driver
            .wait_for_label(self.app, label, self.ctx.timeout)
            .map_err(|e| format!("{section}: control for {key}: {e}"))?;
        let control = element.role.clone();
        let typed = typed_value(key, kind, &self.ctx.profile.root, &self.bin_dir);
        // A poll, so a fast desktop returns at once; a CI runner needs longer
        // for the write, the daemon's reload and the route's refresh.
        let settle = Duration::from_secs(15);
        let landed: Result<(Value, String), String> = if key == "dictation.vocabulary"
            || key == "polish.transforms"
        {
            let mut values = before.as_array().cloned().unwrap_or_default();
            let term = if key == "dictation.vocabulary" {
                "qa-vocabulary-term"
            } else {
                "removeFillers"
            };
            if let Some(index) = values.iter().position(|v| v == term) {
                values.remove(index);
                if key == "dictation.vocabulary" {
                    self.click_named(&format!("Remove vocabulary term {term}"))?;
                } else {
                    self.click_named(label)?;
                }
            } else {
                values.push(json!(term));
                self.click_named(label)?;
                if key == "dictation.vocabulary" {
                    std::thread::sleep(Duration::from_millis(150));
                    self.driver
                        .type_text(self.app, &format!("{term}\n"))
                        .map_err(|e| e.to_string())?;
                }
            }
            self.wait_get(key, settle, |v, s| s == "file" && v == &json!(values))
        } else if control == "text" {
            self.driver
                .click(self.app, &element)
                .map_err(|e| e.to_string())?;
            std::thread::sleep(Duration::from_millis(150));
            self.driver
                .type_text(self.app, &format!("{typed}\n"))
                .map_err(|e| format!("type into {key}: {e}"))?;
            let want = typed.clone();
            self.wait_get(key, settle, |v, s| s == "file" && matches(&want, v))
        } else if control == "check box" || control == "toggle button" || control == "push button" {
            self.driver
                .click(self.app, &element)
                .map_err(|e| e.to_string())?;
            let was = before.as_bool().unwrap_or(false);
            self.wait_get(key, settle, |v, s| s == "file" && v.as_bool() == Some(!was))
        } else if control.contains("tab list") {
            let tree = self.snapshot()?;
            let tabs: Vec<Element> = tabs_after(&tree, &element).into_iter().cloned().collect();
            if tabs.len() < 2 {
                return Err(format!("{key}: the segments show {} tabs", tabs.len()));
            }
            self.pick(section, key, &before, &tabs[tabs.len() - 1], &tabs[0])
        } else if control == "combo box" {
            let items = self.popup_items(key, &element)?;
            self.pick(section, key, &before, &items[items.len() - 1], &items[0])
        } else {
            return Err(format!("{key}: no control kind for role {control:?}"));
        };
        let (value, outcome) = match landed {
            Ok((value, _)) => (value, "written".to_string()),
            Err(e) if key == "audio.input_device" => {
                // The daemon checks the name against PipeWire when a
                // service runs: the refusal shows under the field.
                self.driver
                    .wait_for_label(self.app, &format!("{key} refused"), settle)
                    .map_err(|_| format!("{key}: {e}, and no refusal is shown"))?;
                (before.clone(), "refused inline".to_string())
            }
            Err(e) => return Err(format!("{section}: {e}")),
        };
        self.daemon.call("config.unset", json!({"key": key}))?;
        self.wait_get(key, settle, |_, s| s != "file")
            .map_err(|e| format!("{key} after unset: {e}"))?;
        Ok(KeyOutcome {
            key: key.into(),
            section: section.into(),
            control,
            outcome,
            value,
        })
    }

    /// Opens a combo box and reads its items: the elements the popup adds
    /// to the tree, waited for, with one more click when the first one
    /// found the window not yet in front.
    fn popup_items(&mut self, key: &str, combo: &Element) -> Result<Vec<Element>, String> {
        let closed = self.snapshot()?;
        for attempt in 0..2 {
            self.driver
                .click(self.app, combo)
                .map_err(|e| e.to_string())?;
            let deadline = Instant::now() + Duration::from_secs(2);
            while Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(200));
                let open = self.snapshot()?;
                let items: Vec<Element> = new_elements(&closed, &open)
                    .into_iter()
                    .filter(|e| e.role == "list item" || e.role == "menu item")
                    .cloned()
                    .collect();
                if items.len() >= 2 {
                    return Ok(items);
                }
            }
            if attempt == 0 {
                app_command(self.ctx, &json!({"cmd": "raise"}))?;
            }
        }
        Err(format!("{key}: the combo box never showed its items"))
    }

    /// Clicks `first`, and `second` when the value did not move (the
    /// first was the value in force).
    fn pick(
        &mut self,
        section: &str,
        key: &str,
        before: &Value,
        first: &Element,
        second: &Element,
    ) -> Result<(Value, String), String> {
        self.driver
            .click(self.app, first)
            .map_err(|e| e.to_string())?;
        let moved = |v: &Value, s: &str| s == "file" && v != before;
        if let Ok(got) = self.wait_get(key, Duration::from_secs(2), moved) {
            return Ok(got);
        }
        self.open(section, key)?;
        let element = self
            .driver
            .wait_for_label(self.app, key, self.ctx.timeout)
            .map_err(|e| e.to_string())?;
        if element.role == "combo box" {
            self.driver
                .click(self.app, &element)
                .map_err(|e| e.to_string())?;
            std::thread::sleep(Duration::from_millis(300));
        }
        let fresh = self
            .driver
            .wait_for_label(self.app, &second.name, Duration::from_secs(2))
            .map_err(|e| format!("{key}: second choice {:?}: {e}", second.name))?;
        self.driver
            .click(self.app, &fresh)
            .map_err(|e| e.to_string())?;
        self.wait_get(key, Duration::from_secs(15), moved)
    }
}
