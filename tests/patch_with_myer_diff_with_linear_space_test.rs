mod common;

use common::{applied_with_conflicts, lines};
use diffutils::{MyersDiffWithLinearSpace, diff_with_algorithm};

#[test]
fn test_patch_change_with_exception_processor() {
    let mut from = lines(&["aaa", "bbb", "ccc", "ddd"]);
    let to = lines(&["aaa", "bxb", "cxc", "ddd"]);
    let patch = diff_with_algorithm(&from, &to, &MyersDiffWithLinearSpace, None, false).expect("changes fit");
    from[2] = "CDC".to_string();
    let data = applied_with_conflicts(&patch, &from).expect("conflicts are written into the result");
    assert_eq!(data.len(), 11);
    assert_eq!(
        data,
        lines(&["aaa", "bxb", "cxc", "<<<<<< HEAD", "bbb", "CDC", "======", "bbb", "ccc", ">>>>>>> PATCH", "ddd"])
    );
}
