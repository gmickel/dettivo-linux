//! Which lines of an engine's redacted stderr a crash warning may carry.

/// The known diagnostics among an engine's redacted stderr lines, for a
/// log, matched by format: our own `engine stream closed` line, ggml's
/// `ggml_<backend>:` lines, a C or C++ source location (an assert), a CUDA
/// error, a Rust panic, the C++ runtime's report of an uncaught Vulkan or
/// ggml exception, and the engine's own WARN and ERROR lines.
pub fn diagnostic_lines(tail: &str) -> String {
    tail.lines()
        .filter(|line| is_diagnostic(line))
        .collect::<Vec<_>>()
        .join("\n")
}

/// libstdc++'s two lines for an exception nothing caught, kept whole by
/// redaction because they carry only library text: `terminate called
/// after throwing an instance of '<type>'` for a plain C++ type name, and
/// `  what():  <message>` when the message is Vulkan's (`vk::`) or
/// ggml's, such as `vk::Device::allocateMemory: ErrorOutOfDeviceMemory`.
pub fn is_library_exception(line: &str) -> bool {
    if let Some(rest) = line.strip_prefix("terminate called after throwing an instance of '") {
        return rest.strip_suffix('\'').is_some_and(|t| {
            !t.is_empty()
                && t.chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == ':')
        });
    }
    line.trim_start()
        .strip_prefix("what():")
        .map(str::trim_start)
        .is_some_and(|m| m.starts_with("vk::") || m.starts_with("ggml"))
}

fn is_diagnostic(line: &str) -> bool {
    let head = line.split(": ").next().unwrap_or("");
    let mut words = line.split_whitespace();
    let (stamp, level) = (words.next().unwrap_or(""), words.next().unwrap_or(""));
    line.starts_with("engine stream closed")
        || is_library_exception(line)
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
            "terminate called after throwing an instance of 'vk::OutOfDeviceMemoryError'",
            "  what():  vk::Device::allocateMemory: ErrorOutOfDeviceMemory",
            "terminate called after throwing an instance of 'our launch is friday'",
            "  what():  we should ship on friday",
            "what(): <redacted 40 chars>",
        ]
        .join("\n");
        assert_eq!(
            diagnostic_lines(&tail),
            [
                "engine stream closed; last stderr:",
                "ggml_vulkan: <redacted 50 chars>",
                "/__w/x/ggml/src/ggml-backend.cpp: <redacted 30 chars>",
                "2026-09-29T12:56:00.1Z ERROR load failed",
                "terminate called after throwing an instance of 'vk::OutOfDeviceMemoryError'",
                "  what():  vk::Device::allocateMemory: ErrorOutOfDeviceMemory",
            ]
            .join("\n")
        );
    }
}
