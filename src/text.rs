//! Differences over whole texts: line diffs of `\n`-separated text and character-level inline diffs.

use crate::java;
use crate::myers::diff;
use crate::patch::{Chunk, Delta, Patch};

fn lines(text: &str) -> Vec<String> {
    java::split(text, "\n").into_iter().map(str::to_string).collect()
}

/// Line differences between two texts split at `\n` the way `String.split("\n")` splits them.
pub fn diff_text(source: &str, target: &str) -> Patch {
    diff(&lines(source), &lines(target))
}

/// Character-level differences, by UTF-16 code unit as the reference splits a string into its `char`s;
/// each side of every delta is folded into one string, and positions count code units.
pub fn diff_inline(original: &str, revised: &str) -> Patch {
    let unit_patch = diff(&java::utf16_units(original), &java::utf16_units(revised));
    let mut patch = Patch::new();
    for delta in unit_patch.deltas() {
        patch.add_delta(Delta::new(
            delta.kind,
            Chunk::new(delta.source.position, fold(&delta.source.lines)),
            Chunk::new(delta.target.position, fold(&delta.target.lines)),
        ));
    }
    patch
}

fn fold(units: &[u16]) -> Vec<String> {
    if units.is_empty() { Vec::new() } else { vec![java::from_utf16(units)] }
}
