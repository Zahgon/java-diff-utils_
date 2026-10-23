//! Unified diff emission and parsing.

use crate::java::{self, spaces};
use crate::patch::{Chunk, Delta, DeltaType, Patch};
use crate::unifieddiff::{INVALID_NUMBER, ParseFailed};

const NULL_FILE: &str = "/dev/null";

/// Match a hunk header `@@\s+-a[,b]\s+\+c[,d]\s+@@.*$`, returning the digits of the two start lines (the
/// counts are matched but never read).
fn hunk_header(line: &str) -> Option<(&str, &str)> {
    let rest = spaces(line.strip_prefix("@@")?)?.strip_prefix('-')?;
    let (old_start, rest) = java::digits(rest)?;
    let rest = spaces(optional_count(rest)?)?.strip_prefix('+')?;
    let (new_start, rest) = java::digits(rest)?;
    let trailer = spaces(optional_count(rest)?)?.strip_prefix("@@")?;
    let end = trailer.find(java::is_line_terminator).unwrap_or(trailer.len());
    java::dollar_at(trailer, end).then_some((old_start, new_start))
}

fn optional_count(text: &str) -> Option<&str> {
    match text.strip_prefix(',') {
        Some(rest) => java::digits(rest).map(|(_, rest)| rest),
        None => Some(text),
    }
}

/// A start line of a hunk header as `Integer.parseInt` reads it, 0 counting as 1.
fn start_line(digits: &str, line: usize) -> Result<usize, ParseFailed> {
    let value = java::int_value(digits).ok_or_else(|| ParseFailed { reason: INVALID_NUMBER.to_string(), line })?;
    Ok(usize::try_from(value).unwrap_or(0).max(1))
}

/// Build the patch described by unified diff text: one change delta per hunk, with context
/// lines on both sides. Text the reference parser rejects gives an empty patch; see
/// [`parse_unified_diff_checked`].
pub fn parse_unified_diff(diff: &[String]) -> Patch {
    parse_unified_diff_checked(diff).unwrap_or_default()
}

/// [`parse_unified_diff`], or the failure the reference parser raises: `INVALID_NUMBER` for a hunk
/// start line too large for an `int`, with the 1-based index of that line in `diff`.
pub fn parse_unified_diff_checked(diff: &[String]) -> Result<Patch, ParseFailed> {
    let mut patch = Patch::new();
    let mut in_prelude = true;
    let mut raw: Vec<(char, String)> = Vec::new();
    let (mut old_ln, mut new_ln) = (0usize, 0usize);
    for (index, line) in diff.iter().enumerate() {
        if in_prelude {
            if line.starts_with("+++") {
                in_prelude = false;
            }
            continue;
        }
        if let Some((old_start, new_start)) = hunk_header(line) {
            flush(&mut raw, &mut patch, old_ln, new_ln);
            old_ln = start_line(old_start, index + 1)?;
            new_ln = start_line(new_start, index + 1)?;
        } else if let Some(tag) = line.chars().next() {
            if tag == ' ' || tag == '+' || tag == '-' {
                raw.push((tag, line[1..].to_string()));
            }
        } else {
            raw.push((' ', String::new()));
        }
    }
    flush(&mut raw, &mut patch, old_ln, new_ln);
    Ok(patch)
}

fn flush(raw: &mut Vec<(char, String)>, patch: &mut Patch, old_ln: usize, new_ln: usize) {
    if raw.is_empty() {
        return;
    }
    let mut old_lines = Vec::new();
    let mut new_lines = Vec::new();
    // The positions of the removed and added lines, counted from the hunk's start line as the reference
    // parser counts them (context lines advance both counts).
    let mut removed = Vec::new();
    let mut added = Vec::new();
    for (tag, text) in raw.drain(..) {
        if tag == ' ' || tag == '-' {
            old_lines.push(text.clone());
            if tag == '-' {
                removed.push((old_ln + old_lines.len()).wrapping_sub(1));
            }
        }
        if tag == ' ' || tag == '+' {
            new_lines.push(text);
            if tag == '+' {
                added.push((new_ln + new_lines.len()).wrapping_sub(1));
            }
        }
    }
    patch.add_delta(Delta::new(
        DeltaType::Change,
        Chunk::with_changes(old_ln.wrapping_sub(1), old_lines, removed),
        Chunk::with_changes(new_ln.wrapping_sub(1), new_lines, added),
    ));
}

/// Render `patch` (computed from `original`) as unified diff lines with `context` lines of
/// context; a missing name prints as `/dev/null`.
pub fn generate_unified_diff(
    original_name: Option<&str>,
    revised_name: Option<&str>,
    original: &[String],
    patch: &Patch,
    context: usize,
) -> Vec<String> {
    let deltas = patch.deltas();
    if deltas.is_empty() {
        return Vec::new();
    }
    let mut out = vec![
        format!("--- {}", original_name.unwrap_or(NULL_FILE)),
        format!("+++ {}", revised_name.unwrap_or(NULL_FILE)),
    ];
    let ctx = context as isize;
    let mut group: Vec<&Delta> = vec![&deltas[0]];
    for pair in deltas.windows(2) {
        let (current, next) = (&pair[0], &pair[1]);
        let reach = current.source.position as isize + current.source.size() as isize + ctx;
        if reach >= next.source.position as isize - ctx {
            group.push(next);
        } else {
            out.extend(hunk(original, &group, context, false));
            group = vec![next];
        }
    }
    let new_file = deltas.len() == 1 && original_name.is_none();
    out.extend(hunk(original, &group, context, new_file));
    out
}

fn hunk(original: &[String], deltas: &[&Delta], context: usize, new_file: bool) -> Vec<String> {
    let first = deltas[0];
    let ctx = context as isize;
    let orig_start = if new_file { 0 } else { (first.source.position as isize + 1 - ctx).max(1) };
    let rev_start = (first.target.position as isize + 1 - ctx).max(1);
    let mut body = Vec::new();
    let (mut orig_total, mut rev_total) = (0usize, 0usize);
    let lead = first.source.position.saturating_sub(context);
    for line in &original[lead..first.source.position] {
        body.push(format!(" {line}"));
        orig_total += 1;
        rev_total += 1;
    }
    let mut previous = first;
    for (index, delta) in deltas.iter().enumerate() {
        if index > 0 {
            let gap_start = previous.source.position + previous.source.size();
            for line in &original[gap_start..delta.source.position.max(gap_start)] {
                body.push(format!(" {line}"));
                orig_total += 1;
                rev_total += 1;
            }
        }
        body.extend(delta.source.lines.iter().map(|l| format!("-{l}")));
        body.extend(delta.target.lines.iter().map(|l| format!("+{l}")));
        orig_total += delta.source.size();
        rev_total += delta.target.size();
        previous = delta;
    }
    let tail_start = previous.source.position + previous.source.size();
    let tail_end = (tail_start + context).min(original.len()).max(tail_start);
    for line in &original[tail_start..tail_end] {
        body.push(format!(" {line}"));
        orig_total += 1;
        rev_total += 1;
    }
    let mut out = vec![format!("@@ -{orig_start},{orig_total} +{rev_start},{rev_total} @@")];
    out.extend(body);
    out
}

const NO_CHANGE_HEADER: &str = "@@ -0,0 +0,0 @@";

/// The original text with the unified diff of `revised` woven into it, as
/// `UnifiedDiffUtils.generateOriginalAndDiff` builds it: a missing name prints as `original` or
/// `revised`, the diff has no context, and unchanged lines carry a leading space.
pub fn generate_original_and_diff(
    original: &[String],
    revised: &[String],
    original_name: Option<&str>,
    revised_name: Option<&str>,
) -> Vec<String> {
    let original_name = original_name.unwrap_or("original");
    let revised_name = revised_name.unwrap_or("revised");
    let patch = crate::myers::diff(original, revised);
    let mut unified = generate_unified_diff(Some(original_name), Some(revised_name), original, &patch, 0);
    if unified.is_empty() {
        unified.push(format!("--- {original_name}"));
        unified.push(format!("+++ {revised_name}"));
        unified.push(NO_CHANGE_HEADER.to_string());
    } else if unified.len() >= 3 && !unified[2].contains("@@ -1,") {
        unified.insert(2, NO_CHANGE_HEADER.to_string());
    }
    let prefixed: Vec<String> = original.iter().map(|line| format!(" {line}")).collect();
    insert_original(&prefixed, &unified)
}

/// Split the diff into blocks at every hunk header except the no-change header and a header
/// starting at line 1, then weave the original lines between the blocks.
fn insert_original(original: &[String], unified: &[String]) -> Vec<String> {
    let mut blocks: Vec<Vec<String>> = Vec::new();
    let mut block: Vec<String> = Vec::new();
    for (i, line) in unified.iter().enumerate() {
        if line.starts_with("@@") && line != NO_CHANGE_HEADER && !line.contains("@@ -1,") {
            blocks.push(std::mem::take(&mut block));
            block.push(line.clone());
            continue;
        }
        if i == unified.len() - 1 {
            block.push(line.clone());
            blocks.push(std::mem::take(&mut block));
            break;
        }
        block.push(line.clone());
    }
    let mut result = Vec::new();
    for (i, block) in blocks.iter().enumerate() {
        let next = blocks.get(i + 1);
        let symbol = if i == 0 { block.get(2) } else { block.first() };
        let next_symbol = next.and_then(|b| b.first());
        result.extend(block.iter().cloned());
        let (org_row, org_del) = symbol.map_or((0, 0), |s| row_map(s));
        if let Some(next_symbol) = next_symbol {
            let (next_rev_row, _) = row_map(next_symbol);
            let start = if org_row != 0 { org_row + org_del - 1 } else { 0 };
            let end = next_rev_row - 2;
            result.extend(original_slice(original, start, end));
        }
        let start = org_row + org_del - 1;
        let start = if start == -1 { 0 } else { start };
        let len = original.len() as i64;
        let first_line_hunk = symbol.is_some_and(|s| s.contains("@@ -1,"));
        if (first_line_hunk && next_symbol.is_none() && org_del != len)
            || (next_symbol.is_none() && org_row + org_del - 1 < len)
        {
            result.extend(original_slice(original, start, len - 1));
        }
    }
    result
}

/// The `-row,count` pair of a hunk header; the reference reads the revised row from the same pair.
fn row_map(header: &str) -> (i64, i64) {
    if !header.starts_with("@@") {
        return (0, 0);
    }
    let original = java::split(header, " ").get(1).copied().unwrap_or("");
    let parts = java::split(original, ",");
    let row = parts.first().and_then(|p| p.get(1..)).and_then(|p| p.parse().ok()).unwrap_or(0);
    let count = parts.get(1).and_then(|p| p.parse().ok()).unwrap_or(0);
    (row, count)
}

fn original_slice(original: &[String], start: i64, end: i64) -> Vec<String> {
    let len = original.len() as i64;
    if len >= 1 && start <= end && end < len && start >= 0 {
        original[start as usize..=end as usize].to_vec()
    } else {
        Vec::new()
    }
}
