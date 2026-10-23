mod common;

use common::{lines, patch_string};
use diffutils::{DiffAlgorithm, DiffAlgorithmListener, MyersDiffWithLinearSpace, Patch, diff_with_algorithm};

const EXPECTED: &str = "Patch{deltas=[[InsertDelta, position: 0, lines: [C]], [DeleteDelta, position: 0, lines: [A]], [DeleteDelta, position: 2, lines: [C]], [DeleteDelta, position: 5, lines: [B]], [InsertDelta, position: 7, lines: [C]]]}";

struct Log(Vec<String>);

impl DiffAlgorithmListener for Log {
    fn diff_start(&mut self) {
        self.0.push("start".to_string());
    }
    fn diff_step(&mut self, value: usize, max: usize) {
        self.0.push(format!("{value} - {max}"));
    }
    fn diff_end(&mut self) {
        self.0.push("end".to_string());
    }
}

#[test]
fn test_diff_myers_example1_forward() {
    let original = lines(&["A", "B", "C", "A", "B", "B", "A"]);
    let revised = lines(&["C", "B", "A", "B", "A", "C"]);
    let changes = MyersDiffWithLinearSpace.compute_diff(&original, &revised, None);
    let patch = Patch::generate(&original, &revised, &changes, false).expect("changes fit");
    assert_eq!(patch.deltas().len(), 5);
    assert_eq!(patch_string(&patch), EXPECTED);
}

#[test]
fn test_diff_myers_example1_forward_with_listener() {
    let original = lines(&["A", "B", "C", "A", "B", "B", "A"]);
    let revised = lines(&["C", "B", "A", "B", "A", "C"]);
    let mut log = Log(Vec::new());
    let changes = MyersDiffWithLinearSpace.compute_diff(&original, &revised, Some(&mut log));
    let patch = Patch::generate(&original, &revised, &changes, false).expect("changes fit");
    assert_eq!(patch.deltas().len(), 5);
    assert_eq!(patch_string(&patch), EXPECTED);
    assert_eq!(log.0.len(), 11);
}

#[test]
fn test_performance_problems_issue124() {
    let old = lines(&["abcd"]);
    let new: Vec<String> = (0..90000).map(|i| i.to_string()).collect();
    let patch = diff_with_algorithm(&old, &new, &MyersDiffWithLinearSpace, None, false).expect("changes fit");
    let mut patched = old.clone();
    assert!(patch.apply_to(&mut patched).is_ok());
    assert_eq!(patched, new);
}
