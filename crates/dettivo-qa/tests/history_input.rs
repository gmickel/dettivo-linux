//! Search submission requires an observed value, including after transient read errors.
use std::collections::VecDeque;
use std::path::Path;
use std::time::Duration;

use dettivo_qa::driver::{App, Driver, DriverError, Element, Launch};
use dettivo_qa::evidence::Timings;
use dettivo_qa::profile::Profile;
use dettivo_qa::scenarios::{Context, history_seed};

struct FieldDriver {
    reads: VecDeque<Result<String, DriverError>>,
    typed: Vec<String>,
}

impl Driver for FieldDriver {
    fn name(&self) -> &'static str {
        "test"
    }
    fn preflight(&mut self) -> Result<(), DriverError> {
        Ok(())
    }
    fn launch(&mut self, _: &Launch, _: Duration) -> Result<App, DriverError> {
        unreachable!()
    }
    fn snapshot(&mut self, _: &App) -> Result<Vec<Element>, DriverError> {
        Ok(vec![Element {
            index: 0,
            id: "field".into(),
            role: "text".into(),
            name: "Search history".into(),
            value: None,
            bounds: None,
            focusable: true,
            focused: false,
            enabled: true,
            parent: None,
            native_id: None,
        }])
    }
    fn click(&mut self, _: &App, _: &Element) -> Result<(), DriverError> {
        Ok(())
    }
    fn type_text(&mut self, _: &App, text: &str) -> Result<(), DriverError> {
        self.typed.push(text.into());
        Ok(())
    }
    fn press_key(&mut self, _: &App, _: &str) -> Result<(), DriverError> {
        unreachable!("search submission uses type_text")
    }
    fn read_value(&mut self, _: &App, _: &Element) -> Result<String, DriverError> {
        if self.reads.len() > 1 {
            self.reads.pop_front().unwrap()
        } else {
            self.reads[0].clone()
        }
    }
    fn screenshot(&mut self, _: &App, _: &Path) -> Result<(), DriverError> {
        unreachable!()
    }
    fn close(&mut self, _: &App) -> Result<(), DriverError> {
        Ok(())
    }
}

#[test]
fn only_an_observed_query_allows_submission() {
    let missing = Err(DriverError::Failed(
        "org.freedesktop.DBus.Error.UnknownObject".into(),
    ));
    for (reads, succeeds) in [
        (vec![missing.clone()], false),
        (vec![Ok("Search history".into())], false),
        (vec![missing, Ok("api".into())], true),
    ] {
        let mut driver = FieldDriver {
            reads: reads.into(),
            typed: Vec::new(),
        };
        let mut profile = Profile::create("history-input", None).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let mut timings = Timings::start();
        let mut evidence = Vec::new();
        let mut ctx = Context {
            profile: &mut profile,
            evidence_dir: dir.path(),
            repo_root: dir.path(),
            timings: &mut timings,
            evidence: &mut evidence,
            timeout: Duration::from_millis(50),
        };
        let result = history_seed::type_query(
            &mut driver,
            &App {
                pid: 1,
                window: None,
            },
            &mut ctx,
            "api",
        );
        assert_eq!(
            result.is_ok(),
            succeeds,
            "{result:?}; typed {:?}",
            driver.typed
        );
        assert_eq!(driver.typed.contains(&"\n".to_string()), succeeds);
        assert_eq!(driver.typed.iter().filter(|s| *s == "api").count(), 1);
    }
}
