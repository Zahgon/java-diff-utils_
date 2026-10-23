//! Diff and patch toolkit over lists of lines: the Myers difference algorithms, unified diff
//! emission and parsing, multi-file diff reading and writing, strict, fuzzy and conflict-marking
//! patch application, and side-by-side diff rows.

mod java;
mod myers;
mod patch;
mod rows;
mod text;
mod unified;
mod unifieddiff;

pub use myers::{
    Change, DiffAlgorithm, DiffAlgorithmListener, MyersDiff, MyersDiffWithLinearSpace, diff, diff_with_algorithm, diff_linear_space, diff_linear_space_with_equalizer, diff_linear_space_with_listener,
    diff_text_with_listener, diff_with_equal_parts, diff_with_equalizer, diff_with_listener,
};
pub use patch::{Chunk, ConflictOutput, Delta, DeltaType, Patch, PatchFailed, Placement, VerifyChunk, patch, unpatch};
pub use rows::{DiffRow, DiffRowGenerator, DiffRowGeneratorBuilder, DiffTag, InlineDeltaMergeInfo, merge_inline_deltas};
pub use text::{diff_inline, diff_text};
pub use unified::{generate_original_and_diff, generate_unified_diff, parse_unified_diff, parse_unified_diff_checked};
pub use unifieddiff::{ParseFailed, UnifiedDiff, UnifiedDiffFile, read_unified_diff, write_unified_diff};
