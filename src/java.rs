//! The Java string and regular-expression semantics the reference library relies on, in one place:
//! `java.util.regex` `\s`, `.` and `$` (no flags), `String.trim`, `String.split` with a literal
//! separator, UTF-16 lengths, and `BufferedReader.readLine` line splitting.

/// `\s` of `java.util.regex` without UNICODE_CHARACTER_CLASS.
pub(crate) fn is_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\u{b}' | '\u{c}' | '\r')
}

/// The characters `.` of `java.util.regex` does not match without DOTALL.
pub(crate) fn is_line_terminator(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{85}' | '\u{2028}' | '\u{2029}')
}

/// Whether `$` (no MULTILINE) matches at byte offset `at` of `text`: at the end, or before one
/// final line terminator (`\r\n` counting as one).
pub(crate) fn dollar_at(text: &str, at: usize) -> bool {
    let rest = &text[at..];
    if rest.is_empty() || rest == "\r\n" {
        return true;
    }
    let mut chars = rest.chars();
    match (chars.next(), chars.next()) {
        (Some('\n'), None) => !text[..at].ends_with('\r'),
        (Some(c), None) => is_line_terminator(c),
        _ => false,
    }
}

/// `(.+)$` at the start of `text`: the captured run when it matches.
pub(crate) fn rest_of_line(text: &str) -> Option<&str> {
    let end = text.find(is_line_terminator).unwrap_or(text.len());
    (end > 0 && dollar_at(text, end)).then(|| &text[..end])
}

/// `String.trim`: strips every char up to U+0020 at both ends.
pub(crate) fn trim(text: &str) -> &str {
    text.trim_matches(|c: char| c <= ' ')
}

/// `String.split` with a literal separator: no match gives the whole text, and trailing empty
/// strings are removed.
pub(crate) fn split<'a>(text: &'a str, separator: &str) -> Vec<&'a str> {
    if !text.contains(separator) {
        return vec![text];
    }
    let mut parts: Vec<&str> = text.split(separator).collect();
    while parts.last() == Some(&"") {
        parts.pop();
    }
    parts
}

/// Replace every run of `\s` characters with `replacement`, as `replaceAll("\\s+", ...)` does.
pub(crate) fn replace_space_runs(text: &str, replacement: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_run = false;
    for c in text.chars() {
        if is_space(c) {
            if !in_run {
                out.push_str(replacement);
            }
            in_run = true;
        } else {
            out.push(c);
            in_run = false;
        }
    }
    out
}

/// Leading ASCII digits of `text` (`\d+`), with the rest.
pub(crate) fn digits(text: &str) -> Option<(&str, &str)> {
    let end = text.find(|c: char| !c.is_ascii_digit()).unwrap_or(text.len());
    (end > 0).then(|| text.split_at(end))
}

/// `Integer.valueOf` of a run of ASCII digits; `None` when it overflows an `int`.
pub(crate) fn int_value(digits: &str) -> Option<i32> {
    digits.parse::<i32>().ok()
}

/// One or more `\s` characters at the start of `text`, with the rest.
pub(crate) fn spaces(text: &str) -> Option<&str> {
    let rest = text.trim_start_matches(is_space);
    (rest.len() < text.len()).then_some(rest)
}

/// Exactly one `\s` character at the start of `text`, with the rest.
pub(crate) fn one_space(text: &str) -> Option<&str> {
    text.strip_prefix(is_space)
}

pub(crate) fn utf16_len(text: &str) -> usize {
    text.encode_utf16().count()
}

/// The UTF-16 code units of `text`, the `char`s of a Java string.
pub(crate) fn utf16_units(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}

/// The string of some UTF-16 code units; an unpaired surrogate becomes U+FFFD.
pub(crate) fn from_utf16(units: &[u16]) -> String {
    String::from_utf16_lossy(units)
}

/// [`trim`] and then [`replace_space_runs`] with one space, over UTF-16 code units, where a half of a surrogate
/// pair is neither trimmed nor a space.
pub(crate) fn adjust_whitespace_units(units: &[u16]) -> Vec<u16> {
    let trimmed = |unit: &u16| *unit <= 0x20;
    let start = units.iter().position(|u| !trimmed(u)).unwrap_or(units.len());
    let end = units.iter().rposition(|u| !trimmed(u)).map_or(start, |at| at + 1);
    let mut out = Vec::with_capacity(end - start);
    let mut in_run = false;
    for &unit in &units[start..end] {
        if char::from_u32(u32::from(unit)).is_some_and(is_space) {
            if !in_run {
                out.push(0x20);
            }
            in_run = true;
        } else {
            out.push(unit);
            in_run = false;
        }
    }
    out
}

/// `StringUtils.wrapText`: insert `<br/>` every `width` UTF-16 code units of the original line,
/// shifting a break that would split a surrogate pair.
pub(crate) fn wrap_text(line: &str, width: usize) -> String {
    if width == 0 {
        return line.to_string();
    }
    const BREAK: &str = "<br/>";
    let delimiter = BREAK.len();
    let mut units: Vec<u16> = line.encode_utf16().collect();
    let length = units.len();
    let mut width_index = width;
    let mut count = 0;
    while length > width_index {
        let mut at = width_index + delimiter * count;
        if is_high_surrogate(units[at - 1]) && is_low_surrogate(units[at]) {
            at += 1;
            if at == units.len() {
                at -= 2;
            }
        }
        units.splice(at..at, BREAK.encode_utf16());
        width_index += width;
        count += 1;
    }
    String::from_utf16_lossy(&units)
}

fn is_high_surrogate(unit: u16) -> bool {
    (0xD800..0xDC00).contains(&unit)
}

fn is_low_surrogate(unit: u16) -> bool {
    (0xDC00..0xE000).contains(&unit)
}

/// `BufferedReader` over a text: `read_line` splits at `\n`, `\r` and `\r\n`, and `ready` tells
/// whether characters remain.
pub(crate) struct LineReader<'a> {
    text: &'a str,
    at: usize,
    lines_read: usize,
}

impl<'a> LineReader<'a> {
    pub(crate) fn new(text: &'a str) -> LineReader<'a> {
        LineReader { text, at: 0, lines_read: 0 }
    }

    pub(crate) fn read_line(&mut self) -> Option<&'a str> {
        if self.at >= self.text.len() {
            return None;
        }
        self.lines_read += 1;
        let rest = &self.text[self.at..];
        match rest.find(['\n', '\r']) {
            Some(end) => {
                let skip = if rest[end..].starts_with("\r\n") { 2 } else { 1 };
                self.at += end + skip;
                Some(&rest[..end])
            }
            None => {
                self.at = self.text.len();
                Some(rest)
            }
        }
    }

    /// How many lines `read_line` has returned so far.
    pub(crate) fn lines_read(&self) -> usize {
        self.lines_read
    }

    pub(crate) fn ready(&self) -> bool {
        self.at < self.text.len()
    }
}
