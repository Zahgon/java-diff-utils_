mod common;

use common::{chunk_eq, delta_string, fixture_text, java_split, kind_name, lines, patch_string};
use diffutils::{DeltaType, diff, diff_inline, diff_with_equal_parts};

#[test]
fn test_diff_insert() {
    let patch = diff(&lines(&["hhh"]), &lines(&["hhh", "jjj", "kkk"]));
    assert_eq!(patch.deltas().len(), 1);
    let delta = &patch.deltas()[0];
    assert!(matches!(delta.kind, DeltaType::Insert));
    assert!(chunk_eq(&delta.source, 1, &[]));
    assert!(chunk_eq(&delta.target, 1, &["jjj", "kkk"]));
}

#[test]
fn test_diff_delete() {
    let patch = diff(&lines(&["ddd", "fff", "ggg"]), &lines(&["ggg"]));
    assert_eq!(patch.deltas().len(), 1);
    let delta = &patch.deltas()[0];
    assert!(matches!(delta.kind, DeltaType::Delete));
    assert!(chunk_eq(&delta.source, 0, &["ddd", "fff"]));
    assert!(chunk_eq(&delta.target, 0, &[]));
}

#[test]
fn test_diff_change() {
    let patch = diff(&lines(&["aaa", "bbb", "ccc"]), &lines(&["aaa", "zzz", "ccc"]));
    assert_eq!(patch.deltas().len(), 1);
    let delta = &patch.deltas()[0];
    assert!(matches!(delta.kind, DeltaType::Change));
    assert!(chunk_eq(&delta.source, 1, &["bbb"]));
    assert!(chunk_eq(&delta.target, 1, &["zzz"]));
}

#[test]
fn test_diff_empty_list() {
    let patch = diff::<String>(&[], &[]);
    assert_eq!(patch.deltas().len(), 0);
}

#[test]
fn test_diff_empty_list_with_non_empty() {
    let patch = diff(&[], &lines(&["aaa"]));
    assert_eq!(patch.deltas().len(), 1);
    assert!(matches!(patch.deltas()[0].kind, DeltaType::Insert));
}

#[test]
fn test_diff_inline() {
    let patch = diff_inline("", "test");
    assert_eq!(patch.deltas().len(), 1);
    let delta = &patch.deltas()[0];
    assert!(matches!(delta.kind, DeltaType::Insert));
    assert_eq!(delta.source.position, 0);
    assert_eq!(delta.source.lines.len(), 0);
    assert_eq!(delta.target.lines[0], "test");
}

#[test]
fn test_diff_inline2() {
    let patch = diff_inline("es", "fest");
    assert_eq!(patch.deltas().len(), 2);
    let deltas = patch.deltas();
    assert!(matches!(deltas[0].kind, DeltaType::Insert));
    assert_eq!(deltas[0].source.position, 0);
    assert_eq!(deltas[1].source.position, 2);
    assert_eq!(deltas[0].source.lines.len(), 0);
    assert_eq!(deltas[1].source.lines.len(), 0);
    assert_eq!(deltas[0].target.lines[0], "f");
    assert_eq!(deltas[1].target.lines[0], "t");
}

#[test]
fn test_diff_integer_list() {
    let patch = diff(&lines(&["1", "2", "3", "4", "5"]), &lines(&["2", "3", "4", "6"]));
    assert_eq!(patch.deltas().len(), 2);
    assert_eq!(delta_string(&patch.deltas()[0]), "[DeleteDelta, position: 0, lines: [1]]");
    assert_eq!(delta_string(&patch.deltas()[1]), "[ChangeDelta, position: 4, lines: [5] to [6]]");
}

#[test]
fn test_diff_misses_change_fork_dnaumenko_issue31() {
    let patch = diff(&lines(&["line1", "line2", "line3"]), &lines(&["line1", "line2-2", "line4"]));
    assert_eq!(patch.deltas().len(), 1);
    assert_eq!(
        delta_string(&patch.deltas()[0]),
        "[ChangeDelta, position: 1, lines: [line2, line3] to [line2-2, line4]]"
    );
}

#[test]
#[ignore = "the Java test is disabled and its large_dataset1.zip fixture is not in the repository"]
fn test_possible_diff_hang_on_large_dataset_dnaumenko_issue26() {
    let original = common::fixture_lines("mocks/large_dataset1/ta");
    let revised = common::fixture_lines("mocks/large_dataset1/tb");
    assert_eq!(diff(&original, &revised).deltas().len(), 1);
}

#[test]
fn test_diff_myers_example1() {
    let patch = diff(&lines(&["A", "B", "C", "A", "B", "B", "A"]), &lines(&["C", "B", "A", "B", "A", "C"]));
    assert_eq!(patch.deltas().len(), 4);
    assert_eq!(
        patch_string(&patch),
        "Patch{deltas=[[DeleteDelta, position: 0, lines: [A, B]], [InsertDelta, position: 3, lines: [B]], [DeleteDelta, position: 5, lines: [B]], [InsertDelta, position: 7, lines: [C]]]}"
    );
}

#[test]
fn test_diff_equal() {
    let patch = diff_with_equal_parts(&lines(&["hhh", "jjj", "kkk"]), &lines(&["hhh", "jjj", "kkk"]));
    assert_eq!(patch.deltas().len(), 1);
    let delta = &patch.deltas()[0];
    assert!(matches!(delta.kind, DeltaType::Equal));
    assert!(chunk_eq(&delta.source, 0, &["hhh", "jjj", "kkk"]));
    assert!(chunk_eq(&delta.target, 0, &["hhh", "jjj", "kkk"]));
}

#[test]
fn test_diff_insert_with_equal() {
    let patch = diff_with_equal_parts(&lines(&["hhh"]), &lines(&["hhh", "jjj", "kkk"]));
    assert_eq!(patch.deltas().len(), 2);
    let delta = &patch.deltas()[0];
    assert!(matches!(delta.kind, DeltaType::Equal));
    assert!(chunk_eq(&delta.source, 0, &["hhh"]));
    assert!(chunk_eq(&delta.target, 0, &["hhh"]));
    let delta = &patch.deltas()[1];
    assert!(matches!(delta.kind, DeltaType::Insert));
    assert!(chunk_eq(&delta.source, 1, &[]));
    assert!(chunk_eq(&delta.target, 1, &["jjj", "kkk"]));
}

#[test]
fn test_diff_problem_issue42() {
    let patch = diff_with_equal_parts(&lines(&["The", "dog", "is", "brown"]), &lines(&["The", "fox", "is", "down"]));
    assert_eq!(patch.deltas().len(), 4);
    let kinds: Vec<&str> = patch.deltas().iter().map(|d| kind_name(&d.kind)).collect();
    assert_eq!(kinds, ["EQUAL", "CHANGE", "EQUAL", "CHANGE"]);
    let d = patch.deltas();
    assert!(chunk_eq(&d[0].source, 0, &["The"]) && chunk_eq(&d[0].target, 0, &["The"]));
    assert!(chunk_eq(&d[1].source, 1, &["dog"]) && chunk_eq(&d[1].target, 1, &["fox"]));
    assert!(chunk_eq(&d[2].source, 2, &["is"]) && chunk_eq(&d[2].target, 2, &["is"]));
    assert!(chunk_eq(&d[3].source, 3, &["brown"]) && chunk_eq(&d[3].target, 3, &["down"]));
}

#[test]
fn test_diff_patch_issue189_problem() {
    let original = fixture_text("text/issue_189_insert_original.txt");
    let revised = fixture_text("text/issue_189_insert_revised.txt");
    let patch = diff(&java_split(&original, "\n"), &java_split(&revised, "\n"));
    assert_eq!(patch.deltas().len(), 1);
}
