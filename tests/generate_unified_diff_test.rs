mod common;

use common::{change_positions, fixture_lines, lines};
use diffutils::{DeltaType, diff, generate_unified_diff, parse_unified_diff};

fn verify(orig_lines: &[String], rev_lines: &[String], original_file: &str, revised_file: &str) {
    let patch = diff(orig_lines, rev_lines);
    let unified = generate_unified_diff(Some(original_file), Some(revised_file), orig_lines, &patch, 10);
    let from_unified = parse_unified_diff(&unified);
    let mut patched = orig_lines.to_vec();
    if let Err(e) = from_unified.apply_to(&mut patched) {
        panic!("could not apply patch due to {}", e.reason);
    }
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
    let patch = diff(&test, &lines(&["abc2"]));
    let text = generate_unified_diff(Some("abc1"), Some("abc2"), &test, &patch, 0).join("\n");
    assert!(text.contains("--- abc1"), "original filename should be abc1");
    assert!(text.contains("+++ abc2"), "revised filename should be abc2");
}

#[test]
fn test_diff_issue10() {
    let mut base = fixture_lines("mocks/issue10_base.txt");
    let patch = parse_unified_diff(&fixture_lines("mocks/issue10_patch.txt"));
    if let Err(e) = patch.apply_to(&mut base) {
        panic!("could not apply patch due to {}", e.reason);
    }
}

#[test]
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
    let udiff = generate_unified_diff(Some("original"), Some("revised"), &original, &patch, 10);
    let parsed = parse_unified_diff(&udiff);
    let mut patched = original.clone();
    assert!(parsed.apply_to(&mut patched).is_ok());
    assert_eq!(patched, revised);
}

#[test]
fn test_new_file_creation() {
    let original: Vec<String> = Vec::new();
    let revised = lines(&["line1", "line2"]);
    let patch = diff(&original, &revised);
    let udiff = generate_unified_diff(None, Some("revised"), &original, &patch, 10);
    assert_eq!(udiff[0], "--- /dev/null");
    assert_eq!(udiff[1], "+++ revised");
    assert_eq!(udiff[2], "@@ -0,0 +1,2 @@");
    let parsed = parse_unified_diff(&udiff);
    assert_eq!(parsed.deltas().len(), 1);
    assert!(matches!(parsed.deltas()[0].kind, DeltaType::Change));
}

#[test]
fn test_change_position() {
    let patch = parse_unified_diff(&fixture_lines("mocks/issue89_patch.txt"));
    let (removed, added) = change_positions(&patch, 0);
    assert_eq!(removed, [3]);
    assert_eq!(added, [3, 7, 8, 9, 10, 11, 12, 13, 14]);
    let (removed, added) = change_positions(&patch, 1);
    assert!(removed.is_empty());
    assert_eq!(added, [27, 28]);
}

#[test]
fn test_failing_patch_by_exception() {
    let mut base = fixture_lines("mocks/issue10_base.txt");
    let patch = parse_unified_diff(&fixture_lines("mocks/issue10_patch.txt"));
    base[40] = format!("{} corrupted ", base[40]);
    let before = base.clone();
    let failure = patch.apply_to(&mut base).expect_err("a corrupted base must not take the patch");
    assert_eq!(failure.reason, "CONTENT_DOES_NOT_MATCH_TARGET");
    assert_eq!(base, before);
}

#[test]
fn test_wrong_context_length() {
    let original = fixture_lines("text/issue_119_original.txt");
    let revised = fixture_lines("text/issue_119_revised.txt");
    let patch = diff(&original, &revised);
    let udiff = generate_unified_diff(Some("a/$filename"), Some("b/$filename"), &original, &patch, 3);
    assert!(udiff.iter().any(|l| l == "@@ -1,4 +1,4 @@"));
}
