mod common;

use common::{fixture_lines, java_split, lines, row_string, rows_string, tag_name};
use diffutils::{
    DiffRow, DiffRowGenerator, DiffTag, InlineDeltaMergeInfo, MyersDiffWithLinearSpace, diff_with_algorithm,
    merge_inline_deltas,
};

fn split(content: &str) -> Vec<String> {
    java_split(content, "\n")
}

fn tilde(_: DiffTag, _: bool) -> String {
    "~".to_string()
}

fn stars(_: DiffTag, _: bool) -> String {
    "**".to_string()
}

fn tags(rows: &[DiffRow]) -> Vec<&'static str> {
    rows.iter().map(|r| tag_name(&r.tag)).collect()
}

/// `DiffRowGenerator.splitStringPreserveDelimiter(line, Pattern.compile(","))`.
fn split_preserving_commas(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut run = String::new();
    for c in line.chars() {
        if c == ',' {
            if !run.is_empty() {
                out.push(std::mem::take(&mut run));
            }
            out.push(",".to_string());
        } else {
            run.push(c);
        }
    }
    if !run.is_empty() {
        out.push(run);
    }
    out
}

/// Rows whose equal lines agree on both sides, as every generated row list must.
fn assert_consistent(rows: &[DiffRow]) {
    assert!(!rows.is_empty());
    for row in rows {
        if matches!(row.tag, DiffTag::Equal) {
            assert_eq!(row.old_line, row.new_line);
        }
    }
}

fn assert_inline_diff_result(generator: &DiffRowGenerator, original: &str, revised: &str, expected: &str) {
    let rows = generator.generate_diff_rows(&lines(&[original]), &lines(&[revised]));
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].old_line, expected);
}

const ISSUE65_OLD: [&str; 2] = ["Ich möchte nicht mit einem Bot sprechen.", "Ich soll das schon wieder wiederholen?"];
const ISSUE65_NEW: [&str; 2] =
    ["Ich möchte nicht mehr mit dir sprechen. Leite mich weiter.", "Kannst du mich zum Kundendienst weiterleiten?"];

const ISSUE129_OLD: [&str; 7] =
    ["apple1", "apple2", "apple3", "A man named Frankenstein abc to Switzerland for cookies!", "banana1", "banana2", "banana3"];
const ISSUE129_NEW: [&str; 9] = [
    "apple1",
    "apple2",
    "apple3",
    "A man named Frankenstein",
    "xyz",
    "to Switzerland for cookies!",
    "banana1",
    "banana2",
    "banana3",
];

fn old_tag_129(tag: DiffTag, opening: bool) -> String {
    if opening { format!("==old{}==>", tag_name(&tag)) } else { "<==old==".to_string() }
}

fn new_tag_129(tag: DiffTag, opening: bool) -> String {
    if opening { format!("==new{}==>", tag_name(&tag)) } else { "<==new==".to_string() }
}

#[test]
fn test_generator_default() {
    let generator = DiffRowGenerator::builder().column_width(i32::MAX).build();
    let rows = generator.generate_diff_rows(&split("anything \n \nother"), &split("anything\n\nother"));
    assert_eq!(rows.len(), 3);
}

#[test]
fn test_normalize_list() {
    let generator = DiffRowGenerator::builder().build();
    let rows = generator.generate_diff_rows(&lines(&["\ttest"]), &lines(&["\ttest"]));
    assert_eq!(rows[0].old_line, "    test");
    assert_eq!(rows[0].new_line, "    test");
}

#[test]
fn test_generator_default2() {
    let generator = DiffRowGenerator::builder().column_width(0).build();
    let rows = generator.generate_diff_rows(&split("anything \n \nother"), &split("anything\n\nother"));
    assert_eq!(rows.len(), 3);
}

#[test]
fn test_generator_inline_diff() {
    let generator = DiffRowGenerator::builder().show_inline_diffs(true).column_width(i32::MAX).build();
    let rows = generator.generate_diff_rows(&split("anything \n \nother"), &split("anything\n\nother"));
    assert_eq!(rows.len(), 3);
    assert!(rows[0].old_line.find("<span").is_some_and(|i| i > 0));
}

#[test]
fn test_generator_ignore_whitespaces() {
    let generator = DiffRowGenerator::builder().ignore_white_spaces(true).column_width(i32::MAX).build();
    let rows = generator
        .generate_diff_rows(&split("anything \n \nother\nmore lines"), &split("anything\n\nother\nsome more lines"));
    assert_eq!(rows.len(), 4);
    assert_eq!(tags(&rows), ["EQUAL", "EQUAL", "EQUAL", "CHANGE"]);
}

#[test]
fn test_generator_with_word_wrap() {
    let generator = DiffRowGenerator::builder().column_width(5).build();
    let rows = generator.generate_diff_rows(&split("anything \n \nother"), &split("anything\n\nother"));
    assert_eq!(rows.len(), 3);
    assert_eq!(row_string(&rows[0]), "[CHANGE,anyth<br/>ing ,anyth<br/>ing]");
    assert_eq!(row_string(&rows[1]), "[CHANGE, ,]");
    assert_eq!(row_string(&rows[2]), "[EQUAL,other,other]");
}

#[test]
fn test_generator_with_merge() {
    let generator = DiffRowGenerator::builder().show_inline_diffs(true).merge_original_revised(true).build();
    let rows = generator.generate_diff_rows(&split("anything \n \nother"), &split("anything\n\nother"));
    assert_eq!(rows.len(), 3);
    assert_eq!(row_string(&rows[0]), "[CHANGE,anything<span class=\"editOldInline\"> </span>,anything]");
    assert_eq!(row_string(&rows[1]), "[CHANGE,<span class=\"editOldInline\"> </span>,]");
    assert_eq!(row_string(&rows[2]), "[EQUAL,other,other]");
}

#[test]
fn test_generator_with_merge2() {
    let generator = DiffRowGenerator::builder().show_inline_diffs(true).merge_original_revised(true).build();
    let rows = generator.generate_diff_rows(&lines(&["Test"]), &lines(&["ester"]));
    assert_eq!(rows.len(), 1);
    assert_eq!(
        row_string(&rows[0]),
        "[CHANGE,<span class=\"editOldInline\">T</span>est<span class=\"editNewInline\">er</span>,ester]"
    );
}

#[test]
fn test_generator_with_merge3() {
    let generator = DiffRowGenerator::builder().show_inline_diffs(true).merge_original_revised(true).build();
    let rows = generator.generate_diff_rows(&split("test\nanything \n \nother"), &split("anything\n\nother\ntest\ntest2"));
    assert_eq!(rows.len(), 6);
    assert_eq!(row_string(&rows[0]), "[CHANGE,<span class=\"editOldInline\">test</span>,anything]");
    assert_eq!(row_string(&rows[1]), "[CHANGE,anything<span class=\"editOldInline\"> </span>,]");
    assert_eq!(row_string(&rows[2]), "[DELETE,<span class=\"editOldInline\"> </span>,]");
    assert_eq!(row_string(&rows[3]), "[EQUAL,other,other]");
    assert_eq!(row_string(&rows[4]), "[INSERT,<span class=\"editNewInline\">test</span>,test]");
    assert_eq!(row_string(&rows[5]), "[INSERT,<span class=\"editNewInline\">test2</span>,test2]");
}

#[test]
fn test_generator_with_merge_by_word4() {
    let generator =
        DiffRowGenerator::builder().show_inline_diffs(true).merge_original_revised(true).inline_diff_by_word(true).build();
    let rows = generator.generate_diff_rows(&lines(&["Test"]), &lines(&["ester"]));
    assert_eq!(rows.len(), 1);
    assert_eq!(
        row_string(&rows[0]),
        "[CHANGE,<span class=\"editOldInline\">Test</span><span class=\"editNewInline\">ester</span>,ester]"
    );
}

#[test]
fn test_generator_with_merge_by_word5() {
    let generator = DiffRowGenerator::builder()
        .show_inline_diffs(true)
        .merge_original_revised(true)
        .inline_diff_by_word(true)
        .column_width(80)
        .build();
    let rows = generator.generate_diff_rows(&lines(&["Test feature"]), &lines(&["ester feature best"]));
    assert_eq!(rows.len(), 1);
    assert_eq!(
        row_string(&rows[0]),
        "[CHANGE,<span class=\"editOldInline\">Test</span><span class=\"editNewInline\">ester</span> <br/>feature<span class=\"editNewInline\"> best</span>,ester feature best]"
    );
}

/// The word splitter's tokens, made visible by marking every token of a line that shares nothing with the other.
fn word_tokens(line: &str) -> String {
    let generator = DiffRowGenerator::builder()
        .show_inline_diffs(true)
        .inline_diff_by_word(true)
        .old_tag(|_, open| if open { "[".to_string() } else { "]".to_string() })
        .new_tag(|_, open| if open { "{".to_string() } else { "}".to_string() })
        .merge_whitespace_equalities(false)
        .build();
    let rows = generator.generate_diff_rows(&lines(&[line]), &lines(&[""]));
    rows[0].old_line.clone()
}

#[test]
fn test_split_string() {
    let generator = DiffRowGenerator::builder()
        .show_inline_diffs(true)
        .inline_diff_by_word(true)
        .old_tag(tilde)
        .new_tag(stars)
        .build();
    let rows = generator.generate_diff_rows(&lines(&["test,test2"]), &lines(&["test,test3"]));
    assert_eq!(rows[0].old_line, "test,~test2~");
    assert_eq!(rows[0].new_line, "test,**test3**");
    assert_eq!(word_tokens("test,test2"), "[test,test2]");
}

#[test]
fn test_split_string2() {
    let generator = DiffRowGenerator::builder()
        .show_inline_diffs(true)
        .inline_diff_by_word(true)
        .old_tag(tilde)
        .new_tag(stars)
        .build();
    let rows = generator.generate_diff_rows(&lines(&["test , test2"]), &lines(&["test , test3"]));
    assert_eq!(rows[0].old_line, "test , ~test2~");
    let rows = generator.generate_diff_rows(&lines(&["test , test2"]), &lines(&["test ; test2"]));
    assert_eq!(rows[0].old_line, "test ~,~ test2");
}

#[test]
fn test_split_string3() {
    let generator = DiffRowGenerator::builder()
        .show_inline_diffs(true)
        .inline_diff_by_word(true)
        .old_tag(tilde)
        .new_tag(stars)
        .build();
    let rows = generator.generate_diff_rows(&lines(&["test,test2,"]), &lines(&["test,test2"]));
    assert_eq!(rows[0].old_line, "test,test2~,~");
}

#[test]
fn test_generator_example1() {
    let generator = DiffRowGenerator::builder()
        .show_inline_diffs(true)
        .merge_original_revised(true)
        .inline_diff_by_word(true)
        .old_tag(tilde)
        .new_tag(stars)
        .build();
    let rows = generator.generate_diff_rows(&lines(&["This is a test senctence."]), &lines(&["This is a test for diffutils."]));
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].old_line, "This is a test ~senctence~**for diffutils**.");
}

#[test]
fn test_generator_example2() {
    let generator =
        DiffRowGenerator::builder().show_inline_diffs(true).inline_diff_by_word(true).old_tag(tilde).new_tag(stars).build();
    let rows = generator.generate_diff_rows(
        &lines(&["This is a test senctence.", "This is the second line.", "And here is the finish."]),
        &lines(&["This is a test for diffutils.", "This is the second line."]),
    );
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].old_line, "This is a test ~senctence~.");
    assert_eq!(rows[0].new_line, "This is a test **for diffutils**.");
}

#[test]
fn test_generator_unchanged() {
    let generator = DiffRowGenerator::builder().column_width(5).report_lines_unchanged(true).build();
    let rows = generator.generate_diff_rows(&split("anything \n \nother"), &split("anything\n\nother"));
    assert_eq!(rows.len(), 3);
    assert_eq!(row_string(&rows[0]), "[CHANGE,anything ,anything]");
    assert_eq!(row_string(&rows[1]), "[CHANGE, ,]");
    assert_eq!(row_string(&rows[2]), "[EQUAL,other,other]");
}

#[test]
fn test_generator_issue14() {
    let generator = DiffRowGenerator::builder()
        .show_inline_diffs(true)
        .merge_original_revised(true)
        .inline_diff_by_splitter(split_preserving_commas)
        .old_tag(tilde)
        .new_tag(stars)
        .build();
    let rows = generator.generate_diff_rows(&lines(&["J. G. Feldstein, Chair"]), &lines(&["T. P. Pastor, Chair"]));
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].old_line, "~J. G. Feldstein~**T. P. Pastor**, Chair");
}

#[test]
fn test_generator_issue15() {
    let generator =
        DiffRowGenerator::builder().show_inline_diffs(true).inline_diff_by_word(true).old_tag(tilde).new_tag(stars).build();
    let rows = generator.generate_diff_rows(&fixture_lines("mocks/issue15_1.txt"), &fixture_lines("mocks/issue15_2.txt"));
    assert_eq!(rows.len(), 9);
    for row in &rows {
        if !row.old_line.starts_with("TABLE_NAME") {
            assert!(row.new_line.starts_with("**ACTIONS_C16913**"));
            assert!(row.old_line.starts_with("~ACTIONS_C1700"));
        }
    }
}

#[test]
fn test_generator_issue22() {
    let generator =
        DiffRowGenerator::builder().show_inline_diffs(true).inline_diff_by_word(true).old_tag(tilde).new_tag(stars).build();
    let rows = generator.generate_diff_rows(
        &split("This is a test senctence."),
        &split("This is a test for diffutils.\nThis is the second line."),
    );
    assert_eq!(
        rows_string(&rows),
        "[[CHANGE,This is a test ~senctence~.,This is a test **for diffutils**.], [INSERT,,**This is the second line.**]]"
    );
}

#[test]
fn test_generator_issue22_2() {
    let generator =
        DiffRowGenerator::builder().show_inline_diffs(true).inline_diff_by_word(true).old_tag(tilde).new_tag(stars).build();
    let rows = generator.generate_diff_rows(
        &split("This is a test for diffutils.\nThis is the second line."),
        &split("This is a test senctence."),
    );
    assert_eq!(
        rows_string(&rows),
        "[[CHANGE,This is a test ~for diffutils~.,This is a test **senctence**.], [DELETE,~This is the second line.~,]]"
    );
}

#[test]
fn test_generator_issue22_3() {
    let generator =
        DiffRowGenerator::builder().show_inline_diffs(true).inline_diff_by_word(true).old_tag(tilde).new_tag(stars).build();
    let rows = generator.generate_diff_rows(
        &split("This is a test senctence."),
        &split("This is a test for diffutils.\nThis is the second line.\nAnd one more."),
    );
    assert_eq!(
        rows_string(&rows),
        "[[CHANGE,This is a test ~senctence~.,This is a test **for diffutils**.], [INSERT,,**This is the second line.**], [INSERT,,**And one more.**]]"
    );
}

#[test]
fn test_generator_issue41_default_normalizer() {
    let rows = DiffRowGenerator::builder().build().generate_diff_rows(&lines(&["<"]), &lines(&["<"]));
    assert_eq!(rows_string(&rows), "[[EQUAL,&lt;,&lt;]]");
}

#[test]
fn test_generator_issue41_user_normalizer() {
    let generator = DiffRowGenerator::builder().line_normalizer(|s| s.replace('\t', "    ")).build();
    assert_eq!(rows_string(&generator.generate_diff_rows(&lines(&["<"]), &lines(&["<"]))), "[[EQUAL,<,<]]");
    assert_eq!(rows_string(&generator.generate_diff_rows(&lines(&["\t<"]), &lines(&["<"]))), "[[CHANGE,    <,<]]");
}

#[test]
fn test_generation_issue44report_lines_unchanged_problem() {
    let generator = DiffRowGenerator::builder()
        .show_inline_diffs(true)
        .report_lines_unchanged(true)
        .old_tag(|_, _| "~~".to_string())
        .new_tag(stars)
        .build();
    let rows = generator.generate_diff_rows(&lines(&["<dt>To do</dt>"]), &lines(&["<dt>Done</dt>"]));
    assert_eq!(rows_string(&rows), "[[CHANGE,<dt>~~T~~o~~ do~~</dt>,<dt>**D**o**ne**</dt>]]");
}

fn whitespace_word_generator() -> DiffRowGenerator {
    DiffRowGenerator::builder()
        .show_inline_diffs(true)
        .inline_diff_by_word(true)
        .ignore_white_spaces(true)
        .merge_original_revised(true)
        .old_tag(tilde)
        .new_tag(stars)
        .build()
}

#[test]
fn test_ignore_whitespace_issue66() {
    let rows = whitespace_word_generator().generate_diff_rows(&lines(&["This\tis\ta\ttest."]), &lines(&["This is a test"]));
    assert_eq!(rows[0].old_line, "This    is    a    test~.~");
}

#[test]
fn test_ignore_whitespace_issue66_2() {
    let rows = whitespace_word_generator().generate_diff_rows(&lines(&["This  is  a  test."]), &lines(&["This is a test"]));
    assert_eq!(rows[0].old_line, "This  is  a  test~.~");
}

#[test]
fn test_ignore_whitespace_issue64() {
    let rows = whitespace_word_generator()
        .generate_diff_rows(&split("test\n\ntestline"), &split("A new text line\n\nanother one"));
    let old: Vec<&str> = rows.iter().map(|r| r.old_line.as_str()).collect();
    assert_eq!(old, ["~test~**A new text line**", "", "~testline~**another one**"]);
}

#[test]
fn test_replace_diffs_issue63() {
    let generator = DiffRowGenerator::builder()
        .show_inline_diffs(true)
        .inline_diff_by_word(true)
        .merge_original_revised(true)
        .old_tag(tilde)
        .new_tag(stars)
        .process_diffs(|s| s.replace(' ', "/"))
        .build();
    let rows = generator.generate_diff_rows(&lines(&["This  is  a  test."]), &lines(&["This is a test"]));
    assert_eq!(rows[0].old_line, "This~//~**/**is~//~**/**a~//~**/**test~.~");
}

#[test]
fn test_problem_too_many_diff_rows_issue65() {
    let generator = DiffRowGenerator::builder()
        .show_inline_diffs(true)
        .report_lines_unchanged(true)
        .old_tag(tilde)
        .new_tag(stars)
        .merge_original_revised(true)
        .inline_diff_by_word(false)
        .replace_original_linefeed_in_changes_with_spaces(true)
        .build();
    assert_eq!(generator.generate_diff_rows(&lines(&ISSUE65_OLD), &lines(&ISSUE65_NEW)).len(), 2);
}

#[test]
fn test_problem_too_many_diff_rows_issue65_no_merge() {
    let generator = DiffRowGenerator::builder()
        .show_inline_diffs(true)
        .report_lines_unchanged(true)
        .old_tag(tilde)
        .new_tag(stars)
        .merge_original_revised(false)
        .inline_diff_by_word(false)
        .build();
    assert_eq!(generator.generate_diff_rows(&lines(&ISSUE65_OLD), &lines(&ISSUE65_NEW)).len(), 2);
}

#[test]
fn test_problem_too_many_diff_rows_issue65_diff_by_word() {
    let generator = DiffRowGenerator::builder()
        .show_inline_diffs(true)
        .report_lines_unchanged(true)
        .old_tag(tilde)
        .new_tag(stars)
        .merge_original_revised(true)
        .inline_diff_by_word(true)
        .build();
    assert_eq!(generator.generate_diff_rows(&lines(&ISSUE65_OLD), &lines(&ISSUE65_NEW)).len(), 2);
}

#[test]
fn test_problem_too_many_diff_rows_issue65_no_inline_diff() {
    let generator = DiffRowGenerator::builder()
        .show_inline_diffs(false)
        .report_lines_unchanged(true)
        .old_tag(tilde)
        .new_tag(stars)
        .merge_original_revised(true)
        .inline_diff_by_word(false)
        .build();
    assert_eq!(generator.generate_diff_rows(&lines(&ISSUE65_OLD), &lines(&ISSUE65_NEW)).len(), 2);
}

#[test]
fn test_linefeed_in_standard_tags_with_line_width_issue81() {
    let original = split(
        "American bobtail jaguar. American bobtail bombay but turkish angora and tomcat.\nRussian blue leopard. Lion. Tabby scottish fold for russian blue, so savannah yet lynx. Tomcat singapura, cheetah.\nBengal tiger panther but singapura but bombay munchkin for cougar.",
    );
    let revised = split(
        "bobtail jaguar. American bobtail turkish angora and tomcat.\nRussian blue leopard. Lion. Tabby scottish folded for russian blue, so savannah yettie? lynx. Tomcat singapura, cheetah.\nBengal tiger panther but singapura but bombay munchkin for cougar. And more.",
    );
    let generator =
        DiffRowGenerator::builder().show_inline_diffs(true).ignore_white_spaces(true).column_width(100).build();
    let rows = generator.generate_diff_rows(&original, &revised);
    assert_eq!(tags(&rows), ["CHANGE", "CHANGE", "CHANGE"]);
    assert!(rows.iter().all(|r| !r.old_line.contains('\n') && !r.new_line.contains('\n')));
}

#[test]
fn test_issue86_wrong_inline_diff() {
    let original = fixture_lines("text/issue_86_original.txt");
    let revised = fixture_lines("text/issue_86_revised.txt");
    let generator = DiffRowGenerator::builder()
        .show_inline_diffs(true)
        .merge_original_revised(true)
        .inline_diff_by_word(true)
        .old_tag(tilde)
        .new_tag(stars)
        .build();
    let rows = generator.generate_diff_rows(&original, &revised);
    assert_consistent(&rows);
    for row in &rows {
        if matches!(row.tag, DiffTag::Change | DiffTag::Delete) {
            assert!(row.old_line.contains('~') || row.old_line.contains("**"));
        }
    }
}

#[test]
fn test_correct_change_issue114() {
    let generator =
        DiffRowGenerator::builder().show_inline_diffs(false).inline_diff_by_word(true).old_tag(tilde).new_tag(stars).build();
    let rows = generator.generate_diff_rows(&lines(&["A", "B", "C", "D", "E"]), &lines(&["a", "C", "", "E"]));
    assert_eq!(tags(&rows), ["CHANGE", "DELETE", "EQUAL", "CHANGE", "EQUAL"]);
}

#[test]
fn test_correct_change_issue114_2() {
    let generator =
        DiffRowGenerator::builder().show_inline_diffs(true).inline_diff_by_word(true).old_tag(tilde).new_tag(stars).build();
    let rows = generator.generate_diff_rows(&lines(&["A", "B", "C", "D", "E"]), &lines(&["a", "C", "", "E"]));
    assert_eq!(tags(&rows), ["CHANGE", "DELETE", "EQUAL", "CHANGE", "EQUAL"]);
    assert_eq!(row_string(&rows[1]), "[DELETE,~B~,]");
}

#[test]
fn test_issue119_wrong_context_length() {
    let original = fixture_lines("text/issue_119_original.txt");
    let revised = fixture_lines("text/issue_119_revised.txt");
    let generator = DiffRowGenerator::builder()
        .show_inline_diffs(true)
        .merge_original_revised(true)
        .inline_diff_by_word(true)
        .old_tag(tilde)
        .new_tag(stars)
        .build();
    let rows = generator.generate_diff_rows(&original, &revised);
    assert_consistent(&rows);
    assert!(rows.len() >= original.len().max(revised.len()));
}

#[test]
fn test_issue129_with_delta_decompression() {
    let rows = DiffRowGenerator::builder()
        .show_inline_diffs(true)
        .old_tag(old_tag_129)
        .new_tag(new_tag_129)
        .build()
        .generate_diff_rows(&lines(&ISSUE129_OLD), &lines(&ISSUE129_NEW));
    assert_eq!(tags(&rows).join(" "), "EQUAL EQUAL EQUAL CHANGE INSERT INSERT EQUAL EQUAL EQUAL");
}

#[test]
fn test_issue129_skip_delta_decompression() {
    let rows = DiffRowGenerator::builder()
        .show_inline_diffs(true)
        .decompress_deltas(false)
        .old_tag(old_tag_129)
        .new_tag(new_tag_129)
        .build()
        .generate_diff_rows(&lines(&ISSUE129_OLD), &lines(&ISSUE129_NEW));
    assert_eq!(tags(&rows).join(" "), "EQUAL EQUAL EQUAL CHANGE CHANGE CHANGE EQUAL EQUAL EQUAL");
}

#[test]
fn test_issue129_skip_whitespace_changes() {
    let original = fixture_lines("text/issue129_1.txt");
    let revised = fixture_lines("text/issue129_2.txt");
    let generator = DiffRowGenerator::builder()
        .show_inline_diffs(true)
        .merge_original_revised(true)
        .inline_diff_by_word(true)
        .ignore_white_spaces(true)
        .old_tag(old_tag_129)
        .new_tag(new_tag_129)
        .build();
    assert_eq!(generator.generate_diff_rows(&original, &revised).len(), 13);
}

#[test]
fn test_generator_with_whitespace_delta_merge() {
    let generator = DiffRowGenerator::builder()
        .show_inline_diffs(true)
        .merge_original_revised(true)
        .inline_diff_by_word(true)
        .old_tag(tilde)
        .new_tag(stars)
        .line_normalizer(|s| s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;"))
        .merge_whitespace_equalities(true)
        .build();
    assert_inline_diff_result(&generator, "No diff", "No diff", "No diff");
    assert_inline_diff_result(&generator, " x whitespace before diff", " y whitespace before diff", " ~x~**y** whitespace before diff");
    assert_inline_diff_result(&generator, "Whitespace after diff x ", "Whitespace after diff y ", "Whitespace after diff ~x~**y** ");
    assert_inline_diff_result(&generator, "Diff x x between", "Diff y y between", "Diff ~x x~**y y** between");
    assert_inline_diff_result(&generator, "Hello \t world", "Hi \t universe", "~Hello \t world~**Hi \t universe**");
    assert_inline_diff_result(
        &generator,
        "The quick brown fox jumps over the lazy dog",
        "A lazy dog jumps over a fox",
        "~The quick brown fox ~**A lazy dog **jumps over ~the lazy dog~**a fox**",
    );
}

#[test]
fn test_generator_with_merging_deltas_for_short_equalities() {
    let generator = DiffRowGenerator::builder()
        .show_inline_diffs(true)
        .merge_original_revised(true)
        .inline_diff_by_word(true)
        .old_tag(tilde)
        .new_tag(stars)
        .inline_delta_merger(|info: &InlineDeltaMergeInfo| {
            merge_inline_deltas(info, |equalities| equalities.iter().map(|e| e.encode_utf16().count()).sum::<usize>() < 6)
        })
        .build();
    assert_inline_diff_result(&generator, "No diff", "No diff", "No diff");
    assert_inline_diff_result(&generator, "aaa bbb ccc", "xxx bbb zzz", "~aaa bbb ccc~**xxx bbb zzz**");
    assert_inline_diff_result(&generator, "aaa bbbb ccc", "xxx bbbb zzz", "~aaa~**xxx** bbbb ~ccc~**zzz**");
}

#[test]
fn test_issue188_hang_on_examples() {
    let original = fixture_lines("text/test-zip/old.html");
    let revised = fixture_lines("text/test-zip/new.html");
    let generator = DiffRowGenerator::builder()
        .line_normalizer(|line| line.to_string())
        .show_inline_diffs(true)
        .merge_original_revised(true)
        .inline_diff_by_word(true)
        .decompress_deltas(true)
        .old_tag(|_, open| if open { "<s style=\"background-color: #bbbbbb\">".to_string() } else { "</s>".to_string() })
        .new_tag(|_, open| if open { "<b style=\"background-color: #aaffaa\">".to_string() } else { "</b>".to_string() })
        .build();
    let patch = diff_with_algorithm(&original, &revised, &MyersDiffWithLinearSpace, None, false).expect("changes fit");
    let rows = generator.generate_diff_rows_from_patch(&original, &patch).expect("the patch fits the original");
    assert_consistent(&rows);
}
