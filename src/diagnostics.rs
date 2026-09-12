// Gosh AppImage Manager — verbose diagnostics gated by the persisted
// `DebugLogging` setting. Made by Gosh.
//
// The Settings switch used to persist a flag nothing read. These helpers make
// it real without new dependencies: call sites check the already-loaded
// setting and emit one `[goshaim:<category>]` line to stderr when enabled.
// Only file names are logged, never full home paths, arguments, environment
// pairs, URLs, or tokens.

use std::io::Write;

/// Format one diagnostics line. Pure and testable; emission helpers below
/// decide whether to print it.
pub fn format_line(category: &str, detail: &str) -> String {
    format!("[goshaim:{category}] {detail}")
}

/// Basename-only label so logs never carry full home paths. Truncated to
/// keep a hostile 255-byte file name from bloating stderr.
pub fn file_label(path: &str) -> String {
    const MAX_LABEL_CHARS: usize = 96;
    let base = std::path::Path::new(path)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let base = base.trim();
    if base.is_empty() {
        return "file".to_string();
    }
    let mut label: String = base.chars().take(MAX_LABEL_CHARS).collect();
    if base.chars().count() > MAX_LABEL_CHARS {
        label.push('…');
    }
    label
}

/// Write one line to an explicit writer (CLI stderr) when enabled.
/// Keeps diagnostics testable: tests pass a `Vec<u8>` instead of stderr.
pub fn write_if(writer: &mut dyn Write, verbose: bool, category: &str, detail: &str) {
    if verbose {
        let _ = writeln!(writer, "{}", format_line(category, detail));
    }
}

/// Emit one line to process stderr (GUI workers, services) when enabled.
pub fn emit_if(verbose: bool, category: &str, detail: &str) {
    if verbose {
        eprintln!("{}", format_line(category, detail));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_produces_no_output() {
        let mut buf = Vec::new();
        write_if(&mut buf, false, "inspect", "file=Foo.AppImage");
        assert!(buf.is_empty());
        assert!(format_line("inspect", "x").starts_with("[goshaim:inspect]"));
    }

    #[test]
    fn file_label_hides_directories() {
        assert_eq!(
            file_label("/home/someone/AppImages/Foo.AppImage"),
            "Foo.AppImage"
        );
        assert_eq!(file_label(""), "file");
        assert_eq!(file_label("/"), "file");
        let long = format!("/{}.AppImage", "a".repeat(200));
        let label = file_label(&long);
        assert!(!label.contains("home"));
        assert!(label.chars().count() <= 97);
    }
}
