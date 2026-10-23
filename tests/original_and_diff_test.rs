mod common;

use common::fixture_lines;
use diffutils::generate_original_and_diff;

/// Every line of `original` appears in order among the unmarked and `-` lines of the combined listing, and every
/// line of `revised` among its unmarked and `+` lines.
fn check(original: &[String], revised: &[String]) {
    let combined = generate_original_and_diff(original, revised, None, None);
    assert!(!combined.is_empty());
    let is_header = |l: &&String| l.starts_with("--- ") || l.starts_with("+++ ") || l.starts_with("@@ ");
    let body: Vec<&String> = combined.iter().filter(|l| !is_header(l)).collect();
    let old: Vec<String> = body.iter().filter(|l| !l.starts_with('+')).map(|l| strip(l)).collect();
    let new: Vec<String> = body.iter().filter(|l| !l.starts_with('-')).map(|l| strip(l)).collect();
    assert_eq!(old, original);
    assert_eq!(new, revised);
}

fn strip(line: &str) -> String {
    line.strip_prefix(['+', '-', ' ']).unwrap_or(line).to_string()
}

#[test]
fn test_generate_original_and_diff() {
    check(&fixture_lines("mocks/original.txt"), &fixture_lines("mocks/revised.txt"));
}

#[test]
fn test_generate_original_and_diff_first_line_change() {
    check(&fixture_lines("mocks/issue_170_original.txt"), &fixture_lines("mocks/issue_170_revised.txt"));
}
