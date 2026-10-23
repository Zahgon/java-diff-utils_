mod common;

use common::{applied_with_conflicts, lines};
use diffutils::diff;

#[test]
fn test_patch_change_with_exception_processor() {
    let mut from = lines(&["aaa", "bbb", "ccc", "ddd"]);
    let to = lines(&["aaa", "bxb", "cxc", "ddd"]);
    let patch = diff(&from, &to);
    from[2] = "CDC".to_string();
    let data = applied_with_conflicts(&patch, &from).expect("conflicts are written into the result");
    assert_eq!(data.len(), 9);
    assert_eq!(data, lines(&["aaa", "<<<<<< HEAD", "bbb", "CDC", "======", "bbb", "ccc", ">>>>>>> PATCH", "ddd"]));
}

#[test]
fn test_patch_three_way_issue138() {
    let words = |text: &str| -> Vec<String> { text.split_whitespace().map(str::to_string).collect() };
    let base = words("Imagine there's no heaven");
    let left = words("Imagine there's no HEAVEN");
    let right = words("IMAGINE there's no heaven");
    let right_patch = diff(&base, &right);
    let merged = applied_with_conflicts(&right_patch, &left).expect("the change applies");
    assert_eq!(merged.join(" "), "IMAGINE there's no HEAVEN");
}
