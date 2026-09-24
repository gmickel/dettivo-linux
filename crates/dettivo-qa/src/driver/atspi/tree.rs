//! Complete snapshots across accessibility delegate replacement.
use super::{DriverError, Element};

pub(super) fn snapshot(
    mut walk: impl FnMut() -> Result<Vec<Element>, DriverError>,
) -> Result<Vec<Element>, DriverError> {
    let mut retried = false;
    loop {
        match walk() {
            Err(DriverError::Failed(why))
                if why.contains("org.freedesktop.DBus.Error.UnknownObject") =>
            {
                if retried {
                    return Err(DriverError::NotFound(format!(
                        "accessibility tree changed: {why}"
                    )));
                }
                retried = true;
            }
            result => return result,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    #[test]
    fn reacquisition_is_fresh_bounded_and_only_for_vanished_objects() {
        let vanished = DriverError::Failed(
            "children: org.freedesktop.DBus.Error.UnknownObject: No such object".into(),
        );
        let permanent = DriverError::Failed("access denied".into());
        let absent = DriverError::NotFound("application not registered".into());
        let fresh = vec![Element {
            index: 0,
            id: "new-object".into(),
            role: "text".into(),
            name: "fresh tree".into(),
            value: Some("current".into()),
            bounds: None,
            focusable: false,
            focused: false,
            enabled: true,
            parent: None,
            native_id: None,
        }];
        for (errors, expected_calls, expected_error) in [
            (vec![vanished.clone()], 2, None),
            (
                vec![vanished.clone(), vanished.clone()],
                2,
                Some(DriverError::NotFound(format!(
                    "accessibility tree changed: {}",
                    match &vanished {
                        DriverError::Failed(why) => why,
                        _ => unreachable!(),
                    }
                ))),
            ),
            (vec![permanent.clone()], 1, Some(permanent)),
            (vec![absent.clone()], 1, Some(absent)),
        ] {
            let mut replies: VecDeque<_> = errors.into_iter().map(Err).collect();
            replies.push_back(Ok(fresh.clone()));
            let mut calls = 0;
            let result = snapshot(|| {
                calls += 1;
                replies.pop_front().expect("walk bound exceeded")
            });
            assert_eq!(calls, expected_calls);
            match expected_error {
                Some(error) => assert_eq!(result, Err(error)),
                None => assert_eq!(result, Ok(fresh.clone())),
            }
        }
    }
}
