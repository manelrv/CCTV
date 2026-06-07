//! Reads context token usage from Claude Code transcript files (.jsonl).
//!
//! Transcripts live at ~/.claude/projects/<slug>/<session_id>.jsonl where the
//! slug is the cwd with every '/' replaced by '-' (cwd starts with '/', so the
//! slug starts with '-'). Verified empirically 2026-06-07.
//!
//! Context occupancy = input_tokens + cache_read_input_tokens +
//! cache_creation_input_tokens of the LAST assistant message that contains a
//! `message.usage` field.

use std::path::{Path, PathBuf};

/// Converts an absolute cwd path to the directory slug used by Claude Code.
/// Rule (empirically verified): replace every '/' with '-'.
/// Because cwd starts with '/', the result starts with '-'.
///
/// Example: /Users/me/side-projects/CCTV → -Users-me-side-projects-CCTV
pub fn cwd_to_slug(cwd: &str) -> String {
    cwd.replace('/', "-")
}

/// Returns the transcript path for a given cwd + session_id pair.
/// Returns None if the home directory cannot be resolved.
pub fn transcript_path(cwd: &str, session_id: &str) -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    let slug = cwd_to_slug(cwd);
    let filename = format!("{session_id}.jsonl");
    Some(home.join(".claude").join("projects").join(slug).join(filename))
}

/// Reads the last `message.usage` block from a transcript file and returns the
/// sum of the three input-side token fields (input_tokens +
/// cache_read_input_tokens + cache_creation_input_tokens).
///
/// Strategy: seek to the last 256 KiB of the file (transcripts can be several
/// MB), skip the first (potentially partial) line, scan forward looking for
/// lines that contain a `message.usage` field, keep the last one found.
///
/// Returns None on any I/O error, if no usage line is found, or if parsing fails.
pub fn read_context_tokens(path: &Path) -> Option<u64> {
    use std::fs::File;
    use std::io::{Read, Seek, SeekFrom};

    let mut file = File::open(path).ok()?;
    let len = file.metadata().ok()?.len();

    const WINDOW: u64 = 256 * 1024;
    let start = len.saturating_sub(WINDOW);
    if start > 0 {
        file.seek(SeekFrom::Start(start)).ok()?;
    }

    let mut buf = String::new();
    file.read_to_string(&mut buf).ok()?;

    let mut lines = buf.split('\n');

    // If we seeked into the middle of the file the first chunk is a partial line;
    // skip it unconditionally (when start==0 we still skip the first line, which
    // is only wrong for single-line files — acceptable for our purposes).
    if start > 0 {
        lines.next();
    }

    let mut last_tokens: Option<u64> = None;

    for line in lines {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(tokens) = extract_usage_tokens(line) {
            last_tokens = Some(tokens);
        }
    }

    last_tokens
}

/// Parses a single JSONL line and extracts the sum of the three input-side
/// token fields from `message.usage`, if present.
fn extract_usage_tokens(line: &str) -> Option<u64> {
    let v: serde_json::Value = serde_json::from_str(line).ok()?;
    let usage = v.get("message")?.get("usage")?;
    let input = usage.get("input_tokens").and_then(|x| x.as_u64()).unwrap_or(0);
    let cache_read = usage
        .get("cache_read_input_tokens")
        .and_then(|x| x.as_u64())
        .unwrap_or(0);
    let cache_create = usage
        .get("cache_creation_input_tokens")
        .and_then(|x| x.as_u64())
        .unwrap_or(0);
    Some(input + cache_read + cache_create)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_jsonl(lines: &[&str]) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        for line in lines {
            writeln!(f, "{}", line).unwrap();
        }
        f
    }

    fn usage_line(input: u64, cache_read: u64, cache_create: u64) -> String {
        format!(
            r#"{{"message":{{"usage":{{"input_tokens":{input},"cache_read_input_tokens":{cache_read},"cache_creation_input_tokens":{cache_create}}}}}}}"#
        )
    }

    #[test]
    fn returns_last_usage_line() {
        let line1 = usage_line(10, 100, 50);
        let line2 = usage_line(2, 303833, 38);
        let f = write_jsonl(&[&line1, &line2]);
        // line2: 2 + 303833 + 38 = 303873
        assert_eq!(read_context_tokens(f.path()), Some(303873));
    }

    #[test]
    fn no_usage_lines_returns_none() {
        let f = write_jsonl(&[
            r#"{"type":"human","message":{"role":"user","content":"hello"}}"#,
            r#"{"type":"assistant","message":{"role":"assistant","content":"hi"}}"#,
        ]);
        assert_eq!(read_context_tokens(f.path()), None);
    }

    #[test]
    fn garbage_lines_are_ignored() {
        let good = usage_line(5, 200, 10);
        let f = write_jsonl(&["not json at all", "{incomplete", &good, "also bad"]);
        // Only the third line is valid and has usage
        assert_eq!(read_context_tokens(f.path()), Some(215));
    }

    #[test]
    fn file_larger_than_window_still_works() {
        use std::io::Write;
        let mut f = tempfile::NamedTempFile::new().unwrap();
        // Write padding lines to exceed the 256 KiB window
        let padding = r#"{"type":"human","message":{"role":"user","content":"x"}}"#;
        for _ in 0..5000 {
            writeln!(f, "{}", padding).unwrap();
        }
        // Now write the final usage line that must be found
        let good = usage_line(3, 400000, 22);
        writeln!(f, "{}", good).unwrap();
        assert_eq!(read_context_tokens(f.path()), Some(400025));
    }

    #[test]
    fn missing_file_returns_none() {
        let p = std::path::Path::new("/nonexistent/path/to/file.jsonl");
        assert_eq!(read_context_tokens(p), None);
    }

    #[test]
    fn cwd_to_slug_replaces_slashes() {
        assert_eq!(
            cwd_to_slug("/Users/me/side-projects/CCTV"),
            "-Users-me-side-projects-CCTV"
        );
        assert_eq!(
            cwd_to_slug("/Users/manelrv/side-projects/CCTV/src-tauri"),
            "-Users-manelrv-side-projects-CCTV-src-tauri"
        );
    }

    #[test]
    fn cwd_to_slug_no_dots_introduced() {
        // Dots in path segments must be preserved as-is (not converted)
        assert_eq!(cwd_to_slug("/Users/x/my.project"), "-Users-x-my.project");
    }
}
