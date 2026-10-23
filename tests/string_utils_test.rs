//! `StringUtils` is internal to the crate; its helpers are checked through the row generator that uses them:
//! `htmlEntites` and `normalize` through the default line normalizer, `wrapText` through `column_width`.

mod common;

use common::lines;
use diffutils::DiffRowGenerator;

fn equal_row(generator: &DiffRowGenerator, line: &str) -> String {
    let rows = generator.generate_diff_rows(&lines(&[line]), &lines(&[line]));
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].old_line, rows[0].new_line);
    rows[0].old_line.clone()
}

fn wrapped(line: &str, width: i32) -> String {
    equal_row(&DiffRowGenerator::builder().column_width(width).build(), line)
}

#[test]
fn test_html_entites() {
    assert_eq!(equal_row(&DiffRowGenerator::builder().build(), "<test>"), "&lt;test&gt;");
}

#[test]
fn test_normalize_string() {
    assert_eq!(equal_row(&DiffRowGenerator::builder().build(), "\ttest"), "    test");
}

#[test]
fn test_wrap_text_string_int() {
    assert_eq!(wrapped("test", 2), "te<br/>st");
    assert_eq!(wrapped("test", 3), "tes<br/>t");
    assert_eq!(wrapped("test", 10), "test");
    assert_eq!(wrapped(".\u{10001}.", 2), ".\u{10001}<br/>.");
    assert_eq!(wrapped("..\u{10001}", 3), "..<br/>\u{10001}");
}

#[test]
fn test_wrap_text_string_int_zero() {
    assert_eq!(wrapped("test", 0), "test");
    assert_eq!(wrapped("test", -1), "test");
}
