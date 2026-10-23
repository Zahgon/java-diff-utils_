mod common;

use common::{change_positions, fixture_text, patch_string};
use diffutils::{UnifiedDiff, parse_unified_diff, read_unified_diff};

fn read(name: &str) -> UnifiedDiff {
    let text = fixture_text(&format!("unifieddiff/{name}"));
    read_unified_diff(&text).unwrap_or_else(|e| panic!("{name}: {} at line {}", e.reason, e.line))
}

fn from_files(diff: &UnifiedDiff) -> Vec<Option<&str>> {
    diff.files.iter().map(|f| f.from_file.as_deref()).collect()
}

fn check_jsqlparser(diff: &UnifiedDiff) {
    assert_eq!(diff.files.len(), 2);
    let file1 = &diff.files[0];
    assert_eq!(file1.from_file.as_deref(), Some("src/main/jjtree/net/sf/jsqlparser/parser/JSqlParserCC.jjt"));
    assert_eq!(file1.patch.deltas().len(), 3);
    assert_eq!(diff.tail.as_deref(), Some("2.17.1.windows.2\n"));
}

#[test]
fn test_simple_parse() {
    check_jsqlparser(&read("jsqlparser_patch_1.diff"));
}

#[test]
fn test_parse_diff_block() {
    let name = "src/test/java/net/sf/jsqlparser/statement/select/SelectTest.java";
    let text = format!("diff --git a/{name} b/{name}\n--- a/{name}\n+++ b/{name}\n@@ -1 +1 @@\n-x\n+y\n");
    let diff = read_unified_diff(&text).expect("a well-formed diff");
    let file = &diff.files[0];
    assert_eq!(file.diff_command.as_deref(), Some(format!("diff --git a/{name} b/{name}").as_str()));
    assert_eq!(file.from_file.as_deref(), Some(name));
    assert_eq!(file.to_file.as_deref(), Some(name));
}

/// The hunk header's start numbers, as the reader's chunk pattern captures them (groups 1 and 3).
fn hunk_starts(header: &str) -> (usize, usize) {
    let text = vec!["--- a".to_string(), "+++ b".to_string(), header.to_string(), "-x".to_string(), "+y".to_string()];
    let patch = parse_unified_diff(&text);
    let delta = &patch.deltas()[0];
    (delta.source.position + 1, delta.target.position + 1)
}

#[test]
fn test_chunk_header_parsing() {
    assert_eq!(hunk_starts("@@ -189,6 +189,7 @@ TOKEN: /* SQL Keywords. prefixed with K_ to avoid name clashes */"), (189, 189));
}

#[test]
fn test_chunk_header_parsing2() {
    assert_eq!(hunk_starts("@@ -189,6 +189,7 @@"), (189, 189));
}

#[test]
fn test_chunk_header_parsing3() {
    assert_eq!(hunk_starts("@@ -1,27 +1,27 @@"), (1, 1));
}

#[test]
fn test_simple_parse2() {
    let diff = read("jsqlparser_patch_1.diff");
    check_jsqlparser(&diff);
    let first = &diff.files[0].patch.deltas()[0];
    assert!(!first.source.lines.is_empty());
    assert!(!first.target.lines.is_empty());
}

#[test]
fn test_parse_issue201() {
    let diff = read("jsqlparser_patch_1.diff");
    check_jsqlparser(&diff);
    let first = &diff.files[0].patch.deltas()[0];
    assert!(!first.source.lines.is_empty());
    assert!(!first.target.lines.is_empty());
}

#[test]
fn test_simple_pattern() {
    let diff = read_unified_diff("--- original.txt\n+++ revised.txt\n@@ -1 +1 @@\n-a\n+b\n").expect("a well-formed diff");
    assert_eq!(diff.files[0].to_file.as_deref(), Some("revised.txt"));
}

#[test]
fn test_parse_issue46() {
    let diff = read("problem_diff_issue46.diff");
    assert_eq!(diff.files.len(), 1);
    assert_eq!(diff.files[0].from_file.as_deref(), Some("a.vhd"));
    assert_eq!(diff.files[0].patch.deltas().len(), 1);
    assert_eq!(diff.tail, None);
}

#[test]
fn test_parse_issue33() {
    let diff = read("problem_diff_issue33.diff");
    assert_eq!(diff.files.len(), 1);
    assert_eq!(diff.files[0].from_file.as_deref(), Some("Main.java"));
    assert_eq!(diff.files[0].patch.deltas().len(), 1);
    assert_eq!(diff.tail, None);
    assert_eq!(diff.header, None);
}

#[test]
fn test_parse_issue51() {
    let diff = read("problem_diff_issue51.diff");
    assert_eq!(diff.files.len(), 2);
    assert_eq!(diff.files[0].from_file.as_deref(), Some("f1"));
    assert_eq!(diff.files[0].patch.deltas().len(), 1);
    assert_eq!(diff.files[1].from_file.as_deref(), Some("f2"));
    assert_eq!(diff.files[1].patch.deltas().len(), 1);
    assert_eq!(diff.tail, None);
}

#[test]
fn test_parse_issue79() {
    let diff = read("problem_diff_issue79.diff");
    assert_eq!(diff.files.len(), 1);
    assert_eq!(diff.files[0].from_file.as_deref(), Some("test/Issue.java"));
    assert_eq!(diff.files[0].patch.deltas().len(), 0);
    assert_eq!(diff.tail, None);
    assert_eq!(diff.header, None);
}

#[test]
fn test_parse_issue84() {
    let diff = read("problem_diff_issue84.diff");
    assert_eq!(diff.files.len(), 2);
    assert_eq!(diff.files[0].from_file.as_deref(), Some("config/ant-phase-verify.xml"));
    assert_eq!(diff.files[0].patch.deltas().len(), 1);
    assert_eq!(diff.files[1].from_file.as_deref(), Some("/dev/null"));
    assert_eq!(diff.files[1].patch.deltas().len(), 1);
    assert_eq!(diff.tail.as_deref(), Some("2.7.4"));
    let header = diff.header.as_deref().expect("a mail header");
    assert!(header.starts_with("From b53e612a2ab5ff15d14860e252f84c0f343fe93a Mon Sep 17 00:00:00 2001"));
}

#[test]
fn test_parse_issue85() {
    let diff = read("problem_diff_issue85.diff");
    assert_eq!(diff.files.len(), 1);
    let file1 = &diff.files[0];
    assert_eq!(file1.diff_command.as_deref(), Some("diff -r 83e41b73d115 -r a4438263b228 tests/test-check-pyflakes.t"));
    assert_eq!(file1.from_file.as_deref(), Some("tests/test-check-pyflakes.t"));
    assert_eq!(file1.to_file.as_deref(), Some("tests/test-check-pyflakes.t"));
    assert_eq!(file1.patch.deltas().len(), 1);
    assert_eq!(diff.tail, None);
}

#[test]
fn test_time_stamp_regexp() {
    let stamp = "2019-04-18 13:49:39.516149751 +0200";
    let text = format!("--- a/x {stamp}\n+++ b/x {stamp}\n@@ -1 +1 @@\n-a\n+b\n");
    let diff = read_unified_diff(&text).expect("a well-formed diff");
    assert_eq!(diff.files[0].from_timestamp.as_deref(), Some(stamp));
    assert_eq!(diff.files[0].to_timestamp.as_deref(), Some(stamp));
    assert_eq!(diff.files[0].from_file.as_deref(), Some("x"));
}

#[test]
fn test_parse_issue98() {
    let diff = read("problem_diff_issue98.diff");
    assert_eq!(diff.files.len(), 1);
    let file1 = &diff.files[0];
    assert_eq!(file1.deleted_file_mode.as_deref(), Some("100644"));
    assert_eq!(file1.from_file.as_deref(), Some("src/test/java/se/bjurr/violations/lib/model/ViolationTest.java"));
    assert_eq!(diff.tail.as_deref(), Some("2.25.1"));
}

#[test]
fn test_parse_issue104() {
    let diff = read("problem_diff_parsing_issue104.diff");
    assert_eq!(diff.files.len(), 6);
    let file = &diff.files[2];
    assert_eq!(file.from_file.as_deref(), Some("/dev/null"));
    assert_eq!(file.to_file.as_deref(), Some("doc/samba_data_tool_path.xml.in"));
    assert_eq!(patch_string(&file.patch), "Patch{deltas=[[InsertDelta, position: 0, lines: [@SAMBA_DATA_TOOL@]]]}");
    assert_eq!(diff.tail.as_deref(), Some("2.14.4"));
}

#[test]
fn test_parse_issue107_bazel_diff() {
    let diff = read("01-bazel-strip-unused.patch_issue107.diff");
    assert_eq!(diff.files.len(), 450);
    let file = &diff.files[0];
    assert_eq!(file.from_file.as_deref(), Some("./src/main/java/com/amazonaws/AbortedException.java"));
    assert_eq!(
        file.to_file.as_deref(),
        Some("/home/greg/projects/bazel/third_party/aws-sdk-auth-lite/src/main/java/com/amazonaws/AbortedException.java")
    );
    assert_eq!(diff.files.iter().filter(|f| f.no_newline_at_end).count(), 48);
}

#[test]
fn test_parse_issue107_2() {
    let diff = read("problem_diff_issue107.diff");
    assert_eq!(diff.files.len(), 2);
    assert_eq!(diff.files[0].from_file.as_deref(), Some("Main.java"));
    assert_eq!(diff.files[0].patch.deltas().len(), 1);
}

#[test]
fn test_parse_issue107_3() {
    let diff = read("problem_diff_issue107_3.diff");
    assert_eq!(diff.files.len(), 1);
    assert_eq!(diff.files[0].from_file.as_deref(), Some("Billion laughs attack.md"));
    assert_eq!(diff.files[0].patch.deltas().len(), 1);
}

#[test]
fn test_parse_issue107_4() {
    let diff = read("problem_diff_issue107_4.diff");
    assert_eq!(diff.files.len(), 27);
    assert!(from_files(&diff).contains(&Some("README.md")));
}

#[test]
fn test_parse_issue107_5() {
    let diff = read("problem_diff_issue107_5.diff");
    assert_eq!(diff.files.len(), 22);
    assert!(from_files(&diff).contains(&Some("rt/management/src/test/java/org/apache/cxf/management/jmx/MBServerConnectorFactoryTest.java")));
}

#[test]
fn test_parse_issue110() {
    let diff = read("0001-avahi-python-Use-the-agnostic-DBM-interface.patch");
    assert_eq!(diff.files.len(), 5);
    let file = &diff.files[4];
    assert_eq!(file.similarity_index, Some(87));
    assert_eq!(file.rename_from.as_deref(), Some("service-type-database/build-db.in"));
    assert_eq!(file.rename_to.as_deref(), Some("service-type-database/build-db"));
    assert_eq!(file.from_file.as_deref(), Some("service-type-database/build-db.in"));
    assert_eq!(file.to_file.as_deref(), Some("service-type-database/build-db"));
}

#[test]
fn test_parse_issue117() {
    let diff = read("problem_diff_issue117.diff");
    assert_eq!(diff.files.len(), 2);
    let patch = &diff.files[0].patch;
    let (removed, added) = change_positions(patch, 0);
    assert_eq!(removed, [24, 27]);
    assert_eq!(added, [24, 27]);
    let (removed, added) = change_positions(patch, 1);
    assert_eq!(removed, [64]);
    assert_eq!(added, [64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74]);
}

#[test]
fn test_parse_issue122() {
    let diff = read("problem_diff_issue122.diff");
    assert_eq!(diff.files.len(), 1);
    assert!(from_files(&diff).contains(&Some("coders/wpg.c")));
}

#[test]
fn test_parse_issue123() {
    let diff = read("problem_diff_issue123.diff");
    assert_eq!(diff.files.len(), 2);
    assert!(from_files(&diff).contains(&Some("src/java/main/org/apache/zookeeper/server/FinalRequestProcessor.java")));
}

#[test]
fn test_parse_issue141() {
    let diff = read("problem_diff_issue141.diff");
    assert_eq!(diff.files[0].from_file.as_deref(), Some("a.txt"));
    assert_eq!(diff.files[0].to_file.as_deref(), Some("a1.txt"));
}

#[test]
fn test_parse_issue182add() {
    assert_eq!(read("problem_diff_issue182_add.diff").files[0].binary_added.as_deref(), Some("some-image.png"));
}

#[test]
fn test_parse_issue182delete() {
    assert_eq!(read("problem_diff_issue182_delete.diff").files[0].binary_deleted.as_deref(), Some("some-image.png"));
}

#[test]
fn test_parse_issue182edit() {
    assert_eq!(read("problem_diff_issue182_edit.diff").files[0].binary_edited.as_deref(), Some("some-image.png"));
}

#[test]
fn test_parse_issue182mode() {
    let diff = read("problem_diff_issue182_mode.diff");
    assert_eq!(diff.files[0].old_mode.as_deref(), Some("100644"));
    assert_eq!(diff.files[0].new_mode.as_deref(), Some("100755"));
}

#[test]
fn test_parse_issue193_copy() {
    let diff = read("problem_diff_parsing_issue193.diff");
    assert_eq!(diff.files[0].copy_from.as_deref(), Some("modules/configuration/config/web/pcf/account/AccountContactCV.pcf"));
    assert_eq!(diff.files[0].copy_to.as_deref(), Some("modules/configuration/config/web/pcf/account/AccountContactCV.default.pcf"));
}
