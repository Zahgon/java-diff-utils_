//! Multi-file (git style) unified diffs: the rule-driven reader and the writer.

use crate::java::{self, LineReader, digits, dollar_at, int_value, one_space, rest_of_line, spaces};
use crate::patch::{Chunk, Delta, DeltaType, Patch, PatchFailed};

/// A parsed multi-file diff.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UnifiedDiff {
    pub header: Option<String>,
    pub tail: Option<String>,
    pub files: Vec<UnifiedDiffFile>,
}

impl UnifiedDiff {
    /// The first file whose `from_file` `find_file` accepts, with its patch applied strictly to a copy of
    /// `original_lines`; a copy of `original_lines` when no file is accepted.
    pub fn apply_patch_to(&self, find_file: impl Fn(Option<&str>) -> bool, original_lines: &[String]) -> Result<Vec<String>, PatchFailed> {
        let mut result = original_lines.to_vec();
        if let Some(file) = self.files.iter().find(|file| find_file(file.from_file.as_deref())) {
            file.patch.apply_to(&mut result)?;
        }
        Ok(result)
    }
}

/// One file section of a multi-file diff.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UnifiedDiffFile {
    pub diff_command: Option<String>,
    pub from_file: Option<String>,
    pub to_file: Option<String>,
    pub from_timestamp: Option<String>,
    pub to_timestamp: Option<String>,
    pub index: Option<String>,
    pub rename_from: Option<String>,
    pub rename_to: Option<String>,
    pub copy_from: Option<String>,
    pub copy_to: Option<String>,
    pub new_file_mode: Option<String>,
    pub deleted_file_mode: Option<String>,
    pub old_mode: Option<String>,
    pub new_mode: Option<String>,
    pub binary_added: Option<String>,
    pub binary_deleted: Option<String>,
    pub binary_edited: Option<String>,
    pub similarity_index: Option<i32>,
    pub no_newline_at_end: bool,
    pub patch: Patch,
}

/// Why a multi-file diff could not be read, and the 1-based number of the last input line read before the
/// failure (0 when no line had been read).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseFailed {
    pub reason: String,
    pub line: usize,
}

impl ParseFailed {
    /// A failure whose line number `read_unified_diff` fills in.
    fn new(reason: &str) -> ParseFailed {
        ParseFailed { reason: reason.to_string(), line: 0 }
    }
}

impl std::fmt::Display for ParseFailed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.reason)
    }
}

impl std::error::Error for ParseFailed {}

const NO_FILE_START: &str = "expected file start line not found";
const NO_DATA_LINE: &str = "expected data line not found";
pub(crate) const INVALID_NUMBER: &str = "INVALID_NUMBER";
const INVALID_RANGE: &str = "INVALID_RANGE";
const NO_FILE: &str = "NO_FILE";
const NO_NEWLINE: &str = "\\ No newline at end of file";

/// The file-start rules, in the order the reader tries them.
#[derive(Clone, Copy)]
enum Rule {
    DiffCommand,
    SimilarityIndex,
    Index,
    FromFile,
    ToFile,
    RenameFrom,
    RenameTo,
    CopyFrom,
    CopyTo,
    NewFileMode,
    DeletedFileMode,
    OldMode,
    NewMode,
    BinaryAdded,
    BinaryDeleted,
    BinaryEdited,
}

const FILE_RULES: [Rule; 16] = [
    Rule::DiffCommand,
    Rule::SimilarityIndex,
    Rule::Index,
    Rule::FromFile,
    Rule::ToFile,
    Rule::RenameFrom,
    Rule::RenameTo,
    Rule::CopyFrom,
    Rule::CopyTo,
    Rule::NewFileMode,
    Rule::DeletedFileMode,
    Rule::OldMode,
    Rule::NewMode,
    Rule::BinaryAdded,
    Rule::BinaryDeleted,
    Rule::BinaryEdited,
];

/// What a matching file-start rule captured.
enum Capture<'a> {
    Line,
    Text(&'a str),
}

/// Words separated by single `\s` characters, as in `^rename\sfrom\s`.
fn words<'a>(line: &'a str, words: &[&str]) -> Option<&'a str> {
    let mut rest = line;
    for word in words {
        rest = one_space(rest.strip_prefix(word)?)?;
    }
    Some(rest)
}

/// The longest prefix of `text` without a line terminator, at least one character long, that
/// `tail` accepts right after it; the greedy `(.+)` of a backtracking matcher.
fn greedy(text: &str, tail: impl Fn(&str) -> bool) -> Option<&str> {
    let limit = text.find(java::is_line_terminator).unwrap_or(text.len());
    let mut ends: Vec<usize> = text[..limit].char_indices().map(|(i, _)| i).skip(1).collect();
    ends.push(limit);
    ends.into_iter().rev().find(|&end| tail(&text[end..])).map(|end| &text[..end])
}

fn literal_after_space<'a>(text: &'a str, literal: &str) -> Option<&'a str> {
    one_space(text)?.strip_prefix(literal)
}

fn mode(line: &str, prefix: &[&str]) -> Option<String> {
    words(line, prefix).and_then(digits).map(|(d, _)| d.to_string())
}

impl Rule {
    fn matches(self, line: &str) -> Option<Capture<'_>> {
        match self {
            Rule::DiffCommand => line.strip_prefix("diff").and_then(one_space).map(|_| Capture::Line),
            Rule::SimilarityIndex => {
                let (number, rest) = digits(line.strip_prefix("similarity index ")?)?;
                let after = rest.strip_prefix('%')?;
                dollar_at(after, 0).then_some(Capture::Text(number))
            }
            Rule::Index => {
                let rest = words(line, &["index"])?;
                let alnum = |t: &str| t.find(|c: char| !c.is_ascii_alphanumeric()).unwrap_or(t.len());
                let first = alnum(rest);
                let rest = rest[first..].strip_prefix("..").filter(|_| first > 0)?;
                let second = alnum(rest);
                if second == 0 {
                    return None;
                }
                let rest = &rest[second..];
                let mode_ok = || one_space(rest).and_then(digits).is_some_and(|(_, after)| dollar_at(after, 0));
                (dollar_at(rest, 0) || mode_ok()).then_some(Capture::Line)
            }
            Rule::FromFile => line.strip_prefix("---").and_then(one_space).map(|_| Capture::Line),
            Rule::ToFile => line.strip_prefix("+++").and_then(one_space).map(|_| Capture::Line),
            Rule::RenameFrom => words(line, &["rename", "from"]).and_then(rest_of_line).map(Capture::Text),
            Rule::RenameTo => words(line, &["rename", "to"]).and_then(rest_of_line).map(Capture::Text),
            Rule::CopyFrom => words(line, &["copy", "from"]).and_then(rest_of_line).map(Capture::Text),
            Rule::CopyTo => words(line, &["copy", "to"]).and_then(rest_of_line).map(Capture::Text),
            Rule::NewFileMode | Rule::DeletedFileMode | Rule::OldMode | Rule::NewMode => None,
            Rule::BinaryAdded => {
                let rest = words(line, &["Binary", "files", "/dev/null", "and"])?.strip_prefix("b/")?;
                greedy(rest, |t| literal_after_space(t, "differ").is_some()).map(Capture::Text)
            }
            Rule::BinaryDeleted => {
                let rest = words(line, &["Binary", "files"])?.strip_prefix("a/")?;
                let tail = |t: &str| {
                    one_space(t)
                        .and_then(|t| words(t, &["and", "/dev/null"]))
                        .and_then(|t| t.strip_prefix("differ"))
                        .is_some()
                };
                greedy(rest, tail).map(Capture::Text)
            }
            Rule::BinaryEdited => {
                let rest = words(line, &["Binary", "files"])?.strip_prefix("a/")?;
                let tail = |t: &str| {
                    one_space(t)
                        .and_then(|t| words(t, &["and"]))
                        .and_then(|t| t.strip_prefix("b/"))
                        .and_then(|t| greedy(t, |u| literal_after_space(u, "differ").is_some()))
                        .is_some()
                };
                greedy(rest, tail).map(Capture::Text)
            }
        }
    }

    fn mode_value(self, line: &str) -> Option<String> {
        match self {
            Rule::NewFileMode => mode(line, &["new", "file", "mode"]),
            Rule::DeletedFileMode => mode(line, &["deleted", "file", "mode"]),
            Rule::OldMode => mode(line, &["old", "mode"]),
            Rule::NewMode => mode(line, &["new", "mode"]),
            _ => None,
        }
    }

    fn valid(self, line: &str) -> bool {
        self.matches(line).is_some() || self.mode_value(line).is_some()
    }
}

/// The four numbers of a hunk header, as text; `None` for an absent count.
struct HunkHeader<'a> {
    old_start: &'a str,
    old_size: Option<&'a str>,
    new_start: &'a str,
    new_size: Option<&'a str>,
}

fn range(text: &str) -> Option<(&str, Option<&str>, &str)> {
    let (start, rest) = digits(text)?;
    match rest.strip_prefix(',').and_then(digits) {
        Some((size, rest)) => Some((start, Some(size), rest)),
        None => Some((start, None, rest)),
    }
}

fn hunk_header(line: &str) -> Option<HunkHeader<'_>> {
    let rest = spaces(line.strip_prefix("@@")?)?.strip_prefix('-')?;
    let (old_start, old_size, rest) = range(rest)?;
    let rest = spaces(rest)?.strip_prefix('+')?;
    let (new_start, new_size, rest) = range(rest)?;
    spaces(rest)?.strip_prefix("@@")?;
    Some(HunkHeader { old_start, old_size, new_start, new_size })
}

fn number(text: Option<&str>) -> Result<i64, ParseFailed> {
    match text {
        None => Ok(1),
        Some(digits) => int_value(digits).map(i64::from).ok_or_else(|| ParseFailed::new(INVALID_NUMBER)),
    }
}

/// The first `yyyy-mm-dd[T ]hh:mm:ss.fff[ +zone]` in `line`, as a byte range.
fn find_timestamp(line: &str) -> Option<(usize, usize)> {
    let bytes = line.as_bytes();
    let digit = |i: usize| bytes.get(i).is_some_and(u8::is_ascii_digit);
    let is = |i: usize, b: u8| bytes.get(i) == Some(&b);
    for start in 0..bytes.len() {
        let shape = [0, 1, 2, 3, 5, 6, 8, 9, 11, 12, 14, 15, 17, 18].iter().all(|&o| digit(start + o))
            && is(start + 4, b'-')
            && is(start + 7, b'-')
            && (is(start + 10, b'T') || is(start + 10, b' '))
            && is(start + 13, b':')
            && is(start + 16, b':')
            && is(start + 19, b'.');
        if !shape {
            continue;
        }
        let mut end = start + 20;
        while digit(end) {
            end += 1;
        }
        if end - (start + 20) < 3 {
            continue;
        }
        if is(end, b' ') && (is(end + 1, b'+') || is(end + 1, b'-')) && digit(end + 2) {
            end += 2;
            while digit(end) {
                end += 1;
            }
        }
        return Some((start, end));
    }
    None
}

fn extract_file_name(line: &str) -> Result<String, ParseFailed> {
    let line = match find_timestamp(line) {
        Some((start, _)) => &line[..start],
        None => line,
    };
    let first = *java::split(line, "\t").first().ok_or_else(|| ParseFailed::new(INVALID_RANGE))?;
    let name = first.get(4..).filter(|_| java::utf16_len(first) >= 4).ok_or_else(|| ParseFailed::new(INVALID_RANGE))?;
    let name = ["a/", "b/", "old/", "new/"].iter().find_map(|p| name.strip_prefix(p)).unwrap_or(name);
    Ok(java::trim(name).to_string())
}

fn extract_timestamp(line: &str) -> Option<String> {
    find_timestamp(line).map(|(start, end)| line[start..end].to_string())
}

struct Reader<'a> {
    input: LineReader<'a>,
    data: UnifiedDiff,
    file: Option<usize>,
    original: Vec<String>,
    revised: Vec<String>,
    deletes: usize,
    inserts: usize,
    /// Context and deleted lines, and context and added lines, of the hunk so far.
    del_line_idx: i64,
    add_line_idx: i64,
    /// Positions of the deleted and of the added lines of the hunk so far.
    removed: Vec<usize>,
    added: Vec<usize>,
    old_ln: i64,
    old_size: i64,
    new_ln: i64,
    new_size: i64,
}

impl<'a> Reader<'a> {
    fn file(&mut self) -> Result<&mut UnifiedDiffFile, ParseFailed> {
        let index = self.file.ok_or_else(|| ParseFailed::new(NO_FILE))?;
        Ok(&mut self.data.files[index])
    }

    fn header_line(line: &str) -> bool {
        FILE_RULES.iter().any(|rule| rule.valid(line)) || hunk_header(line).is_some()
    }

    fn start_file(&mut self) -> Result<(), ParseFailed> {
        if !self.original.is_empty() || !self.revised.is_empty() {
            return Err(ParseFailed::new("ILLEGAL_STATE"));
        }
        self.data.files.push(UnifiedDiffFile::default());
        self.file = Some(self.data.files.len() - 1);
        Ok(())
    }

    /// Apply the first file-start rule that matches; `false` when none does.
    fn file_line(&mut self, line: &str) -> Result<bool, ParseFailed> {
        for rule in FILE_RULES {
            if let Some(value) = rule.mode_value(line) {
                let file = self.file()?;
                match rule {
                    Rule::NewFileMode => file.new_file_mode = Some(value),
                    Rule::DeletedFileMode => file.deleted_file_mode = Some(value),
                    Rule::OldMode => file.old_mode = Some(value),
                    _ => file.new_mode = Some(value),
                }
                return Ok(true);
            }
            let Some(capture) = rule.matches(line) else { continue };
            let text = match capture {
                Capture::Text(text) => Some(text.to_string()),
                Capture::Line => None,
            };
            match rule {
                Rule::DiffCommand => {
                    let parts = java::split(line, " ");
                    if parts.len() < 4 {
                        return Err(ParseFailed::new(INVALID_RANGE));
                    }
                    let from = parts[2].strip_prefix("a/").unwrap_or(parts[2]).to_string();
                    let to = parts[3].strip_prefix("b/").unwrap_or(parts[3]).to_string();
                    let file = self.file()?;
                    file.from_file = Some(from);
                    file.to_file = Some(to);
                    file.diff_command = Some(line.to_string());
                }
                Rule::SimilarityIndex => {
                    let value = text.as_deref().and_then(int_value).ok_or_else(|| ParseFailed::new(INVALID_NUMBER))?;
                    self.file()?.similarity_index = Some(value);
                }
                Rule::Index => self.file()?.index = Some(line[6..].to_string()),
                Rule::FromFile => {
                    let (name, stamp) = (extract_file_name(line)?, extract_timestamp(line));
                    let file = self.file()?;
                    file.from_file = Some(name);
                    file.from_timestamp = stamp;
                }
                Rule::ToFile => {
                    let (name, stamp) = (extract_file_name(line)?, extract_timestamp(line));
                    let file = self.file()?;
                    file.to_file = Some(name);
                    file.to_timestamp = stamp;
                }
                Rule::RenameFrom => self.file()?.rename_from = text,
                Rule::RenameTo => self.file()?.rename_to = text,
                Rule::CopyFrom => self.file()?.copy_from = text,
                Rule::CopyTo => self.file()?.copy_to = text,
                Rule::BinaryAdded => self.file()?.binary_added = text,
                Rule::BinaryDeleted => self.file()?.binary_deleted = text,
                Rule::BinaryEdited => self.file()?.binary_edited = text,
                Rule::NewFileMode | Rule::DeletedFileMode | Rule::OldMode | Rule::NewMode => {}
            }
            return Ok(true);
        }
        Ok(false)
    }

    fn start_hunk(&mut self, line: &str) -> Result<(), ParseFailed> {
        let Some(header) = hunk_header(line) else { return Ok(()) };
        self.old_ln = number(Some(header.old_start))?;
        self.old_size = number(header.old_size)?;
        self.new_ln = number(Some(header.new_start))?;
        self.new_size = number(header.new_size)?;
        if self.old_ln == 0 {
            self.old_ln = 1;
        }
        if self.new_ln == 0 {
            self.new_ln = 1;
        }
        Ok(())
    }

    /// Record one hunk line; `false` when it is neither context, addition nor deletion.
    fn data_line(&mut self, line: &str) -> bool {
        let Some(first) = line.chars().next() else { return false };
        let text = line[first.len_utf8()..].to_string();
        if java::is_space(first) {
            self.original.push(text.clone());
            self.revised.push(text);
            self.del_line_idx += 1;
            self.add_line_idx += 1;
        } else if first == '+' {
            self.revised.push(text);
            self.inserts += 1;
            self.add_line_idx += 1;
            self.added.push((self.new_ln - 1 + self.add_line_idx) as usize);
        } else if first == '-' {
            self.original.push(text);
            self.deletes += 1;
            self.del_line_idx += 1;
            self.removed.push((self.old_ln - 1 + self.del_line_idx) as usize);
        } else {
            return false;
        }
        true
    }

    fn hunk_complete(&self) -> bool {
        let (orig, rev) = (self.original.len() as i64, self.revised.len() as i64);
        (orig == self.old_size && rev == self.new_size)
            || (self.old_size == 0 && self.new_size == 0 && orig == self.old_ln && rev == self.new_ln)
    }

    fn finish_hunk(&mut self) -> Result<(), ParseFailed> {
        if self.original.is_empty() && self.revised.is_empty() {
            return Ok(());
        }
        let has_context = self.original.len() != self.deletes || self.revised.len() != self.inserts;
        let (has_deletes, has_inserts) = (self.deletes > 0, self.inserts > 0);
        let kind = if has_context || (has_deletes && has_inserts) {
            DeltaType::Change
        } else if has_deletes {
            DeltaType::Delete
        } else if has_inserts {
            DeltaType::Insert
        } else {
            DeltaType::Equal
        };
        let delta = Delta::new(
            kind,
            Chunk::with_changes((self.old_ln - 1) as usize, std::mem::take(&mut self.original), std::mem::take(&mut self.removed)),
            Chunk::with_changes((self.new_ln - 1) as usize, std::mem::take(&mut self.revised), std::mem::take(&mut self.added)),
        );
        self.file()?.patch.add_delta(delta);
        self.old_ln = 0;
        self.new_ln = 0;
        self.deletes = 0;
        self.inserts = 0;
        self.del_line_idx = 0;
        self.add_line_idx = 0;
        Ok(())
    }

    fn no_newline(&mut self, line: Option<&'a str>) -> Result<Option<&'a str>, ParseFailed> {
        if line == Some(NO_NEWLINE) {
            self.file()?.no_newline_at_end = true;
            return Ok(self.input.read_line());
        }
        Ok(line)
    }

    fn parse(&mut self) -> Result<UnifiedDiff, ParseFailed> {
        let mut line = self.input.read_line();
        while line.is_some() {
            let mut header = String::new();
            while let Some(text) = line {
                if Self::header_line(text) {
                    break;
                }
                header.push_str(text);
                header.push('\n');
                line = self.input.read_line();
            }
            if !header.is_empty() {
                self.data.header = Some(header);
            }
            if line.is_some_and(|text| hunk_header(text).is_none()) {
                self.start_file()?;
                while let Some(text) = line.filter(|text| hunk_header(text).is_none()) {
                    if !self.file_line(text)? {
                        return Err(ParseFailed::new(NO_FILE_START));
                    }
                    line = self.input.read_line();
                }
            }
            if let Some(text) = line {
                self.start_hunk(text)?;
                loop {
                    line = self.input.read_line();
                    if line.is_none() {
                        break;
                    }
                    line = self.no_newline(line)?;
                    if !line.is_some_and(|text| self.data_line(text)) {
                        return Err(ParseFailed::new(NO_DATA_LINE));
                    }
                    if self.hunk_complete() {
                        self.finish_hunk()?;
                        break;
                    }
                }
                line = self.input.read_line();
                line = self.no_newline(line)?;
            }
            match line {
                None => break,
                Some(text) if text.starts_with("--") && !text.starts_with("---") => break,
                _ => {}
            }
        }
        if self.input.ready() {
            let mut tail = String::new();
            while self.input.ready() {
                if !tail.is_empty() {
                    tail.push('\n');
                }
                tail.push_str(self.input.read_line().unwrap_or_default());
            }
            self.data.tail = Some(tail);
        }
        Ok(std::mem::take(&mut self.data))
    }
}

/// Read a multi-file unified diff (git style) from its text.
pub fn read_unified_diff(text: &str) -> Result<UnifiedDiff, ParseFailed> {
    let mut reader = Reader {
        input: LineReader::new(text),
        data: UnifiedDiff::default(),
        file: None,
        original: Vec::new(),
        revised: Vec::new(),
        deletes: 0,
        inserts: 0,
        del_line_idx: 0,
        add_line_idx: 0,
        removed: Vec::new(),
        added: Vec::new(),
        old_ln: 0,
        old_size: 0,
        new_ln: 0,
        new_size: 0,
    };
    reader.parse().map_err(|failure| ParseFailed { line: reader.input.lines_read(), ..failure })
}

fn delta_text(out: &mut Vec<String>, delta: &Delta) {
    out.extend(delta.source.lines.iter().map(|l| format!("-{l}")));
    out.extend(delta.target.lines.iter().map(|l| format!("+{l}")));
}

fn context(out: &mut Vec<String>, lines: &[String], from: usize, to: usize) -> usize {
    let to = to.min(lines.len());
    let from = from.min(to);
    out.extend(lines[from..to].iter().map(|l| format!(" {l}")));
    to - from
}

fn write_hunk(out: &mut Vec<String>, lines: &[String], deltas: &[&Delta], context_size: usize, new_file: bool) {
    let first = deltas[0];
    let start = |position: usize| (position as i64 + 1 - context_size as i64).max(1);
    let orig_start = if new_file { 0 } else { start(first.source.position) };
    let rev_start = start(first.target.position);
    let mut body = Vec::new();
    let mut shared = context(&mut body, lines, first.source.position.saturating_sub(context_size), first.source.position);
    let (mut orig_total, mut rev_total) = (0, 0);
    delta_text(&mut body, first);
    orig_total += first.source.size();
    rev_total += first.target.size();
    let mut current = first;
    for next in &deltas[1..] {
        shared += context(&mut body, lines, current.source.position + current.source.size(), next.source.position);
        delta_text(&mut body, next);
        orig_total += next.source.size();
        rev_total += next.target.size();
        current = next;
    }
    let after = current.source.position + current.source.size();
    shared += context(&mut body, lines, after, after + context_size);
    out.push(format!("@@ -{},{} +{},{} @@", orig_start, orig_total + shared, rev_start, rev_total + shared));
    out.extend(body);
}

/// Render a multi-file diff, one element per line handed to the writer (the header is one element
/// as stored); `original_lines` gives the original text of a file by its `from_file` name.
pub fn write_unified_diff(
    diff: &UnifiedDiff,
    original_lines: &dyn Fn(Option<&str>) -> Vec<String>,
    context_size: usize,
) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(header) = &diff.header {
        out.push(header.clone());
    }
    for file in &diff.files {
        let deltas = file.patch.deltas();
        if deltas.is_empty() {
            continue;
        }
        out.extend(file.diff_command.clone());
        if let Some(index) = &file.index {
            out.push(format!("index {index}"));
        }
        out.push(format!("--- {}", file.from_file.as_deref().unwrap_or("/dev/null")));
        if let Some(to) = &file.to_file {
            out.push(format!("+++ {to}"));
        }
        let lines = original_lines(file.from_file.as_deref());
        let mut group: Vec<&Delta> = vec![&deltas[0]];
        for pair in deltas.windows(2) {
            let (current, next) = (&pair[0], &pair[1]);
            let reach = (current.source.position + current.source.size() + context_size) as i64;
            if reach >= next.source.position as i64 - context_size as i64 {
                group.push(next);
            } else {
                write_hunk(&mut out, &lines, &group, context_size, false);
                group = vec![next];
            }
        }
        let new_file = deltas.len() == 1 && file.from_file.is_none();
        write_hunk(&mut out, &lines, &group, context_size, new_file);
    }
    if let Some(tail) = &diff.tail {
        out.push("--".to_string());
        out.push(tail.clone());
    }
    out
}
