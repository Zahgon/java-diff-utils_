mod common;

use common::{applied, change_delta, lines};
use diffutils::{DiffAlgorithm, MyersDiff, MyersDiffWithLinearSpace, Patch, diff_with_algorithm};

fn algorithms() -> [&'static dyn DiffAlgorithm; 2] {
    [&MyersDiff, &MyersDiffWithLinearSpace]
}

fn check(from: &[&str], to: &[&str]) {
    let (from, to) = (lines(from), lines(to));
    for algorithm in algorithms() {
        let patch = diff_with_algorithm(&from, &to, algorithm, None, false).expect("changes fit");
        assert_eq!(applied(&patch, &from), Ok(to.clone()));
    }
}

#[test]
fn test_patch_insert() {
    check(&["hhh"], &["hhh", "jjj", "kkk", "lll"]);
}

#[test]
fn test_patch_delete() {
    check(&["ddd", "fff", "ggg", "hhh"], &["ggg"]);
}

#[test]
fn test_patch_change() {
    check(&["aaa", "bbb", "ccc", "ddd"], &["aaa", "bxb", "cxc", "ddd"]);
}

#[test]
fn test_patch_serializable() {
    let (from, to) = (lines(&["aaa", "bbb", "ccc", "ddd"]), lines(&["aaa", "bxb", "cxc", "ddd"]));
    for algorithm in algorithms() {
        let patch = diff_with_algorithm(&from, &to, algorithm, None, false).expect("changes fit");
        let mut copy = Patch::new();
        for delta in patch.deltas() {
            let source: Vec<&str> = delta.source.lines.iter().map(String::as_str).collect();
            let target: Vec<&str> = delta.target.lines.iter().map(String::as_str).collect();
            copy.add_delta(change_delta(delta.source.position, &source, delta.target.position, &target));
        }
        assert_eq!(applied(&copy, &from), Ok(to.clone()));
    }
}
