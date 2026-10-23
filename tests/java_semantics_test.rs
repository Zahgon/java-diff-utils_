//! Behaviour the Java suite leaves implicit: `String.trim`, the row builder defaults, the greedy algorithm's choice
//! between equal-cost paths, when unified diff hunks merge, `.` and `\s` of `java.util.regex`, `String.split`,
//! `Integer.parseInt` overflow and `BufferedReader.readLine` line ends.

mod common;

use common::{delta_string, lines};
use diffutils::{
    DiffRowGenerator, DiffTag, diff, diff_text, generate_unified_diff, parse_unified_diff, parse_unified_diff_checked,
    read_unified_diff,
};

fn whitespace_insensitive_tag(original: &str, revised: &str) -> DiffTag {
    let rows = DiffRowGenerator::builder()
        .ignore_white_spaces(true)
        .build()
        .generate_diff_rows(&lines(&[original]), &lines(&[revised]));
    assert_eq!(rows.len(), 1);
    rows.into_iter().next().map(|r| r.tag).expect("one row")
}

#[test]
fn trim_strips_control_characters() {
    assert!(matches!(whitespace_insensitive_tag("a\u{1}", "a"), DiffTag::Equal));
    assert!(matches!(whitespace_insensitive_tag("\u{1f}a", "a"), DiffTag::Equal));
}

#[test]
fn trim_keeps_unicode_spaces() {
    assert!(matches!(whitespace_insensitive_tag("a\u{a0}", "a"), DiffTag::Change));
    assert!(matches!(whitespace_insensitive_tag("\u{2003}a", "a"), DiffTag::Change));
}

#[test]
fn reader_trims_file_names_like_java() {
    let diff = read_unified_diff("--- a/x\u{1}\n+++ b/y\u{3000}\n@@ -1 +1 @@\n-a\n+b\n").expect("a well-formed diff");
    assert_eq!(diff.files[0].from_file.as_deref(), Some("x"));
    assert_eq!(diff.files[0].to_file.as_deref(), Some("y\u{3000}"));
}

#[test]
fn builder_decompresses_deltas_by_default() {
    let original = lines(&["head", "one two", "tail"]);
    let revised = lines(&["head", "one", "x", "two", "tail"]);
    let tags: Vec<&str> = DiffRowGenerator::builder()
        .build()
        .generate_diff_rows(&original, &revised)
        .iter()
        .map(|r| common::tag_name(&r.tag))
        .collect();
    assert_eq!(tags, ["EQUAL", "CHANGE", "INSERT", "INSERT", "EQUAL"]);
}

fn deltas(original: &[&str], revised: &[&str]) -> Vec<String> {
    diff(&lines(original), &lines(revised)).deltas().iter().map(delta_string).collect()
}

#[test]
fn greedy_ties_extend_from_the_lower_diagonal() {
    assert_eq!(deltas(&["a", "b"], &["b", "a"]), ["[DeleteDelta, position: 0, lines: [a]]", "[InsertDelta, position: 2, lines: [a]]"]);
    assert_eq!(
        deltas(&["a", "b", "c"], &["c", "b", "a"]),
        ["[DeleteDelta, position: 0, lines: [a, b]]", "[InsertDelta, position: 3, lines: [b, a]]"]
    );
    assert_eq!(deltas(&["x", "y"], &["y", "z"]), ["[DeleteDelta, position: 0, lines: [x]]", "[InsertDelta, position: 2, lines: [z]]"]);
}

fn hunk_count(gap: usize, context: usize) -> usize {
    let original: Vec<String> = (0..12 + gap).map(|i| format!("line {i}")).collect();
    let mut revised = original.clone();
    revised[3] = "changed".to_string();
    revised[4 + gap] = "changed too".to_string();
    let patch = diff(&original, &revised);
    assert_eq!(patch.deltas().len(), 2);
    let text = generate_unified_diff(Some("a"), Some("b"), &original, &patch, context);
    text.iter().filter(|l| l.starts_with("@@ ")).count()
}

#[test]
fn hunks_merge_when_the_gap_is_at_most_twice_the_context() {
    for context in 1..=3 {
        assert_eq!(hunk_count(2 * context - 1, context), 1);
        assert_eq!(hunk_count(2 * context, context), 1);
        assert_eq!(hunk_count(2 * context + 1, context), 2);
    }
}

#[test]
fn ascii_space_class_excludes_no_break_space() {
    assert!(matches!(whitespace_insensitive_tag("a\u{a0}b", "a b"), DiffTag::Change));
    assert!(matches!(whitespace_insensitive_tag("a\u{b}b", "a b"), DiffTag::Equal));
}

#[test]
fn hunk_header_dot_stops_at_next_line() {
    let text = lines(&["--- a", "+++ b", "@@ -1 +1 @@", "-a", "+b", "@@ -5 +5 @@ x\u{85}y", "-c", "+d"]);
    assert_eq!(parse_unified_diff(&text).deltas().len(), 1);
    let text = lines(&["--- a", "+++ b", "@@ -1 +1 @@", "-a", "+b", "@@ -5 +5 @@ x\u{85}", "-c", "+d"]);
    assert_eq!(parse_unified_diff(&text).deltas().len(), 2);
}

#[test]
fn text_split_drops_trailing_empty_lines() {
    assert!(diff_text("a\n\n", "a\n").deltas().is_empty());
    assert!(diff_text("a\n\n\n", "a").deltas().is_empty());
}

#[test]
fn numbers_too_large_for_an_int_fail() {
    let checked = parse_unified_diff_checked(&lines(&["--- a", "+++ b", "@@ -4294967297 +1 @@", "-a", "+b"]));
    let failure = checked.expect_err("the start line overflows an int");
    assert_eq!((failure.reason.as_str(), failure.line), ("INVALID_NUMBER", 3));
    let read = read_unified_diff("diff --git a/x b/y\nsimilarity index 4294967296%\nrename from x\nrename to y\n");
    assert_eq!(read.expect_err("the similarity index overflows an int").reason, "INVALID_NUMBER");
}

#[test]
fn reader_ends_lines_at_a_lone_carriage_return() {
    let diff = read_unified_diff("--- a/x\r+++ b/x\r@@ -1 +1 @@\r-a\r+b\r").expect("a well-formed diff");
    assert_eq!(diff.files.len(), 1);
    assert_eq!(diff.files[0].from_file.as_deref(), Some("x"));
    assert_eq!(diff.files[0].patch.deltas().len(), 1);
}
