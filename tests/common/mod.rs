//! Helpers shared by the ported tests: line lists, fixtures, and the text forms the Java tests compare against.
#![allow(dead_code)]

use std::path::PathBuf;

use diffutils::{Chunk, Delta, DeltaType, DiffRow, DiffTag, Patch, diff};

pub fn lines(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

pub fn fixture_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(relative)
}

pub fn fixture_text(relative: &str) -> String {
    let path = fixture_path(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("fixture {}: {e}", path.display()))
}

/// The lines of a fixture as `Files.readAllLines` returns them (split at `\n`, `\r` and `\r\n`).
pub fn fixture_lines(relative: &str) -> Vec<String> {
    read_lines(&fixture_text(relative))
}

/// Lines as `BufferedReader.readLine` returns them.
pub fn read_lines(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while !rest.is_empty() {
        match rest.find(['\n', '\r']) {
            Some(end) => {
                out.push(rest[..end].to_string());
                let skip = if rest[end..].starts_with("\r\n") { 2 } else { 1 };
                rest = &rest[end + skip..];
            }
            None => {
                out.push(rest.to_string());
                rest = "";
            }
        }
    }
    out
}

/// `String.split(separator)` for a literal separator: trailing empty strings are dropped.
pub fn java_split(text: &str, separator: &str) -> Vec<String> {
    let mut parts: Vec<String> = text.split(separator).map(str::to_string).collect();
    while parts.len() > 1 && parts.last().is_some_and(|p| p.is_empty()) {
        parts.pop();
    }
    if parts.len() == 1 && parts[0].is_empty() && !text.is_empty() {
        parts.clear();
    }
    parts
}

/// `List.toString()` of a list of strings.
pub fn list_string(items: &[String]) -> String {
    format!("[{}]", items.join(", "))
}

/// `AbstractDelta.toString()` of the Java delta of the same kind.
pub fn delta_string(delta: &Delta) -> String {
    match delta.kind {
        DeltaType::Change => format!(
            "[ChangeDelta, position: {}, lines: {} to {}]",
            delta.source.position,
            list_string(&delta.source.lines),
            list_string(&delta.target.lines)
        ),
        DeltaType::Delete => {
            format!("[DeleteDelta, position: {}, lines: {}]", delta.source.position, list_string(&delta.source.lines))
        }
        DeltaType::Insert => {
            format!("[InsertDelta, position: {}, lines: {}]", delta.source.position, list_string(&delta.target.lines))
        }
        DeltaType::Equal => {
            format!("[EqualDelta, position: {}, lines: {}]", delta.source.position, list_string(&delta.source.lines))
        }
    }
}

/// `Patch.toString()`.
pub fn patch_string(patch: &Patch) -> String {
    let deltas: Vec<String> = patch.deltas().iter().map(delta_string).collect();
    format!("Patch{{deltas=[{}]}}", deltas.join(", "))
}

/// `Chunk.toString()`.
pub fn chunk_string(chunk: &Chunk) -> String {
    format!("[position: {}, size: {}, lines: {}]", chunk.position, chunk.lines.len(), list_string(&chunk.lines))
}

pub fn tag_name(tag: &DiffTag) -> &'static str {
    match tag {
        DiffTag::Insert => "INSERT",
        DiffTag::Delete => "DELETE",
        DiffTag::Change => "CHANGE",
        DiffTag::Equal => "EQUAL",
    }
}

/// `DiffRow.toString()`.
pub fn row_string(row: &DiffRow) -> String {
    format!("[{},{},{}]", tag_name(&row.tag), row.old_line, row.new_line)
}

/// The name of a delta kind, as `DeltaType.name()`.
pub fn kind_name(kind: &DeltaType) -> &'static str {
    match kind {
        DeltaType::Change => "CHANGE",
        DeltaType::Delete => "DELETE",
        DeltaType::Insert => "INSERT",
        DeltaType::Equal => "EQUAL",
    }
}

/// `List<DiffRow>.toString()`.
pub fn rows_string(rows: &[DiffRow]) -> String {
    let items: Vec<String> = rows.iter().map(row_string).collect();
    format!("[{}]", items.join(", "))
}

pub fn chunk_eq(chunk: &Chunk, position: usize, expected: &[&str]) -> bool {
    chunk.position == position && chunk.lines == lines(expected)
}

/// `patch.applyTo(target)`: the patched copy, or the failure reason.
pub fn applied(patch: &Patch, target: &[String]) -> Result<Vec<String>, String> {
    let mut work = target.to_vec();
    patch.apply_to(&mut work).map(|()| work).map_err(|e| e.reason)
}

/// `patch.applyFuzzy(target, maxFuzz)`: the patched copy, or the failure reason.
pub fn applied_fuzzy(patch: &Patch, target: &[String], max_fuzz: usize) -> Result<Vec<String>, String> {
    let mut work = target.to_vec();
    patch.apply_fuzzy(&mut work, max_fuzz).map(|()| work).map_err(|e| e.reason)
}

/// `applyTo` after `withConflictOutput(CONFLICT_PRODUCES_MERGE_CONFLICT)`.
pub fn applied_with_conflicts(patch: &Patch, target: &[String]) -> Result<Vec<String>, String> {
    let mut work = target.to_vec();
    patch.apply_with_merge_conflicts(&mut work).map(|()| work).map_err(|e| e.reason)
}

pub fn change_delta(from_position: usize, from: &[&str], to_position: usize, to: &[&str]) -> Delta {
    Delta {
        kind: DeltaType::Change,
        source: Chunk::new(from_position, lines(from)),
        target: Chunk::new(to_position, lines(to)),
    }
}

pub fn int_range(count: usize) -> Vec<String> {
    (0..count).map(|i| i.to_string()).collect()
}

pub fn join(parts: &[&[String]]) -> Vec<String> {
    parts.iter().flat_map(|p| p.iter().cloned()).collect()
}

pub const DELTA_FROM: [&str; 6] = ["aaa", "bbb", "ccc", "ddd", "eee", "fff"];
pub const DELTA_TO: [&str; 6] = ["aaa", "bbb", "cxc", "dxd", "eee", "fff"];

/// The fuzzy-apply cases of `FuzzyApplyTestDataGenerator`: (from, to, required fuzz), sorted by required fuzz.
pub fn fuzzy_apply_pairs() -> Vec<(Vec<String>, Vec<String>, usize)> {
    let changed = ["axa", "bxb", "czc", "dzd", "exe", "fxf"];
    let mut pairs = Vec::new();
    for i in 0..(1u32 << 6) {
        if i & 0b001100 != 0 && i & 0b001100 != 0b001100 {
            continue;
        }
        let mut from = lines(&DELTA_FROM);
        let mut to = lines(&DELTA_TO);
        for j in 0..6 {
            if i & (1 << j) != 0 {
                from[j] = changed[j].to_string();
                to[j] = changed[j].to_string();
            }
        }
        let required = if i & 0b001100 != 0 {
            3
        } else if i & 0b010010 != 0 {
            2
        } else if i & 0b100001 != 0 {
            1
        } else {
            0
        };
        pairs.push((from, to, required));
    }
    pairs.sort_by_key(|p| p.2);
    pairs
}

/// The 1-based positions of the lines a parsed hunk removes and adds, as `Chunk.getChangePosition` lists them.
pub fn change_positions(patch: &Patch, index: usize) -> (Vec<usize>, Vec<usize>) {
    let delta = &patch.deltas()[index];
    let inner = diff(&delta.source.lines, &delta.target.lines);
    let mut removed = Vec::new();
    let mut added = Vec::new();
    for d in inner.deltas() {
        removed.extend((0..d.source.lines.len()).map(|i| delta.source.position + d.source.position + i + 1));
        added.extend((0..d.target.lines.len()).map(|i| delta.target.position + d.target.position + i + 1));
    }
    (removed, added)
}

/// `UnifiedDiffWriter.write` to a `Writer`: every element the writer hands on, each ended with a newline.
pub fn written_text(diff: &diffutils::UnifiedDiff, original: &[String], context: usize) -> String {
    let provider = |_: Option<&str>| original.to_vec();
    diffutils::write_unified_diff(diff, &provider, context).iter().map(|l| format!("{l}\n")).collect()
}

/// `UnifiedDiff.from(header, tail, UnifiedDiffFile.from(fromFile, toFile, patch))`.
pub fn single_file_diff(header: &str, tail: &str, from: Option<&str>, to: &str, patch: Patch) -> diffutils::UnifiedDiff {
    diffutils::UnifiedDiff {
        header: Some(header.to_string()),
        tail: Some(tail.to_string()),
        files: vec![diffutils::UnifiedDiffFile {
            from_file: from.map(str::to_string),
            to_file: Some(to.to_string()),
            patch,
            ..Default::default()
        }],
    }
}
