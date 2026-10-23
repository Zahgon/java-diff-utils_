mod common;

use common::{applied, fixture_lines, java_split, lines, single_file_diff, written_text};
use diffutils::{diff, read_unified_diff};

fn verify(orig_lines: &[String], rev_lines: &[String], original_file: &str, revised_file: &str) {
    let patch = diff(orig_lines, rev_lines);
    let text = written_text(&single_file_diff("header", "tail", Some(original_file), revised_file, patch), orig_lines, 10);
    let read = read_unified_diff(&text).unwrap_or_else(|e| panic!("{} at line {}", e.reason, e.line));
    let file = read
        .files
        .iter()
        .find(|f| f.from_file.as_deref() == Some(original_file))
        .expect("the written file is read back");
    let patched = applied(&file.patch, orig_lines).unwrap_or_else(|reason| panic!("could not apply patch due to {reason}"));
    assert_eq!(rev_lines.len(), patched.len());
    for (i, (expected, got)) in rev_lines.iter().zip(&patched).enumerate() {
        assert_eq!(expected, got, "Line {} of the patched file did not match the revised original", i + 1);
    }
}

#[test]
fn test_generate_unified() {
    verify(&fixture_lines("mocks/original.txt"), &fixture_lines("mocks/revised.txt"), "original.txt", "revised.txt");
}

#[test]
fn test_generate_unified_with_one_delta() {
    verify(
        &fixture_lines("mocks/one_delta_test_original.txt"),
        &fixture_lines("mocks/one_delta_test_revised.txt"),
        "one_delta_test_original.txt",
        "one_delta_test_revised.txt",
    );
}

#[test]
fn test_generate_unified_diff_without_any_deltas() {
    let test = lines(&["abc"]);
    let patch = diff(&test, &test);
    let text = written_text(&single_file_diff("header", "tail", Some("abc"), "abc", patch), &test, 0);
    assert!(text.starts_with("header"));
    assert!(!text.contains("@@"));
}

#[test]
fn test_diff_issue10() {
    let base = fixture_lines("mocks/issue10_base.txt");
    let text = fixture_lines("mocks/issue10_patch.txt").join("\n");
    let read = read_unified_diff(&text).unwrap_or_else(|e| panic!("{} at line {}", e.reason, e.line));
    if let Err(reason) = applied(&read.files[0].patch, &base) {
        panic!("could not apply patch due to {reason}");
    }
}

#[test]
#[ignore = "disabled in the Java suite"]
fn test_patch_with_no_deltas() {
    verify(&fixture_lines("mocks/issue11_1.txt"), &fixture_lines("mocks/issue11_2.txt"), "issue11_1.txt", "issue11_2.txt");
}

#[test]
fn test_diff5() {
    verify(&fixture_lines("mocks/5A.txt"), &fixture_lines("mocks/5B.txt"), "5A.txt", "5B.txt");
}

#[test]
fn test_diff_with_header_line_in_text() {
    let original = lines(&["test line1", "test line2", "test line 4", "test line 5"]);
    let revised = lines(&["test line1", "test line2", "@@ -2,6 +2,7 @@", "test line 4", "test line 5"]);
    let patch = diff(&original, &revised);
    let text = written_text(&single_file_diff("header", "tail", Some("original"), "revised", patch), &original, 10);
    let read = read_unified_diff(&text).unwrap_or_else(|e| panic!("{} at line {}", e.reason, e.line));
    assert_eq!(read.files.len(), 1);
    assert_eq!(java_split(&text, "\n").first().map(String::as_str), Some("header"));
}
