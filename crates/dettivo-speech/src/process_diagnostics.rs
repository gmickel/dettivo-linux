//! Which lines of an engine's redacted stderr a crash warning may carry.

/// The known diagnostics among an engine's redacted stderr lines, for a
/// log, matched by format: our own `engine stream closed` line, ggml's
/// `ggml_<backend>:` lines, a C or C++ source location (an assert), a CUDA
/// error, a Rust panic, and the engine's own WARN and ERROR lines.
pub fn diagnostic_lines(tail: &str) -> String {
    tail.lines()
        .filter(|line| is_diagnostic(line))
        .collect::<Vec<_>>()
        .join("\n")
}

fn is_diagnostic(line: &str) -> bool {
    let head = line.split(": ").next().unwrap_or("");
    let mut words = line.split_whitespace();
    let (stamp, level) = (words.next().unwrap_or(""), words.next().unwrap_or(""));
    line.starts_with("engine stream closed")
        || (line.starts_with("ggml_")
            && head.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
        || (line.starts_with('/')
            && [".c", ".cc", ".cpp", ".cu", ".h"]
                .iter()
                .any(|e| head.ends_with(e)))
        || line.starts_with("CUDA error")
        || (line.starts_with("thread '") && line.contains("' panicked at "))
        || (stamp.len() >= 20
            && stamp.as_bytes()[4] == b'-'
            && stamp.ends_with('Z')
            && matches!(level, "WARN" | "ERROR"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A crash log keeps what says why and drops what could be speech.
    #[test]
    fn a_crash_log_keeps_diagnostics_and_drops_everything_else() {
        let tail = [
            "engine stream closed; last stderr:",
            "2026-09-29T12:55:59.987535Z  INFO engine ready",
            "segment: confidential launch plans",
            "ggml_vulkan: <redacted 50 chars>",
            "/__w/x/ggml/src/ggml-backend.cpp: <redacted 30 chars>",
            "2026-09-29T12:56:00.1Z ERROR load failed",
            "we should ship on friday",
            "segment: confidential ggml launch plans",
            "the ggml_vulkan plan is secret",
        ]
        .join("\n");
        assert_eq!(
            diagnostic_lines(&tail),
            [
                "engine stream closed; last stderr:",
                "ggml_vulkan: <redacted 50 chars>",
                "/__w/x/ggml/src/ggml-backend.cpp: <redacted 30 chars>",
                "2026-09-29T12:56:00.1Z ERROR load failed",
            ]
            .join("\n")
        );
    }
}
