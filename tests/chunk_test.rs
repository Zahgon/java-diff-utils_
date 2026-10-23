
use diffutils::{Chunk, Delta, DeltaType, Patch};

fn chars(text: &str) -> Vec<String> {
    text.chars().map(String::from).collect()
}

/// A patch replacing the characters "test" at `position`, so applying it verifies that chunk against a target.
fn test_patch(position: usize) -> Patch {
    let mut patch = Patch::new();
    patch.add_delta(Delta {
        kind: DeltaType::Change,
        source: Chunk::new(position, chars("test")),
        target: Chunk::new(position, chars("TEST")),
    });
    patch
}

fn strict(position: usize, target: &str) -> String {
    let mut work = chars(target);
    match test_patch(position).apply_to(&mut work) {
        Ok(()) => "OK".to_string(),
        Err(e) => e.reason,
    }
}

fn fuzzy(position: usize, target: &str, fuzz: usize) -> String {
    let mut work = chars(target);
    match test_patch(position).apply_fuzzy(&mut work, fuzz) {
        Ok(()) => "OK".to_string(),
        Err(e) => e.reason,
    }
}

#[test]
fn verify_chunk() {
    assert_eq!(strict(7, "prefix test suffix"), "OK");
    assert_eq!(strict(7, "prefix  es  suffix"), "CONTENT_DOES_NOT_MATCH_TARGET");

    assert_eq!(strict(6, "short test suffix"), "OK");
    assert_eq!(strict(8, "loonger test suffix"), "OK");
    assert_eq!(strict(6, "prefix test suffix"), "CONTENT_DOES_NOT_MATCH_TARGET");
    assert_eq!(strict(8, "prefix test suffix"), "CONTENT_DOES_NOT_MATCH_TARGET");

    assert_eq!(fuzzy(7, "prefix test suffix", 1), "OK");
    assert_eq!(fuzzy(7, "prefix  es  suffix", 1), "OK");
    assert_ne!(fuzzy(7, "prefix      suffix", 1), "OK");
}

#[test]
fn verify_chunk_at_the_end_of_the_target() {
    assert_eq!(strict(7, "prefix test"), "OK");
    assert_eq!(strict(7, "prefix tes"), "POSITION_OUT_OF_TARGET");
    assert_eq!(strict(7, "prefix te"), "POSITION_OUT_OF_TARGET");
}
