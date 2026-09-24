use super::*;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

fn script(path: &Path, body: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn recording_cleanup_is_scoped_and_reports_failure() {
    const CHILD: &str = "DETTIVO_TEST_BAR_CLEANUP";
    if let Ok(root) = std::env::var(CHILD) {
        let repo = Path::new(&root);
        let mut profile = crate::profile::Profile::create("bar-cleanup", None).unwrap();
        let mut timings = crate::evidence::Timings::start();
        let mut evidence = Vec::new();
        let mut ctx = Context {
            profile: &mut profile,
            evidence_dir: repo,
            repo_root: repo,
            timings: &mut timings,
            evidence: &mut evidence,
            timeout: Duration::from_millis(100),
        };
        let mut driver = crate::driver::by_name("cua").unwrap();
        let error = OmarchyBar.run(driver.as_mut(), &mut ctx).unwrap_err();
        let mode = std::env::var("DETTIVO_TEST_BAR_MODE").unwrap();
        let state = std::fs::read_to_string(repo.join("state")).unwrap_or_default();
        match mode.as_str() {
            "preexisting" => {
                assert!(error.contains("already running"), "{error}");
                assert_eq!(state.trim(), "other");
                assert!(!repo.join("cancelled").exists());
            }
            "cleanup-error" => {
                assert!(error.contains("grim exited"), "{error}");
                assert!(error.contains("cleanup failed"), "{error}");
            }
            "status-error" => {
                assert!(error.contains("dictation status failed"), "{error}");
                assert_eq!(state.trim(), "", "failed status left recording active");
                assert!(repo.join("cancelled").exists());
            }
            _ => {
                assert!(error.contains("grim exited"), "{error}");
                assert_eq!(state.trim(), "", "failed capture left recording active");
                let cancel = std::fs::read_to_string(repo.join("cancelled")).unwrap();
                assert!(cancel.contains(r#""expected_job_id":"owned""#), "{cancel}");
            }
        }
        return;
    }
    for mode in [
        "capture-error",
        "preexisting",
        "cleanup-error",
        "status-error",
    ] {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path();
        script(
            &repo.join("hyprctl"),
            r#"echo '[{"focused":true,"name":"test","width":900}]'"#,
        );
        script(
            &repo.join("grim"),
            r#"
case "$*" in *listening*) exit 7;; esac
for last do :; done
printf idle > "$last"
"#,
        );
        script(
            &repo.join("build/qt/apps/dettivo/dettivo"),
            r#"
root="$DETTIVO_TEST_BAR_CLEANUP"
case "$3" in
status)
  if test "$DETTIVO_TEST_BAR_MODE" = status-error && test -s "$root/state"; then exit 9; fi
  if test -s "$root/state"; then
    printf '{"is_active":true,"job":{"job_id":"%s","message":"listening"}}\n' "$(/bin/cat "$root/state")"
  else echo '{"is_active":false}'; fi;;
start) printf owned > "$root/state"; echo '{"job":{"job_id":"owned"}}';;
dictation.cancel)
  printf '%s' "$4" > "$root/cancelled"
  if test "$DETTIVO_TEST_BAR_MODE" = cleanup-error; then echo 'cancel unavailable' >&2; exit 2; fi
  printf '' > "$root/state"; echo '{"job":{"job_id":"owned"}}';;
*) exit 8;;
esac
"#,
        );
        let tool = repo.join("build/qt/tools/visual-diff/dettivo-visual-diff");
        script(&tool, "exit 0");
        if mode == "preexisting" {
            std::fs::write(repo.join("state"), "other").unwrap();
        }
        let out = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "scenarios::omarchy_bar::cleanup_tests::recording_cleanup_is_scoped_and_reports_failure", "--nocapture"])
            .env(CHILD, repo)
            .env("DETTIVO_TEST_BAR_MODE", mode)
            .env("PATH", format!("{}:/usr/bin:/bin", repo.display()))
            .output().unwrap();
        assert!(
            out.status.success(),
            "{mode}: {}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
}
