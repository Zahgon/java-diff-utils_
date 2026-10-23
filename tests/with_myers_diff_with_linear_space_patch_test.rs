mod common;

use common::{
    DELTA_FROM, DELTA_TO, applied, applied_fuzzy, applied_with_conflicts, change_delta, fuzzy_apply_pairs, int_range,
    join, lines,
};
use diffutils::{MyersDiffWithLinearSpace, Patch, diff_with_algorithm};

fn linear(from: &[String], to: &[String]) -> Patch {
    diff_with_algorithm(from, to, &MyersDiffWithLinearSpace, None, false).expect("changes fit")
}

#[test]
fn test_patch_insert() {
    let (from, to) = (lines(&["hhh"]), lines(&["hhh", "jjj", "kkk", "lll"]));
    assert_eq!(applied(&linear(&from, &to), &from), Ok(to));
}

#[test]
fn test_patch_delete() {
    let (from, to) = (lines(&["ddd", "fff", "ggg", "hhh"]), lines(&["ggg"]));
    assert_eq!(applied(&linear(&from, &to), &from), Ok(to));
}

#[test]
fn test_patch_change() {
    let (from, to) = (lines(&["aaa", "bbb", "ccc", "ddd"]), lines(&["aaa", "bxb", "cxc", "ddd"]));
    assert_eq!(applied(&linear(&from, &to), &from), Ok(to));
}

#[test]
fn fuzzy_apply() {
    let mut patch = Patch::new();
    patch.add_delta(change_delta(6, &DELTA_FROM, 6, &DELTA_TO));
    let moves = [int_range(6), int_range(3), int_range(9), int_range(0)];
    let pairs = fuzzy_apply_pairs();
    assert_eq!(pairs.len(), 32);
    for (pair_from, pair_to, required) in &pairs {
        for prefix in &moves {
            let from = join(&[prefix, pair_from]);
            let to = join(&[prefix, pair_to]);
            for fuzz in 0..*required {
                assert!(applied_fuzzy(&patch, &from, fuzz).is_err(), "fail for {from:?} -> {to:?} for fuzz {fuzz} required {required}");
            }
            for fuzz in *required..4 {
                assert_eq!(applied_fuzzy(&patch, &from, fuzz), Ok(to.clone()), "with {fuzz}");
            }
        }
    }
}

#[test]
fn fuzzy_apply_two_side_by_side_patches() {
    let mut patch = Patch::new();
    patch.add_delta(change_delta(0, &DELTA_FROM, 0, &DELTA_TO));
    patch.add_delta(change_delta(6, &DELTA_FROM, 6, &DELTA_TO));
    let (from, to) = (lines(&DELTA_FROM), lines(&DELTA_TO));
    assert_eq!(applied_fuzzy(&patch, &join(&[&from, &from]), 0), Ok(join(&[&to, &to])));
}

#[test]
fn fuzzy_apply_to_nearest() {
    let mut patch = Patch::new();
    patch.add_delta(change_delta(0, &DELTA_FROM, 0, &DELTA_TO));
    patch.add_delta(change_delta(10, &DELTA_FROM, 10, &DELTA_TO));
    let (from, to) = (lines(&DELTA_FROM), lines(&DELTA_TO));
    assert_eq!(applied_fuzzy(&patch, &join(&[&from, &from, &from]), 0), Ok(join(&[&to, &from, &to])));
    let one = int_range(1);
    assert_eq!(
        applied_fuzzy(&patch, &join(&[&one, &from, &from, &from]), 0),
        Ok(join(&[&one, &to, &from, &to]))
    );
}

#[test]
fn test_patch_serializable() {
    let (from, to) = (lines(&["aaa", "bbb", "ccc", "ddd"]), lines(&["aaa", "bxb", "cxc", "ddd"]));
    let patch = linear(&from, &to);
    let mut copy = Patch::new();
    for delta in patch.deltas() {
        copy.add_delta(change_delta(
            delta.source.position,
            &delta.source.lines.iter().map(String::as_str).collect::<Vec<_>>(),
            delta.target.position,
            &delta.target.lines.iter().map(String::as_str).collect::<Vec<_>>(),
        ));
    }
    assert_eq!(applied(&copy, &from), Ok(to));
}

#[test]
fn test_patch_change_with_exception_processor() {
    let mut from = lines(&["aaa", "bbb", "ccc", "ddd"]);
    let to = lines(&["aaa", "bxb", "cxc", "ddd"]);
    let patch = linear(&from, &to);
    from[2] = "CDC".to_string();
    let data = applied_with_conflicts(&patch, &from).expect("conflicts are written into the result");
    assert_eq!(data.len(), 11);
    assert_eq!(
        data,
        lines(&["aaa", "bxb", "cxc", "<<<<<< HEAD", "bbb", "CDC", "======", "bbb", "ccc", ">>>>>>> PATCH", "ddd"])
    );
}
