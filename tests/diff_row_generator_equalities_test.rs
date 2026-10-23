mod common;

use common::lines;
use diffutils::{DiffRowGenerator, DiffTag, diff_with_equal_parts};

#[test]
fn test_default_equality_processing_leaves_text_unchanged() {
    let generator = DiffRowGenerator::builder().show_inline_diffs(false).build();
    let rows = generator.generate_diff_rows(&lines(&["hello world"]), &lines(&["hello world"]));
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].old_line, "hello world");
    assert_eq!(rows[0].new_line, "hello world");
    assert!(matches!(rows[0].tag, DiffTag::Equal));
}

#[test]
fn test_custom_equality_processing_is_applied() {
    let generator = DiffRowGenerator::builder().show_inline_diffs(false).process_equalities(|t| format!("[{t}]")).build();
    let rows = generator.generate_diff_rows(&lines(&["A", "B"]), &lines(&["A", "B"]));
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].old_line, "[A]");
    assert_eq!(rows[1].old_line, "[B]");
}

#[test]
fn test_html_escaping_equalities_works_with_default_normalizer() {
    let generator = DiffRowGenerator::builder()
        .show_inline_diffs(true)
        .inline_diff_by_word(true)
        .process_equalities(|s| s.replace('<', "&lt;").replace('>', "&gt;"))
        .build();
    let rows = generator.generate_diff_rows(&lines(&["hello <world>"]), &lines(&["hello <world>"]));
    assert!(rows[0].old_line.contains("&lt;world&gt;"));
    assert!(rows[0].new_line.contains("&lt;world&gt;"));
}

#[test]
fn test_equalities_processed_but_inline_diff_still_present() {
    let generator = DiffRowGenerator::builder()
        .show_inline_diffs(true)
        .inline_diff_by_word(true)
        .process_equalities(|s| format!("({s})"))
        .build();
    let rows = generator.generate_diff_rows(&lines(&["hello world"]), &lines(&["hello there"]));
    let row = &rows[0];
    assert!(matches!(row.tag, DiffTag::Change));
    assert!(row.old_line.contains("span") || row.new_line.contains("span"), "Expected inline <span> diff markup in old or new line");
    assert!(row.old_line.starts_with("hello "), "Equal (unchanged) inline segment should remain unchanged");
}

#[test]
fn test_equal_delta_is_reported_as_equal_without_inline_diffs() {
    let original = lines(&["same", "before"]);
    let patch = diff_with_equal_parts(&original, &lines(&["same", "after"]));
    let rows = DiffRowGenerator::builder()
        .show_inline_diffs(false)
        .build()
        .generate_diff_rows_from_patch(&original, &patch)
        .expect("the patch fits the original");
    assert_eq!(rows.len(), 2);
    assert!(matches!(rows[0].tag, DiffTag::Equal));
    assert_eq!(rows[0].old_line, "same");
    assert!(matches!(rows[1].tag, DiffTag::Change));
}

#[test]
fn test_equal_delta_is_reported_as_equal_with_inline_diffs() {
    let original = lines(&["same", "before"]);
    let patch = diff_with_equal_parts(&original, &lines(&["same", "after"]));
    let rows = DiffRowGenerator::builder()
        .show_inline_diffs(true)
        .build()
        .generate_diff_rows_from_patch(&original, &patch)
        .expect("the patch fits the original");
    assert_eq!(rows.len(), 2);
    assert!(matches!(rows[0].tag, DiffTag::Equal));
    assert_eq!(rows[0].new_line, "same");
    assert!(matches!(rows[1].tag, DiffTag::Change));
}
