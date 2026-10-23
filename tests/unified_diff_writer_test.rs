mod common;

use common::{fixture_text, java_split, lines, single_file_diff, written_text};
use diffutils::{UnifiedDiff, diff, read_unified_diff, write_unified_diff};

#[test]
fn test_write() {
    let text = fixture_text("unifieddiff/jsqlparser_patch_1.diff");
    let diff = read_unified_diff(&text).unwrap_or_else(|e| panic!("{} at line {}", e.reason, e.line));
    let none = |_: Option<&str>| Vec::new();
    let written = write_unified_diff(&diff, &none, 5);
    assert!(!written.is_empty());
    let from = diff.files[0].from_file.clone().expect("a from file");
    assert!(written.iter().any(|l| l.starts_with("--- ") && l.contains(&from)));
}

#[test]
fn test_write_with_new_file() {
    let original: Vec<String> = Vec::new();
    let revised = lines(&["line1", "line2"]);
    let patch = diff(&original, &revised);
    let mut unified = single_file_diff("", "", None, "revised", patch);
    unified = UnifiedDiff { header: None, tail: None, ..unified };
    let text = written_text(&unified, &original, 5);
    let out = java_split(&text, "\n");
    assert_eq!(out[0], "--- /dev/null");
    assert_eq!(out[1], "+++ revised");
    assert_eq!(out[2], "@@ -0,0 +1,2 @@");
}
