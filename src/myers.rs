//! The two Myers difference algorithms of the reference library: the greedy one, keeping its
//! choice of path on every diagonal so that equal-cost edit scripts come out the same way, and
//! the linear-space one with its middle-snake recursion and run merging.

use std::rc::Rc;

use crate::java;
use crate::patch::{Chunk, Delta, DeltaType, Patch, PatchFailed};

/// Receives the progress of a difference computation, as `DiffAlgorithmListener` does.
pub trait DiffAlgorithmListener {
    /// Called once before the computation starts.
    fn diff_start(&mut self);
    /// Called for every step of the computation with the step's value and the maximum.
    fn diff_step(&mut self, value: usize, max: usize);
    /// Called once after the computation ends.
    fn diff_end(&mut self);
}

/// The listener used when nobody listens.
struct Silent;

impl DiffAlgorithmListener for Silent {
    fn diff_start(&mut self) {}
    fn diff_step(&mut self, _value: usize, _max: usize) {}
    fn diff_end(&mut self) {}
}

/// One edit of a script, as half-open ranges of the original and revised sequences.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Change {
    pub kind: DeltaType,
    pub start_original: usize,
    pub end_original: usize,
    pub start_revised: usize,
    pub end_revised: usize,
}

/// A difference algorithm: computes the edit script that turns `source` into `target`.
pub trait DiffAlgorithm<T = String> {
    /// The changes of the edit script, reporting progress to `listener` when one is given.
    fn compute_diff(&self, source: &[T], target: &[T], listener: Option<&mut dyn DiffAlgorithmListener>)
    -> Vec<Change>;
}

/// The greedy Myers algorithm.
#[derive(Clone, Copy, Debug, Default)]
pub struct MyersDiff;

/// The linear-space Myers algorithm.
#[derive(Clone, Copy, Debug, Default)]
pub struct MyersDiffWithLinearSpace;

impl<T: PartialEq> DiffAlgorithm<T> for MyersDiff {
    fn compute_diff(&self, source: &[T], target: &[T], listener: Option<&mut dyn DiffAlgorithmListener>)
    -> Vec<Change> {
        match listener {
            Some(listener) => greedy_changes_with(source, target, &same_line, listener),
            None => greedy_changes(source, target, &same_line),
        }
    }
}

impl<T: PartialEq> DiffAlgorithm<T> for MyersDiffWithLinearSpace {
    fn compute_diff(&self, source: &[T], target: &[T], listener: Option<&mut dyn DiffAlgorithmListener>)
    -> Vec<Change> {
        match listener {
            Some(listener) => linear_space_changes(source, target, &same_line, listener),
            None => linear_space_changes(source, target, &same_line, &mut Silent),
        }
    }
}

struct Node {
    i: isize,
    j: isize,
    snake: bool,
    bootstrap: bool,
    prev: Option<Rc<Node>>,
}

fn previous_snake(node: &Rc<Node>) -> Option<Rc<Node>> {
    if node.bootstrap {
        return None;
    }
    if !node.snake
        && let Some(prev) = &node.prev
    {
        return previous_snake(prev);
    }
    Some(node.clone())
}

fn make(i: isize, j: isize, snake: bool, bootstrap: bool, prev: Option<Rc<Node>>) -> Rc<Node> {
    let prev = if snake { prev } else { prev.as_ref().and_then(previous_snake) };
    Rc::new(Node { i, j, snake, bootstrap, prev })
}

fn build_path<T>(
    orig: &[T],
    rev: &[T],
    equal: &dyn Fn(&T, &T) -> bool,
    listener: &mut dyn DiffAlgorithmListener,
) -> Option<Rc<Node>> {
    let n = orig.len() as isize;
    let m = rev.len() as isize;
    let max = n + m + 1;
    let size = (1 + 2 * max) as usize;
    let middle = (size / 2) as isize;
    let mut diagonal: Vec<Option<Rc<Node>>> = vec![None; size];
    diagonal[(middle + 1) as usize] = Some(make(0, -1, true, true, None));
    for d in 0..max {
        listener.diff_step(d as usize, max as usize);
        let mut k = -d;
        while k <= d {
            let kmiddle = (middle + k) as usize;
            let kplus = kmiddle + 1;
            let kminus = kmiddle - 1;
            let minus_i = diagonal[kminus].as_ref().map(|p| p.i);
            let plus_i = diagonal[kplus].as_ref().map(|p| p.i);
            let take_plus = k == -d || (k != d && minus_i < plus_i);
            let (mut i, prev) = if take_plus {
                let p = diagonal[kplus].clone()?;
                (p.i, p)
            } else {
                let p = diagonal[kminus].clone()?;
                (p.i + 1, p)
            };
            diagonal[kminus] = None;
            let mut j = i - k;
            let mut node = make(i, j, false, false, Some(prev));
            while i < n && j < m && equal(&orig[i as usize], &rev[j as usize]) {
                i += 1;
                j += 1;
            }
            if i != node.i {
                node = make(i, j, true, false, Some(node));
            }
            diagonal[kmiddle] = Some(node.clone());
            if i >= n && j >= m {
                return Some(node);
            }
            k += 2;
        }
        diagonal[(middle + d - 1) as usize] = None;
    }
    None
}

/// The greedy edit script, in the order the path is walked back (last edit first).
pub(crate) fn greedy_changes<T>(orig: &[T], rev: &[T], equal: &dyn Fn(&T, &T) -> bool) -> Vec<Change> {
    greedy_changes_with(orig, rev, equal, &mut Silent)
}

fn greedy_changes_with<T>(
    orig: &[T],
    rev: &[T],
    equal: &dyn Fn(&T, &T) -> bool,
    listener: &mut dyn DiffAlgorithmListener,
) -> Vec<Change> {
    listener.diff_start();
    let mut path = build_path(orig, rev, equal, listener);
    if path.as_ref().is_some_and(|p| p.snake) {
        path = path.and_then(|p| p.prev.clone());
    }
    let mut changes = Vec::new();
    while let Some(node) = path.clone() {
        let Some(prev) = node.prev.clone() else { break };
        if prev.j < 0 {
            break;
        }
        let (i, j) = (node.i as usize, node.j as usize);
        let (ia, ja) = (prev.i as usize, prev.j as usize);
        let kind = if ia == i && ja != j {
            DeltaType::Insert
        } else if ia != i && ja == j {
            DeltaType::Delete
        } else {
            DeltaType::Change
        };
        changes.push(Change { kind, start_original: ia, end_original: i, start_revised: ja, end_revised: j });
        path = if prev.snake { prev.prev.clone() } else { Some(prev) };
    }
    listener.diff_end();
    changes
}

struct Snake {
    start: isize,
    end: isize,
    diag: isize,
}

struct LinearSpace<'a, T> {
    source: &'a [T],
    target: &'a [T],
    equal: &'a dyn Fn(&T, &T) -> bool,
    listener: &'a mut dyn DiffAlgorithmListener,
    max_index: usize,
    down: Vec<isize>,
    up: Vec<isize>,
    script: Vec<Change>,
}

impl<T> LinearSpace<'_, T> {
    fn same(&self, x: isize, y: isize) -> bool {
        (self.equal)(&self.source[x as usize], &self.target[y as usize])
    }

    fn build_script(&mut self, start1: isize, end1: isize, start2: isize, end2: isize) {
        self.listener.diff_step(((end1 - start1) / 2 + (end2 - start2) / 2) as usize, self.max_index);
        let middle = self.middle_snake(start1, end1, start2, end2);
        let base = match &middle {
            None => true,
            Some(s) => (s.start == end1 && s.diag == end1 - end2) || (s.end == start1 && s.diag == start1 - start2),
        };
        if base {
            let (mut i, mut j) = (start1, start2);
            while i < end1 || j < end2 {
                if i < end1 && j < end2 && self.same(i, j) {
                    i += 1;
                    j += 1;
                } else if end1 - start1 > end2 - start2 {
                    match self.script.last_mut() {
                        Some(last) if last.end_original == i as usize && last.kind == DeltaType::Delete => {
                            last.end_original = i as usize + 1;
                        }
                        _ => self.script.push(Change {
                            kind: DeltaType::Delete,
                            start_original: i as usize,
                            end_original: i as usize + 1,
                            start_revised: j as usize,
                            end_revised: j as usize,
                        }),
                    }
                    i += 1;
                } else {
                    match self.script.last_mut() {
                        Some(last) if last.end_revised == j as usize && last.kind == DeltaType::Insert => {
                            last.end_revised = j as usize + 1;
                        }
                        _ => self.script.push(Change {
                            kind: DeltaType::Insert,
                            start_original: i as usize,
                            end_original: i as usize,
                            start_revised: j as usize,
                            end_revised: j as usize + 1,
                        }),
                    }
                    j += 1;
                }
            }
        } else if let Some(s) = middle {
            self.build_script(start1, s.start, start2, s.start - s.diag);
            self.build_script(s.end, end1, s.end - s.diag, end2);
        }
    }

    fn middle_snake(&mut self, start1: isize, end1: isize, start2: isize, end2: isize) -> Option<Snake> {
        let m = end1 - start1;
        let n = end2 - start2;
        if m == 0 || n == 0 {
            return None;
        }
        let delta = m - n;
        let sum = n + m;
        let offset = (if sum % 2 == 0 { sum } else { sum + 1 }) / 2;
        let at = |v: isize| v as usize;
        self.down[at(1 + offset)] = start1;
        self.up[at(1 + offset)] = end1 + 1;
        for d in 0..=offset {
            let mut k = -d;
            while k <= d {
                let i = k + offset;
                if k == -d || (k != d && self.down[at(i - 1)] < self.down[at(i + 1)]) {
                    self.down[at(i)] = self.down[at(i + 1)];
                } else {
                    self.down[at(i)] = self.down[at(i - 1)] + 1;
                }
                let mut x = self.down[at(i)];
                let mut y = x - start1 + start2 - k;
                while x < end1 && y < end2 && self.same(x, y) {
                    x += 1;
                    self.down[at(i)] = x;
                    y += 1;
                }
                if delta % 2 != 0 && delta - d <= k && k <= delta + d && self.up[at(i - delta)] <= self.down[at(i)] {
                    return Some(self.snake(self.up[at(i - delta)], k + start1 - start2, end1, end2));
                }
                k += 2;
            }
            let mut k = delta - d;
            while k <= delta + d {
                let i = k + offset - delta;
                if k == delta - d || (k != delta + d && self.up[at(i + 1)] <= self.up[at(i - 1)]) {
                    self.up[at(i)] = self.up[at(i + 1)] - 1;
                } else {
                    self.up[at(i)] = self.up[at(i - 1)];
                }
                let mut x = self.up[at(i)] - 1;
                let mut y = x - start1 + start2 - k;
                while x >= start1 && y >= start2 && self.same(x, y) {
                    self.up[at(i)] = x;
                    x -= 1;
                    y -= 1;
                }
                if delta % 2 == 0 && -d <= k && k <= d && self.up[at(i)] <= self.down[at(i + delta)] {
                    return Some(self.snake(self.up[at(i)], k + start1 - start2, end1, end2));
                }
                k += 2;
            }
        }
        None
    }

    fn snake(&self, start: isize, diag: isize, end1: isize, end2: isize) -> Snake {
        let mut end = start;
        while end - diag < end2 && end < end1 && self.same(end, end - diag) {
            end += 1;
        }
        Snake { start, end, diag }
    }
}

fn linear_space_changes<T>(
    source: &[T],
    target: &[T],
    equal: &dyn Fn(&T, &T) -> bool,
    listener: &mut dyn DiffAlgorithmListener,
) -> Vec<Change> {
    listener.diff_start();
    let size = source.len() + target.len() + 2;
    let mut work = LinearSpace {
        source,
        target,
        equal,
        listener: &mut *listener,
        max_index: source.len() + target.len(),
        down: vec![0; size],
        up: vec![0; size],
        script: Vec::new(),
    };
    work.build_script(0, source.len() as isize, 0, target.len() as isize);
    let script = std::mem::take(&mut work.script);
    drop(work);
    listener.diff_end();
    script
}

fn chunk<T: Clone + PartialEq>(data: &[T], start: usize, end: usize) -> Chunk<T> {
    Chunk::new(start, data[start..end].to_vec())
}

/// Whether every chunk `generate` takes lies within its sequence, as `List.subList` requires.
fn changes_fit<T>(original: &[T], revised: &[T], changes: &[Change], include_equals: bool) -> bool {
    let fits = |start: usize, end: usize, len: usize| start <= end && end <= len;
    let mut ordered = changes.to_vec();
    if include_equals {
        ordered.sort_by_key(|c| c.start_original);
    }
    let (mut start_original, mut start_revised) = (0, 0);
    for change in &ordered {
        if include_equals
            && start_original < change.start_original
            && !(fits(start_original, change.start_original, original.len())
                && fits(start_revised, change.start_revised, revised.len()))
        {
            return false;
        }
        if !fits(change.start_original, change.end_original, original.len())
            || !fits(change.start_revised, change.end_revised, revised.len())
        {
            return false;
        }
        start_original = change.end_original;
        start_revised = change.end_revised;
    }
    !(include_equals
        && start_original < original.len()
        && !(fits(start_original, original.len(), original.len()) && fits(start_revised, revised.len(), revised.len())))
}

impl<T: Clone + PartialEq> Patch<T> {
    /// The patch of an edit script, as `Patch.generate` builds it: changes of kind `Equal` add no delta, and
    /// with `include_equal_parts` the changes are ordered by original start and every untouched run between
    /// them becomes an equal delta. `INVALID_RANGE` when a change does not fit its sequences.
    pub fn generate(original: &[T], revised: &[T], changes: &[Change], include_equal_parts: bool)
    -> Result<Patch<T>, PatchFailed> {
        if changes_fit(original, revised, changes, include_equal_parts) {
            Ok(generate(original, revised, changes, include_equal_parts))
        } else {
            Err(PatchFailed::new(crate::patch::INVALID_RANGE))
        }
    }
}

/// [`diff`] with a caller-chosen algorithm, reporting progress to `listener` when one is given.
pub fn diff_with_algorithm<T: Clone + PartialEq>(
    original: &[T],
    revised: &[T],
    algorithm: &dyn DiffAlgorithm<T>,
    listener: Option<&mut dyn DiffAlgorithmListener>,
    include_equal_parts: bool,
) -> Result<Patch<T>, PatchFailed> {
    Patch::generate(original, revised, &algorithm.compute_diff(original, revised, listener), include_equal_parts)
}

/// Turn an edit script whose changes fit their sequences into a patch, optionally with equal deltas for the
/// untouched runs.
fn generate<T: Clone + PartialEq>(original: &[T], revised: &[T], changes: &[Change], include_equals: bool) -> Patch<T> {
    let mut ordered = changes.to_vec();
    if include_equals {
        ordered.sort_by_key(|c| c.start_original);
    }
    let mut patch = Patch::new();
    let (mut start_original, mut start_revised) = (0, 0);
    for change in &ordered {
        if include_equals && start_original < change.start_original {
            patch.add_delta(Delta::new(
                DeltaType::Equal,
                chunk(original, start_original, change.start_original),
                chunk(revised, start_revised, change.start_revised),
            ));
        }
        if change.kind != DeltaType::Equal {
            patch.add_delta(Delta::new(
                change.kind,
                chunk(original, change.start_original, change.end_original),
                chunk(revised, change.start_revised, change.end_revised),
            ));
        }
        start_original = change.end_original;
        start_revised = change.end_revised;
    }
    if include_equals && start_original < original.len() {
        patch.add_delta(Delta::new(
            DeltaType::Equal,
            chunk(original, start_original, original.len()),
            chunk(revised, start_revised, revised.len()),
        ));
    }
    patch
}

fn same_line<T: PartialEq>(a: &T, b: &T) -> bool {
    a == b
}

/// The greedy Myers patch under a caller-chosen notion of equal lines.
pub(crate) fn diff_by<T: Clone + PartialEq>(original: &[T], revised: &[T], equal: &dyn Fn(&T, &T) -> bool) -> Patch<T> {
    generate(original, revised, &greedy_changes(original, revised, equal), false)
}

/// Compute the deltas that turn `original` into `revised` with the greedy Myers algorithm.
pub fn diff<T: Clone + PartialEq>(original: &[T], revised: &[T]) -> Patch<T> {
    generate(original, revised, &greedy_changes(original, revised, &same_line), false)
}

/// Like [`diff`], with an equal delta for every untouched run between the edits.
pub fn diff_with_equal_parts<T: Clone + PartialEq>(original: &[T], revised: &[T]) -> Patch<T> {
    generate(original, revised, &greedy_changes(original, revised, &same_line), true)
}

/// Compute the deltas that turn `original` into `revised` with the linear-space Myers algorithm.
pub fn diff_linear_space<T: Clone + PartialEq>(original: &[T], revised: &[T]) -> Patch<T> {
    generate(original, revised, &linear_space_changes(original, revised, &same_line, &mut Silent), false)
}

/// [`diff`] reporting its progress to `listener`.
pub fn diff_with_listener<T: Clone + PartialEq>(original: &[T], revised: &[T], listener: &mut dyn DiffAlgorithmListener) -> Patch<T> {
    generate(original, revised, &greedy_changes_with(original, revised, &same_line, listener), false)
}

/// [`diff_linear_space`] reporting its progress to `listener`.
pub fn diff_linear_space_with_listener<T: Clone + PartialEq>(
    original: &[T],
    revised: &[T],
    listener: &mut dyn DiffAlgorithmListener,
) -> Patch<T> {
    generate(original, revised, &linear_space_changes(original, revised, &same_line, listener), false)
}

/// Line differences of two texts split as `String.split("\n")` splits them, reporting progress.
pub fn diff_text_with_listener(source: &str, target: &str, listener: &mut dyn DiffAlgorithmListener) -> Patch {
    let lines = |text: &str| -> Vec<String> { java::split(text, "\n").into_iter().map(str::to_string).collect() };
    diff_with_listener(&lines(source), &lines(target), listener)
}

/// [`diff`] with a caller-chosen notion of equal elements; `equalizer` gets an original element, then a revised one.
pub fn diff_with_equalizer<T: Clone + PartialEq>(original: &[T], revised: &[T], equalizer: &dyn Fn(&T, &T) -> bool) -> Patch<T> {
    generate(original, revised, &greedy_changes(original, revised, equalizer), false)
}

/// [`diff_linear_space`] with a caller-chosen notion of equal lines.
pub fn diff_linear_space_with_equalizer<T: Clone + PartialEq>(
    original: &[T],
    revised: &[T],
    equalizer: &dyn Fn(&T, &T) -> bool,
) -> Patch<T> {
    generate(original, revised, &linear_space_changes(original, revised, equalizer, &mut Silent), false)
}
