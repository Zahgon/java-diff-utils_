//! Deltas, patches and their application.

use std::fmt;

/// The kind of a delta.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DeltaType {
    Change,
    Delete,
    Insert,
    Equal,
}

/// A run of elements (lines, by default) at a zero-based position of the original or revised sequence.
/// `change_position` lists the positions of the changed elements of the run where the parser that
/// built it records them (`None` for chunks computed by a diff).
#[derive(Clone, Debug)]
pub struct Chunk<T = String> {
    pub position: usize,
    pub lines: Vec<T>,
    pub change_position: Option<Vec<usize>>,
}

/// Two chunks are equal when their positions and elements are; the recorded change positions do not count.
impl<T: PartialEq> PartialEq for Chunk<T> {
    fn eq(&self, other: &Chunk<T>) -> bool {
        self.position == other.position && self.lines == other.lines
    }
}

impl<T: Eq> Eq for Chunk<T> {}

impl<T: Clone + PartialEq> Chunk<T> {
    pub fn new(position: usize, lines: Vec<T>) -> Chunk<T> {
        Chunk { position, lines, change_position: None }
    }

    pub(crate) fn with_changes(position: usize, lines: Vec<T>, change_position: Vec<usize>) -> Chunk<T> {
        Chunk { position, lines, change_position: Some(change_position) }
    }

    pub fn size(&self) -> usize {
        self.lines.len()
    }

    /// The position of the last element (`position + size - 1`, so -1 for an empty run at 0).
    pub fn last(&self) -> isize {
        self.position as isize + self.size() as isize - 1
    }

    /// Whether this chunk fits `target` at `position`, ignoring `fuzz` elements at each end of the run;
    /// `INVALID_RANGE` when the comparison reaches an element before the start of `target`.
    pub fn verify_chunk(&self, target: &[T], fuzz: usize, position: isize) -> Result<VerifyChunk, PatchFailed> {
        match self.verify(target, fuzz, position) {
            Verify::Ok => Ok(VerifyChunk::Ok),
            Verify::OutOfTarget => Ok(VerifyChunk::PositionOutOfTarget),
            Verify::Mismatch => Ok(VerifyChunk::ContentDoesNotMatchTarget),
            Verify::NegativeIndex => Err(PatchFailed::new(INVALID_RANGE)),
        }
    }

    /// Check this chunk against `target` at `position`, ignoring `fuzz` lines at each end.
    fn verify(&self, target: &[T], fuzz: usize, position: isize) -> Verify {
        let size = self.size() as isize;
        let fuzz_i = fuzz as isize;
        let len = target.len() as isize;
        let last = position + size - 1;
        if position + fuzz_i > len || last - fuzz_i >= len {
            return Verify::OutOfTarget;
        }
        let end = self.size().saturating_sub(fuzz);
        for i in fuzz..end {
            let index = position + i as isize;
            if index < 0 {
                return Verify::NegativeIndex;
            }
            if target[index as usize] != self.lines[i] {
                return Verify::Mismatch;
            }
        }
        Verify::Ok
    }
}

/// One difference between two texts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Delta<T = String> {
    pub kind: DeltaType,
    pub source: Chunk<T>,
    pub target: Chunk<T>,
}

impl<T: Clone + PartialEq> Delta<T> {
    pub fn new(kind: DeltaType, source: Chunk<T>, target: Chunk<T>) -> Delta<T> {
        Delta { kind, source, target }
    }

    fn apply(&self, target: &mut Vec<T>) -> Result<(), PatchFailed> {
        let position = self.source.position;
        match self.kind {
            DeltaType::Equal => {}
            DeltaType::Insert => {
                if position > target.len() {
                    return Err(PatchFailed::new(INVALID_RANGE));
                }
                target.splice(position..position, self.target.lines.iter().cloned());
            }
            DeltaType::Delete | DeltaType::Change => {
                let end = position + self.source.size();
                if end > target.len() {
                    return Err(PatchFailed::new(INVALID_RANGE));
                }
                let replacement = if self.kind == DeltaType::Change { self.target.lines.clone() } else { Vec::new() };
                target.splice(position..end, replacement);
            }
        }
        Ok(())
    }

    fn restore(&self, target: &mut Vec<T>) {
        let position = self.target.position.min(target.len());
        let end = (position + self.target.size()).min(target.len());
        match self.kind {
            DeltaType::Equal => {}
            DeltaType::Insert => {
                target.drain(position..end);
            }
            DeltaType::Delete => {
                target.splice(position..position, self.source.lines.iter().cloned());
            }
            DeltaType::Change => {
                target.splice(position..end, self.source.lines.iter().cloned());
            }
        }
    }

    /// Apply this delta at `position`, keeping `fuzz` lines at each end of the hunk as found.
    fn apply_fuzzy_at(&self, target: &mut Vec<T>, fuzz: usize, position: isize) -> Result<(), PatchFailed> {
        match self.kind {
            DeltaType::Equal => return Ok(()),
            DeltaType::Insert | DeltaType::Delete => return Err(PatchFailed::new(UNSUPPORTED)),
            DeltaType::Change => {}
        }
        let start = position + fuzz as isize;
        let source_end = self.source.size() as isize - fuzz as isize;
        let removed = (source_end - fuzz as isize).max(0) as usize;
        if start < 0 || start as usize + removed > target.len() {
            return Err(PatchFailed::new(INVALID_RANGE));
        }
        let start = start as usize;
        let kept_end = self.target.size() as isize - fuzz as isize;
        if fuzz as isize > kept_end {
            return Err(PatchFailed::new(INVALID_RANGE));
        }
        let replacement = self.target.lines[fuzz..kept_end as usize].to_vec();
        target.splice(start..start + removed, replacement);
        Ok(())
    }
}

const UNSUPPORTED: &str = "UNSUPPORTED";
pub(crate) const INVALID_RANGE: &str = "INVALID_RANGE";

/// Why a patch did not apply, and the index in `deltas()` of the delta that was rejected, when the failure is a
/// rejected delta.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PatchFailed {
    pub reason: String,
    pub delta: Option<usize>,
}

impl PatchFailed {
    pub fn new(reason: &str) -> PatchFailed {
        PatchFailed { reason: reason.to_string(), delta: None }
    }
}

impl fmt::Display for PatchFailed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "could not apply patch due to {}", self.reason)
    }
}

impl std::error::Error for PatchFailed {}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Verify {
    Ok,
    OutOfTarget,
    Mismatch,
    NegativeIndex,
}

impl Verify {
    fn reason(self) -> &'static str {
        match self {
            Verify::Ok => "OK",
            Verify::OutOfTarget => "POSITION_OUT_OF_TARGET",
            Verify::Mismatch => "CONTENT_DOES_NOT_MATCH_TARGET",
            Verify::NegativeIndex => INVALID_RANGE,
        }
    }
}

/// Why a delta does not fit where it is applied, as handed to a [`ConflictOutput`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerifyChunk {
    Ok,
    PositionOutOfTarget,
    ContentDoesNotMatchTarget,
}

/// Handles a delta that does not fit: it may change the partly patched `result` and carry on, or fail the
/// whole application with an error, which the apply call returns unchanged.
pub trait ConflictOutput<T> {
    fn process_conflict(&self, verify: VerifyChunk, delta: &Delta<T>, result: &mut Vec<T>) -> Result<(), PatchFailed>;
}

/// What happens when a delta does not fit the text it is applied to.
enum Conflicts<'a, T> {
    Fail,
    Output(&'a dyn ConflictOutput<T>),
}

impl<T> Conflicts<'_, T> {
    /// `index` is the position of `delta` in the patch, reported when the delta is rejected.
    fn resolve(&self, verdict: Verify, delta: &Delta<T>, index: usize, result: &mut Vec<T>) -> Result<(), PatchFailed> {
        let verify = match verdict {
            Verify::Ok => VerifyChunk::Ok,
            Verify::OutOfTarget => VerifyChunk::PositionOutOfTarget,
            Verify::Mismatch => VerifyChunk::ContentDoesNotMatchTarget,
            Verify::NegativeIndex => return Err(PatchFailed { reason: verdict.reason().to_string(), delta: Some(index) }),
        };
        match self {
            Conflicts::Fail => Err(PatchFailed { reason: verdict.reason().to_string(), delta: Some(index) }),
            Conflicts::Output(output) => output.process_conflict(verify, delta, result),
        }
    }
}

/// The merge-conflict output: the lines a delta expected become a conflict block (what was found, then what
/// the patch expected, between merge markers).
struct MergeConflicts;

impl ConflictOutput<String> for MergeConflicts {
    fn process_conflict(&self, _verify: VerifyChunk, delta: &Delta, result: &mut Vec<String>) -> Result<(), PatchFailed> {
        mark_conflict(delta, result)
    }
}

fn mark_conflict(delta: &Delta, result: &mut Vec<String>) -> Result<(), PatchFailed> {
    let position = delta.source.position;
    if result.len() <= position {
        return Err(PatchFailed::new(UNSUPPORTED));
    }
    let end = position + delta.source.size();
    if end > result.len() {
        return Err(PatchFailed::new(INVALID_RANGE));
    }
    let found: Vec<String> = result.drain(position..end).collect();
    let mut block = Vec::with_capacity(found.len() + delta.source.size() + 3);
    block.push("<<<<<< HEAD".to_string());
    block.extend(found);
    block.push("======".to_string());
    block.extend(delta.source.lines.iter().cloned());
    block.push(">>>>>>> PATCH".to_string());
    result.splice(position..position, block);
    Ok(())
}

/// Where fuzzy application put one delta: its index in `deltas()`, the position its search found
/// and the fuzz it was applied with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placement {
    pub delta: usize,
    pub position: usize,
    pub fuzz: usize,
}

/// An ordered list of deltas, kept sorted by source position (stable for equal positions).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Patch<T = String> {
    deltas: Vec<Delta<T>>,
}

impl<T> Default for Patch<T> {
    fn default() -> Patch<T> {
        Patch { deltas: Vec::new() }
    }
}

struct FuzzySearch {
    last_patch_end: isize,
    current_fuzz: usize,
    default_position: isize,
    before_out_of_range: bool,
    after_out_of_range: bool,
}

impl<T: Clone + PartialEq> Patch<T> {
    pub fn new() -> Patch<T> {
        Patch::default()
    }

    pub fn add_delta(&mut self, delta: Delta<T>) {
        let at = self.deltas.partition_point(|d| d.source.position <= delta.source.position);
        self.deltas.insert(at, delta);
    }

    pub fn deltas(&self) -> &[Delta<T>] {
        &self.deltas
    }

    /// Apply every delta exactly where it was recorded; on failure `target` is left unchanged.
    pub fn apply_to(&self, target: &mut Vec<T>) -> Result<(), PatchFailed> {
        self.apply_strict(target, Conflicts::Fail)
    }

    /// [`Patch::apply_to`] with `output` handling every delta that does not fit, as `applyTo` does after
    /// `withConflictOutput(output)`.
    pub fn apply_to_with_conflict_output(&self, target: &mut Vec<T>, output: &dyn ConflictOutput<T>) -> Result<(), PatchFailed> {
        self.apply_strict(target, Conflicts::Output(output))
    }

    /// Apply every delta, searching outward from its expected position and relaxing up to
    /// `max_fuzz` context lines at each end; on failure `target` is left unchanged.
    pub fn apply_fuzzy(&self, target: &mut Vec<T>, max_fuzz: usize) -> Result<(), PatchFailed> {
        self.apply_fuzzy_with(target, max_fuzz, Conflicts::Fail).map(|_| ())
    }

    /// [`Patch::apply_fuzzy`], also reporting where each delta was applied, in application order.
    pub fn apply_fuzzy_report(&self, target: &mut Vec<T>, max_fuzz: usize) -> Result<Vec<Placement>, PatchFailed> {
        self.apply_fuzzy_with(target, max_fuzz, Conflicts::Fail)
    }

    /// [`Patch::apply_fuzzy`] with `output` handling every delta that is found nowhere.
    pub fn apply_fuzzy_with_conflict_output(
        &self,
        target: &mut Vec<T>,
        max_fuzz: usize,
        output: &dyn ConflictOutput<T>,
    ) -> Result<(), PatchFailed> {
        self.apply_fuzzy_with(target, max_fuzz, Conflicts::Output(output)).map(|_| ())
    }

    fn apply_strict(&self, target: &mut Vec<T>, conflicts: Conflicts<'_, T>) -> Result<(), PatchFailed> {
        let mut result = target.clone();
        for (index, delta) in self.deltas.iter().enumerate().rev() {
            let verdict = delta.source.verify(&result, 0, delta.source.position as isize);
            if verdict == Verify::Ok {
                delta.apply(&mut result)?;
            } else {
                conflicts.resolve(verdict, delta, index, &mut result)?;
            }
        }
        *target = result;
        Ok(())
    }

    fn apply_fuzzy_with(&self, target: &mut Vec<T>, max_fuzz: usize, conflicts: Conflicts<'_, T>) -> Result<Vec<Placement>, PatchFailed> {
        let mut result = target.clone();
        let mut placements = Vec::with_capacity(self.deltas.len());
        let mut search = FuzzySearch {
            last_patch_end: -1,
            current_fuzz: 0,
            default_position: 0,
            before_out_of_range: false,
            after_out_of_range: false,
        };
        let mut shift: isize = 0;
        for (index, delta) in self.deltas.iter().enumerate() {
            search.default_position = delta.source.position as isize + shift;
            match find_fuzzy(&mut search, &result, delta, max_fuzz)? {
                Some(position) => {
                    delta.apply_fuzzy_at(&mut result, search.current_fuzz, position)?;
                    placements.push(Placement { delta: index, position: position as usize, fuzz: search.current_fuzz });
                    shift = position - delta.source.position as isize;
                    search.last_patch_end = delta.source.position as isize + delta.source.size() as isize - 1 + shift;
                }
                None => conflicts.resolve(Verify::Mismatch, delta, index, &mut result)?,
            }
        }
        *target = result;
        Ok(placements)
    }

    /// Undo the patch on a text it was applied to.
    pub fn restore(&self, target: &mut Vec<T>) {
        for delta in self.deltas.iter().rev() {
            delta.restore(target);
        }
    }
}

impl Patch<String> {
    /// Like [`Patch::apply_to`], but a delta that does not fit becomes a conflict block.
    pub fn apply_with_merge_conflicts(&self, target: &mut Vec<String>) -> Result<(), PatchFailed> {
        self.apply_strict(target, Conflicts::Output(&MergeConflicts))
    }

    /// Like [`Patch::apply_fuzzy`], but a delta that is found nowhere becomes a conflict block.
    pub fn apply_fuzzy_with_merge_conflicts(&self, target: &mut Vec<String>, max_fuzz: usize) -> Result<(), PatchFailed> {
        self.apply_fuzzy_with(target, max_fuzz, Conflicts::Output(&MergeConflicts)).map(|_| ())
    }
}

fn checked(verdict: Verify) -> Result<bool, PatchFailed> {
    match verdict {
        Verify::NegativeIndex => Err(PatchFailed::new(verdict.reason())),
        other => Ok(other == Verify::Ok),
    }
}

fn find_fuzzy<T: Clone + PartialEq>(search: &mut FuzzySearch, result: &[T], delta: &Delta<T>, max_fuzz: usize) -> Result<Option<isize>, PatchFailed> {
    for fuzz in 0..=max_fuzz {
        search.current_fuzz = fuzz;
        let found = find_with_fuzz(search, result, delta, fuzz)?;
        if found >= 0 {
            return Ok(Some(found));
        }
    }
    Ok(None)
}

/// The position found for one fuzz level; a negative value means none, and a match at a
/// negative position counts as none, exactly like a miss.
fn find_with_fuzz<T: Clone + PartialEq>(search: &mut FuzzySearch, result: &[T], delta: &Delta<T>, fuzz: usize) -> Result<isize, PatchFailed> {
    if checked(delta.source.verify(result, fuzz, search.default_position))? {
        return Ok(search.default_position);
    }
    search.before_out_of_range = false;
    search.after_out_of_range = false;
    let mut more: isize = 0;
    loop {
        let found = find_with_more(search, result, delta, fuzz, more)?;
        if found >= 0 {
            return Ok(found);
        }
        if search.before_out_of_range && search.after_out_of_range {
            return Ok(-1);
        }
        more += 1;
    }
}

fn find_with_more<T: Clone + PartialEq>(search: &mut FuzzySearch, result: &[T], delta: &Delta<T>, fuzz: usize, more: isize) -> Result<isize, PatchFailed> {
    let fuzz_i = fuzz as isize;
    if !search.before_out_of_range && search.default_position - more + fuzz_i <= search.last_patch_end {
        search.before_out_of_range = true;
    }
    if !search.after_out_of_range && (result.len() as isize) < search.default_position + more + delta.source.size() as isize - fuzz_i {
        search.after_out_of_range = true;
    }
    if !search.before_out_of_range && checked(delta.source.verify(result, fuzz, search.default_position - more))? {
        return Ok(search.default_position - more);
    }
    if !search.after_out_of_range && checked(delta.source.verify(result, fuzz, search.default_position + more))? {
        return Ok(search.default_position + more);
    }
    Ok(-1)
}

/// Writes `[a, b, c]`, the form of a Java list's `toString`.
pub(crate) fn write_list<T: fmt::Display>(f: &mut fmt::Formatter<'_>, items: &[T]) -> fmt::Result {
    f.write_str("[")?;
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            f.write_str(", ")?;
        }
        write!(f, "{item}")?;
    }
    f.write_str("]")
}

impl fmt::Display for DeltaType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            DeltaType::Change => "CHANGE",
            DeltaType::Delete => "DELETE",
            DeltaType::Insert => "INSERT",
            DeltaType::Equal => "EQUAL",
        })
    }
}

impl fmt::Display for VerifyChunk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            VerifyChunk::Ok => "OK",
            VerifyChunk::PositionOutOfTarget => "POSITION_OUT_OF_TARGET",
            VerifyChunk::ContentDoesNotMatchTarget => "CONTENT_DOES_NOT_MATCH_TARGET",
        })
    }
}

impl<T: fmt::Display> fmt::Display for Chunk<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[position: {}, size: {}, lines: ", self.position, self.lines.len())?;
        write_list(f, &self.lines)?;
        f.write_str("]")
    }
}

impl<T: fmt::Display> fmt::Display for Delta<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (name, shown) = match self.kind {
            DeltaType::Change => ("ChangeDelta", &self.source.lines),
            DeltaType::Delete => ("DeleteDelta", &self.source.lines),
            DeltaType::Insert => ("InsertDelta", &self.target.lines),
            DeltaType::Equal => ("EqualDelta", &self.source.lines),
        };
        write!(f, "[{name}, position: {}, lines: ", self.source.position)?;
        write_list(f, shown)?;
        if self.kind == DeltaType::Change {
            f.write_str(" to ")?;
            write_list(f, &self.target.lines)?;
        }
        f.write_str("]")
    }
}

impl<T: fmt::Display> fmt::Display for Patch<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Patch{deltas=")?;
        write_list(f, &self.deltas)?;
        f.write_str("}")
    }
}

/// `DiffUtils.patch`: `patch` applied strictly to a copy of `original`.
pub fn patch<T: Clone + PartialEq>(original: &[T], patch: &Patch<T>) -> Result<Vec<T>, PatchFailed> {
    let mut result = original.to_vec();
    patch.apply_to(&mut result)?;
    Ok(result)
}

/// `DiffUtils.unpatch`: `patch` restored on a copy of `revised`.
pub fn unpatch<T: Clone + PartialEq>(revised: &[T], patch: &Patch<T>) -> Vec<T> {
    let mut result = revised.to_vec();
    patch.restore(&mut result);
    result
}
