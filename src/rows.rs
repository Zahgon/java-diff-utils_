//! Side-by-side diff rows with optional inline markup.

use crate::java;
use crate::myers::diff_by;
use crate::patch::{Chunk, Delta, DeltaType, Patch, PatchFailed};

/// The kind of a diff row, also handed to the tag generators.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DiffTag {
    Insert,
    Delete,
    Change,
    Equal,
}

/// One row of a side-by-side view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffRow {
    pub tag: DiffTag,
    pub old_line: String,
    pub new_line: String,
}

impl std::fmt::Display for DiffTag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            DiffTag::Insert => "INSERT",
            DiffTag::Delete => "DELETE",
            DiffTag::Change => "CHANGE",
            DiffTag::Equal => "EQUAL",
        })
    }
}

/// `[TAG,old line,new line]`, as `DiffRow.toString` writes a row.
impl std::fmt::Display for DiffRow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{},{},{}]", self.tag, self.old_line, self.new_line)
    }
}

type TagGenerator = Box<dyn Fn(DiffTag, bool) -> String>;
type TextProcessor = Box<dyn Fn(&str) -> String>;
type Splitter = Box<dyn Fn(&str) -> Vec<String>>;

/// How changed lines are cut into inline tokens.
enum TokenSplit {
    /// `SPLITTER_BY_CHARACTER`: one token per UTF-16 code unit, as `String.toCharArray` cuts a string.
    Character,
    /// `SPLITTER_BY_WORD`.
    Word,
    Custom(Splitter),
}

/// One inline token as UTF-16 code units: a token of the character splitter can be half of a surrogate pair,
/// which a Rust string cannot hold.
type Token = Vec<u16>;

fn units(text: &str) -> Token {
    java::utf16_units(text)
}

fn text_of(token: &[u16]) -> String {
    java::from_utf16(token)
}
type Equalizer = Box<dyn Fn(&str, &str) -> bool>;
type Merger = Box<dyn Fn(&InlineDeltaMergeInfo) -> Vec<Delta>>;

/// What an inline delta merger gets: the inline deltas of one changed block and the token lists
/// they were computed from, as `InlineDeltaMergeInfo` holds them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InlineDeltaMergeInfo {
    pub deltas: Vec<Delta>,
    pub orig_list: Vec<String>,
    pub rev_list: Vec<String>,
}

/// `DeltaMergeUtils.mergeInlineDeltas`: walk the deltas in order and fold each one into the
/// previous kept delta when `replace_equality` accepts the original tokens between them.
pub fn merge_inline_deltas(info: &InlineDeltaMergeInfo, replace_equality: impl Fn(&[String]) -> bool) -> Vec<Delta> {
    if info.deltas.len() < 2 {
        return info.deltas.clone();
    }
    let mut merged: Vec<Delta> = vec![info.deltas[0].clone()];
    for current in &info.deltas[1..] {
        let Some(previous) = merged.last() else { break };
        let gap_start = previous.source.position + previous.source.size();
        let gap = info.orig_list.get(gap_start..current.source.position).unwrap_or(&[]);
        if replace_equality(gap) {
            let mut source = previous.source.lines.clone();
            source.extend_from_slice(gap);
            source.extend(current.source.lines.iter().cloned());
            let mut target = previous.target.lines.clone();
            target.extend_from_slice(gap);
            target.extend(current.target.lines.iter().cloned());
            let replacement = Delta::new(
                DeltaType::Change,
                Chunk::new(previous.source.position, source),
                Chunk::new(previous.target.position, target),
            );
            merged.pop();
            merged.push(replacement);
        } else {
            merged.push(current.clone());
        }
    }
    merged
}

fn whitespace_merger(info: &InlineDeltaMergeInfo) -> Vec<Delta> {
    merge_inline_deltas(info, |gap| gap.iter().all(|token| java::replace_space_runs(token, "").is_empty()))
}

fn default_merger(info: &InlineDeltaMergeInfo) -> Vec<Delta> {
    info.deltas.clone()
}

/// Builds a [`DiffRowGenerator`]; every option starts at the reference library's default.
pub struct DiffRowGeneratorBuilder {
    show_inline_diffs: bool,
    ignore_white_spaces: bool,
    report_lines_unchanged: bool,
    column_width: usize,
    merge_original_revised: bool,
    decompress_deltas: bool,
    splitter: TokenSplit,
    equalizer: Option<Equalizer>,
    replace_original_linefeed_in_changes_with_spaces: bool,
    merger: Merger,
    old_tag: TagGenerator,
    new_tag: TagGenerator,
    line_normalizer: TextProcessor,
    process_diffs: Option<TextProcessor>,
    process_equalities: Option<TextProcessor>,
}

/// A diff row with the index of the original and of the revised line it stands for.
type OriginRow = (DiffRow, Option<usize>, Option<usize>);

/// Turns two line lists into diff rows.
pub struct DiffRowGenerator {
    options: DiffRowGeneratorBuilder,
}

fn span(class: &'static str) -> TagGenerator {
    Box::new(move |_, open| if open { format!("<span class=\"{class}\">") } else { "</span>".to_string() })
}

/// HTML-escape `<` and `>` and expand tabs to four spaces.
fn normalize_for_html(line: &str) -> String {
    line.replace('<', "&lt;").replace('>', "&gt;").replace('\t', "    ")
}

impl Default for DiffRowGeneratorBuilder {
    fn default() -> Self {
        DiffRowGeneratorBuilder {
            show_inline_diffs: false,
            ignore_white_spaces: false,
            report_lines_unchanged: false,
            column_width: 0,
            merge_original_revised: false,
            decompress_deltas: true,
            splitter: TokenSplit::Character,
            equalizer: None,
            replace_original_linefeed_in_changes_with_spaces: false,
            merger: Box::new(default_merger),
            old_tag: span("editOldInline"),
            new_tag: span("editNewInline"),
            line_normalizer: Box::new(normalize_for_html),
            process_diffs: None,
            process_equalities: None,
        }
    }
}

impl DiffRowGeneratorBuilder {
    pub fn show_inline_diffs(mut self, value: bool) -> Self {
        self.show_inline_diffs = value;
        self
    }

    pub fn ignore_white_spaces(mut self, value: bool) -> Self {
        self.ignore_white_spaces = value;
        self
    }

    pub fn report_lines_unchanged(mut self, value: bool) -> Self {
        self.report_lines_unchanged = value;
        self
    }

    /// Wrap width in UTF-16 code units; a negative width leaves the setting unchanged.
    pub fn column_width(mut self, width: i32) -> Self {
        if let Ok(width) = usize::try_from(width) {
            self.column_width = width;
        }
        self
    }

    pub fn merge_original_revised(mut self, value: bool) -> Self {
        self.merge_original_revised = value;
        self
    }

    pub fn decompress_deltas(mut self, value: bool) -> Self {
        self.decompress_deltas = value;
        self
    }

    /// Split changed lines into words (or back into characters); replaces any splitter set before.
    pub fn inline_diff_by_word(mut self, value: bool) -> Self {
        self.splitter = if value { TokenSplit::Word } else { TokenSplit::Character };
        self
    }

    /// Split changed lines with `splitter`; replaces any splitter set before.
    pub fn inline_diff_by_splitter(mut self, splitter: impl Fn(&str) -> Vec<String> + 'static) -> Self {
        self.splitter = TokenSplit::Custom(Box::new(splitter));
        self
    }

    /// Compare lines and inline tokens with `equalizer` (original first), instead of exact or
    /// whitespace-insensitive comparison, whenever it was called.
    pub fn equalizer(mut self, equalizer: impl Fn(&str, &str) -> bool + 'static) -> Self {
        self.equalizer = Some(Box::new(equalizer));
        self
    }

    /// Post-process the inline deltas of every changed block; replaces any merger set before.
    pub fn inline_delta_merger(mut self, merger: impl Fn(&InlineDeltaMergeInfo) -> Vec<Delta> + 'static) -> Self {
        self.merger = Box::new(merger);
        self
    }

    pub fn replace_original_linefeed_in_changes_with_spaces(mut self, value: bool) -> Self {
        self.replace_original_linefeed_in_changes_with_spaces = value;
        self
    }

    /// Use the whitespace-equalities merger (or the default one); replaces any merger set before.
    pub fn merge_whitespace_equalities(mut self, value: bool) -> Self {
        self.merger = if value { Box::new(whitespace_merger) } else { Box::new(default_merger) };
        self
    }

    pub fn old_tag(mut self, tag: impl Fn(DiffTag, bool) -> String + 'static) -> Self {
        self.old_tag = Box::new(tag);
        self
    }

    pub fn new_tag(mut self, tag: impl Fn(DiffTag, bool) -> String + 'static) -> Self {
        self.new_tag = Box::new(tag);
        self
    }

    pub fn line_normalizer(mut self, normalizer: impl Fn(&str) -> String + 'static) -> Self {
        self.line_normalizer = Box::new(normalizer);
        self
    }

    pub fn process_diffs(mut self, processor: impl Fn(&str) -> String + 'static) -> Self {
        self.process_diffs = Some(Box::new(processor));
        self
    }

    pub fn process_equalities(mut self, processor: impl Fn(&str) -> String + 'static) -> Self {
        self.process_equalities = Some(Box::new(processor));
        self
    }

    pub fn build(self) -> DiffRowGenerator {
        DiffRowGenerator { options: self }
    }
}

/// Trim, then collapse every run of whitespace to one space.
fn adjust_whitespace(line: &str) -> String {
    java::replace_space_runs(java::trim(line), " ")
}

fn is_word_delimiter(c: char) -> bool {
    matches!(c, ',' | '.' | '[' | ']' | '(' | ')' | '{' | '}' | '/' | '\\' | '*' | '+' | '-' | '#' | '<' | '>' | ';' | ':' | '&' | '\'')
}

/// Split into words, keeping every run of whitespace and every run of delimiters as its own token.
fn split_by_word(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut pending = 0;
    let mut chars = text.char_indices().peekable();
    while let Some((start, c)) = chars.next() {
        let class: fn(char) -> bool = if java::is_space(c) {
            java::is_space
        } else if is_word_delimiter(c) {
            is_word_delimiter
        } else {
            continue;
        };
        let mut end = start + c.len_utf8();
        while let Some(&(at, next)) = chars.peek() {
            if !class(next) {
                break;
            }
            end = at + next.len_utf8();
            chars.next();
        }
        if pending < start {
            tokens.push(text[pending..start].to_string());
        }
        tokens.push(text[start..end].to_string());
        pending = end;
    }
    if pending < text.len() {
        tokens.push(text[pending..].to_string());
    }
    tokens
}


/// Split a change of unequal sides into a change of the common length and an insert or delete.
fn decompress(delta: &Delta) -> Vec<Delta> {
    if delta.kind != DeltaType::Change || delta.source.size() == delta.target.size() {
        return vec![delta.clone()];
    }
    let common = delta.source.size().min(delta.target.size());
    let (orig, rev) = (&delta.source, &delta.target);
    let head = Delta::new(
        DeltaType::Change,
        Chunk::new(orig.position, orig.lines[..common].to_vec()),
        Chunk::new(rev.position, rev.lines[..common].to_vec()),
    );
    let tail = if orig.size() < rev.size() {
        Delta::new(
            DeltaType::Insert,
            Chunk::new(orig.position + common, Vec::new()),
            Chunk::new(rev.position + common, rev.lines[common..].to_vec()),
        )
    } else {
        Delta::new(
            DeltaType::Delete,
            Chunk::new(orig.position + common, orig.lines[common..].to_vec()),
            Chunk::new(rev.position + common, Vec::new()),
        )
    };
    vec![head, tail]
}

/// Surround the tokens `start..end` with opening and closing tags, one pair per line of the run,
/// walking backwards so earlier positions stay valid.
fn wrap_in_tag(
    sequence: &mut Vec<Token>,
    start: usize,
    end: usize,
    tag: DiffTag,
    generator: &TagGenerator,
    process: Option<&TextProcessor>,
    replace_linefeed: bool,
) {
    const LINEFEED: &[u16] = &[0x0A];
    let start = start as isize;
    let mut end_pos = end as isize;
    let at = |position: isize| (position - 1) as usize;
    let processed = |token: &Token| -> Token {
        match process {
            Some(process) => units(&process(&text_of(token))),
            None => token.clone(),
        }
    };
    while end_pos >= start {
        while end_pos > start {
            if sequence[at(end_pos)] != LINEFEED {
                break;
            } else if replace_linefeed {
                sequence[at(end_pos)] = vec![0x20];
                break;
            }
            end_pos -= 1;
        }
        if end_pos == start {
            break;
        }
        sequence.insert(end_pos as usize, units(&generator(tag, false)));
        if process.is_some() {
            sequence[at(end_pos)] = processed(&sequence[at(end_pos)]);
        }
        end_pos -= 1;
        while end_pos > start {
            if sequence[at(end_pos)] == LINEFEED {
                if replace_linefeed {
                    sequence[at(end_pos)] = vec![0x20];
                } else {
                    break;
                }
            }
            if process.is_some() {
                sequence[at(end_pos)] = processed(&sequence[at(end_pos)]);
            }
            end_pos -= 1;
        }
        sequence.insert(end_pos as usize, units(&generator(tag, true)));
        end_pos -= 1;
    }
}

impl DiffRowGenerator {
    pub fn builder() -> DiffRowGeneratorBuilder {
        DiffRowGeneratorBuilder::default()
    }

    /// `SPLITTER_BY_WORD`: every run of whitespace and every run of word delimiters is its own token, and the
    /// text between them is a token too.
    pub fn split_by_word(line: &str) -> Vec<String> {
        split_by_word(line)
    }

    /// `LINE_NORMALIZER_FOR_HTML`: `<` and `>` escaped and every tab expanded to four spaces.
    pub fn line_normalizer_for_html(line: &str) -> String {
        normalize_for_html(line)
    }

    /// `IGNORE_WHITESPACE_EQUALIZER`: equal after trimming and collapsing every whitespace run to one space.
    pub fn ignore_whitespace_equalizer(original: &str, revised: &str) -> bool {
        adjust_whitespace(original) == adjust_whitespace(revised)
    }

    /// `WHITESPACE_EQUALITIES_MERGER`: neighbouring inline deltas merged across gaps of whitespace tokens.
    pub fn whitespace_equalities_merger(info: &InlineDeltaMergeInfo) -> Vec<Delta> {
        whitespace_merger(info)
    }

    fn equal(&self, a: &String, b: &String) -> bool {
        match &self.options.equalizer {
            Some(equalizer) => equalizer(a, b),
            None if self.options.ignore_white_spaces => adjust_whitespace(a) == adjust_whitespace(b),
            None => a == b,
        }
    }

    pub fn generate_diff_rows(&self, original: &[String], revised: &[String]) -> Vec<DiffRow> {
        let patch = diff_by(original, revised, &|a, b| self.equal(a, b));
        self.generate_diff_rows_from_patch(original, &patch).unwrap_or_default()
    }

    /// [`DiffRowGenerator::generate_diff_rows`] with, for each row, the index of the `original` line
    /// and of the `revised` line it stands for (`None` for a side it has no line from).
    pub fn generate_diff_rows_with_origins(&self, original: &[String], revised: &[String]) -> Vec<(DiffRow, Option<usize>, Option<usize>)> {
        let patch = diff_by(original, revised, &|a, b| self.equal(a, b));
        self.rows_with_origins(original, &patch).unwrap_or_default()
    }

    /// Rows for `original` and a patch computed for it, as `generateDiffRows(List, Patch)` builds
    /// them; `INVALID_RANGE` where the reference throws `IndexOutOfBoundsException` or
    /// `IllegalArgumentException` (a patch whose positions do not fit `original`).
    pub fn generate_diff_rows_from_patch(&self, original: &[String], patch: &Patch) -> Result<Vec<DiffRow>, PatchFailed> {
        Ok(self.rows_with_origins(original, patch)?.into_iter().map(|(row, _, _)| row).collect())
    }

    fn rows_with_origins(&self, original: &[String], patch: &Patch) -> Result<Vec<OriginRow>, PatchFailed> {
        let mut rows = Vec::new();
        let mut end_pos = 0;
        // Revised index minus original index for the lines after the last delta.
        let mut shift: isize = 0;
        for delta in patch.deltas() {
            let parts = if self.options.decompress_deltas { decompress(delta) } else { vec![delta.clone()] };
            for part in &parts {
                end_pos = self.transform(original, end_pos, &mut rows, part)?;
                shift = (part.target.position + part.target.size()) as isize - (part.source.position + part.source.size()) as isize;
            }
        }
        for (offset, line) in sub_list(original, end_pos, original.len())?.iter().enumerate() {
            let processed = self.process_equality(line);
            let at = end_pos + offset;
            rows.push((self.build_row(DiffTag::Equal, &processed, &processed), Some(at), at.checked_add_signed(shift)));
        }
        Ok(rows)
    }

    fn transform(&self, original: &[String], end_pos: usize, rows: &mut Vec<OriginRow>, delta: &Delta) -> Result<usize, PatchFailed> {
        let (orig, rev) = (&delta.source, &delta.target);
        let gap = sub_list(original, end_pos, orig.position)?;
        for (k, line) in gap.iter().enumerate() {
            let processed = self.process_equality(line);
            let revised_at = (rev.position + k).checked_sub(gap.len());
            rows.push((self.build_row(DiffTag::Equal, &processed, &processed), Some(end_pos + k), revised_at));
        }
        match delta.kind {
            DeltaType::Equal => {
                for (j, line) in orig.lines.iter().enumerate() {
                    let processed = self.process_equality(line);
                    rows.push((self.build_row(DiffTag::Equal, &processed, &processed), Some(orig.position + j), Some(rev.position + j)));
                }
            }
            DeltaType::Insert => {
                for (j, line) in rev.lines.iter().enumerate() {
                    rows.push((self.build_row(DiffTag::Insert, "", line), None, Some(rev.position + j)));
                }
            }
            DeltaType::Delete => {
                for (j, line) in orig.lines.iter().enumerate() {
                    rows.push((self.build_row(DiffTag::Delete, line, ""), Some(orig.position + j), None));
                }
            }
            DeltaType::Change => {
                let changed = if self.options.show_inline_diffs {
                    self.inline_rows(delta)
                } else {
                    (0..orig.size().max(rev.size()))
                        .map(|j| {
                            let old = orig.lines.get(j).map_or("", String::as_str);
                            let new = rev.lines.get(j).map_or("", String::as_str);
                            self.build_row(DiffTag::Change, old, new)
                        })
                        .collect()
                };
                for (j, row) in changed.into_iter().enumerate() {
                    let old_at = (j < orig.size()).then_some(orig.position + j);
                    let new_at = (j < rev.size()).then_some(rev.position + j);
                    rows.push((row, old_at, new_at));
                }
            }
        }
        Ok(orig.position + orig.size())
    }

    fn build_row(&self, tag: DiffTag, old_line: &str, new_line: &str) -> DiffRow {
        let o = &self.options;
        if o.report_lines_unchanged {
            return DiffRow { tag, old_line: old_line.to_string(), new_line: new_line.to_string() };
        }
        let mut wrap_old = self.preprocess(old_line);
        if tag == DiffTag::Delete && (o.merge_original_revised || o.show_inline_diffs) {
            wrap_old = format!("{}{}{}", (o.old_tag)(tag, true), wrap_old, (o.old_tag)(tag, false));
        }
        let mut wrap_new = self.preprocess(new_line);
        if tag == DiffTag::Insert {
            if o.merge_original_revised {
                wrap_old = format!("{}{}{}", (o.new_tag)(tag, true), wrap_new, (o.new_tag)(tag, false));
            } else if o.show_inline_diffs {
                wrap_new = format!("{}{}{}", (o.new_tag)(tag, true), wrap_new, (o.new_tag)(tag, false));
            }
        }
        DiffRow { tag, old_line: wrap_old, new_line: wrap_new }
    }

    fn preprocess(&self, line: &str) -> String {
        java::wrap_text(&(self.options.line_normalizer)(line), self.options.column_width)
    }

    fn process_equality(&self, line: &str) -> String {
        match &self.options.process_equalities {
            Some(process) => process(line),
            None => line.to_string(),
        }
    }

    fn normalize_lines(&self, lines: &[String]) -> Vec<String> {
        if self.options.report_lines_unchanged {
            lines.to_vec()
        } else {
            lines.iter().map(|line| (self.options.line_normalizer)(line)).collect()
        }
    }

    fn tokens(&self, text: &str) -> Vec<Token> {
        match &self.options.splitter {
            TokenSplit::Character => java::utf16_units(text).into_iter().map(|unit| vec![unit]).collect(),
            TokenSplit::Word => split_by_word(text).iter().map(|token| units(token)).collect(),
            TokenSplit::Custom(split) => split(text).iter().map(|token| units(token)).collect(),
        }
    }

    fn equal_tokens(&self, a: &Token, b: &Token) -> bool {
        match &self.options.equalizer {
            Some(equalizer) => equalizer(&text_of(a), &text_of(b)),
            None if self.options.ignore_white_spaces => java::adjust_whitespace_units(a) == java::adjust_whitespace_units(b),
            None => a == b,
        }
    }

    fn inline_rows(&self, delta: &Delta) -> Vec<DiffRow> {
        let o = &self.options;
        let mut orig_list = self.tokens(&self.normalize_lines(&delta.source.lines).join("\n"));
        let mut rev_list = self.tokens(&self.normalize_lines(&delta.target.lines).join("\n"));
        let found = diff_by(&orig_list, &rev_list, &|a, b| self.equal_tokens(a, b));
        let as_text = |tokens: &[Token]| -> Vec<String> { tokens.iter().map(|token| text_of(token)).collect() };
        let deltas = found
            .deltas()
            .iter()
            .map(|d| {
                Delta::new(
                    d.kind,
                    Chunk::new(d.source.position, as_text(&d.source.lines)),
                    Chunk::new(d.target.position, as_text(&d.target.lines)),
                )
            })
            .collect();
        let info = InlineDeltaMergeInfo { deltas, orig_list: as_text(&orig_list), rev_list: as_text(&rev_list) };
        let inline = (o.merger)(&info);
        let process = o.process_diffs.as_ref();
        let replace_linefeed = o.replace_original_linefeed_in_changes_with_spaces && o.merge_original_revised;
        for d in inline.iter().rev() {
            let (op, os) = (d.source.position, d.source.size());
            let (rp, rs) = (d.target.position, d.target.size());
            match d.kind {
                DeltaType::Delete => {
                    wrap_in_tag(&mut orig_list, op, op + os, DiffTag::Delete, &o.old_tag, process, replace_linefeed);
                }
                DeltaType::Insert => {
                    if o.merge_original_revised {
                        orig_list.splice(op..op, rev_list[rp..rp + rs].iter().cloned());
                        wrap_in_tag(&mut orig_list, op, op + rs, DiffTag::Insert, &o.new_tag, process, false);
                    } else {
                        wrap_in_tag(&mut rev_list, rp, rp + rs, DiffTag::Insert, &o.new_tag, process, false);
                    }
                }
                DeltaType::Change => {
                    if o.merge_original_revised {
                        let at = op + os;
                        orig_list.splice(at..at, rev_list[rp..rp + rs].iter().cloned());
                        wrap_in_tag(&mut orig_list, at, at + rs, DiffTag::Change, &o.new_tag, process, false);
                    } else {
                        wrap_in_tag(&mut rev_list, rp, rp + rs, DiffTag::Change, &o.new_tag, process, false);
                    }
                    wrap_in_tag(&mut orig_list, op, op + os, DiffTag::Change, &o.old_tag, process, replace_linefeed);
                }
                DeltaType::Equal => {}
            }
        }
        let original = text_of(&orig_list.concat());
        let revised = text_of(&rev_list.concat());
        let original = java::split(&original, "\n");
        let revised = java::split(&revised, "\n");
        (0..original.len().max(revised.len()))
            .map(|j| DiffRow {
                tag: DiffTag::Change,
                old_line: java::wrap_text(original.get(j).copied().unwrap_or(""), o.column_width),
                new_line: java::wrap_text(revised.get(j).copied().unwrap_or(""), o.column_width),
            })
            .collect()
    }
}

/// `List.subList(from, to)` of the reference: `INVALID_RANGE` when `from > to` or `to` is past the end.
fn sub_list(lines: &[String], from: usize, to: usize) -> Result<&[String], PatchFailed> {
    if from > to || to > lines.len() {
        return Err(PatchFailed::new("INVALID_RANGE"));
    }
    Ok(&lines[from..to])
}
