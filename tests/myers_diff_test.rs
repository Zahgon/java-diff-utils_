mod common;

use common::{lines, patch_string};
use diffutils::{DiffAlgorithm, DiffAlgorithmListener, MyersDiff, Patch};

const EXPECTED: &str = "Patch{deltas=[[DeleteDelta, position: 0, lines: [A, B]], [InsertDelta, position: 3, lines: [B]], [DeleteDelta, position: 5, lines: [B]], [InsertDelta, position: 7, lines: [C]]]}";

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
    let changes = MyersDiff.compute_diff(&original, &revised, None);
    let patch = Patch::generate(&original, &revised, &changes, false).expect("changes fit");
    assert_eq!(patch.deltas().len(), 4);
    assert_eq!(patch_string(&patch), EXPECTED);
}

#[test]
fn test_diff_myers_example1_forward_with_listener() {
    let original = lines(&["A", "B", "C", "A", "B", "B", "A"]);
    let revised = lines(&["C", "B", "A", "B", "A", "C"]);
    let mut log = Log(Vec::new());
    let changes = MyersDiff.compute_diff(&original, &revised, Some(&mut log));
    let patch = Patch::generate(&original, &revised, &changes, false).expect("changes fit");
    assert_eq!(patch.deltas().len(), 4);
    assert_eq!(patch_string(&patch), EXPECTED);
    assert_eq!(log.0.len(), 8);
}
