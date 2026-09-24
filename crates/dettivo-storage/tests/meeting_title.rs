//! Local automatic titles, isolated by process so timezone tests never change
//! the environment of another thread.
use dettivo_storage::meetings::MeetingRow;

#[test]
fn automatic_title_uses_the_meeting_dates_local_timezone() {
    if let Ok(zone) = std::env::var("DETTIVO_TITLE_TEST_ZONE") {
        let summer = MeetingRow::derived_title("2026-09-22T11:00:00Z");
        let winter = MeetingRow::derived_title("2026-01-22T11:00:00Z");
        if zone == "Europe/Zurich" {
            assert_eq!(summer, "Meeting 2026-09-22 13:00");
            assert_eq!(winter, "Meeting 2026-01-22 12:00");
            assert_eq!(
                MeetingRow::derived_title("2026-09-22T23:30:00Z"),
                "Meeting 2026-09-23 01:30"
            );
        } else {
            assert_eq!(summer, "Meeting 2026-09-22 11:00");
            assert_eq!(winter, "Meeting 2026-01-22 11:00");
        }
        assert_eq!(MeetingRow::derived_title("invalid"), "Meeting invalid");
        return;
    }
    for zone in ["Europe/Zurich", "UTC"] {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "automatic_title_uses_the_meeting_dates_local_timezone",
                "--nocapture",
            ])
            .env("TZ", zone)
            .env("DETTIVO_TITLE_TEST_ZONE", zone)
            .status()
            .unwrap();
        assert!(status.success(), "timezone {zone}");
    }
}
