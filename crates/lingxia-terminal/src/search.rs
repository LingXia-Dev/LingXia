//! Scrollback search over the complete logical line space.
//!
//! Search runs against text copied out of the grid, never against live
//! terminal state, so long patterns cannot stall PTY processing and no
//! session lock is held while matching. Wrapped rows are joined into
//! logical lines first; match ranges are reported back in physical
//! (line, cell column) coordinates so hosts can highlight and scroll
//! to them directly.

use std::sync::atomic::{AtomicBool, Ordering};

/// One physical grid row copied for searching.
#[derive(Debug, Clone, Default)]
pub struct SearchRow {
    /// Absolute line index (oldest scrollback line = 0).
    pub line: i64,
    /// Visible text of the row, trailing blanks trimmed.
    pub text: String,
    /// Per character in `text`: the cell column and cell width it
    /// came from, so wide characters map back to two columns.
    pub cells: Vec<(u16, u8)>,
    /// Whether the next physical row continues this logical line.
    pub wraps: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchMode {
    /// Case-insensitive plain text.
    Plain,
    /// Case-sensitive plain text.
    CaseSensitive,
    /// Case-insensitive plain text, bounded by non-word characters.
    WholeWord,
    /// Case-sensitive plain text, bounded by non-word characters.
    CaseSensitiveWholeWord,
    /// Regular expression.
    Regex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchMatch {
    pub start_line: i64,
    pub start_col: u16,
    pub end_line: i64,
    /// Exclusive end cell column on `end_line`.
    pub end_col: u16,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResults {
    pub matches: Vec<SearchMatch>,
    /// Total matches found (beyond `max_matches` when truncated).
    pub total: u64,
    pub truncated: bool,
    pub cancelled: bool,
}

/// Check cancellation every this many logical lines.
const CANCEL_CHECK_LINES: usize = 256;

/// A logical line: concatenation of wrapped physical rows plus the
/// mapping from character offsets back to physical coordinates.
struct LogicalLine {
    text: String,
    /// Per physical row segment: (absolute line, first char offset in
    /// the logical text, chars of this segment).
    segments: Vec<(i64, usize, usize)>,
}

fn build_logical_lines(rows: &[SearchRow]) -> Vec<LogicalLine> {
    let mut lines: Vec<LogicalLine> = Vec::new();
    let mut previous_wraps = false;
    for row in rows {
        // A row continues the logical line when the previous physical
        // row wrapped into it.
        let continues = previous_wraps;
        previous_wraps = row.wraps;
        if row.text.is_empty() && !row.wraps {
            continue;
        }
        let char_count = row.text.chars().count();
        if continues && let Some(last) = lines.last_mut() {
            let offset = last.text.chars().count();
            last.text.push_str(&row.text);
            last.segments.push((row.line, offset, char_count));
            continue;
        }
        lines.push(LogicalLine {
            text: row.text.clone(),
            segments: vec![(row.line, 0, char_count)],
        });
    }
    lines
}

/// Map a character offset inside a logical line to (line, cell column).
fn locate(rows: &[SearchRow], segments: &[(i64, usize, usize)], offset: usize) -> (i64, u16) {
    for &(line, start, len) in segments {
        if offset > start + len {
            continue;
        }
        let Ok(index) = rows.binary_search_by_key(&line, |row| row.line) else {
            return (line, 0);
        };
        let row = &rows[index];
        let char_index = offset.saturating_sub(start);
        return match row.cells.get(char_index) {
            Some(&(col, _)) => (line, col),
            // At/past the last character: one cell past the row's end.
            None => {
                let end = row
                    .cells
                    .last()
                    .map(|&(col, width)| col.saturating_add(u16::from(width)))
                    .unwrap_or(0);
                (line, end)
            }
        };
    }
    segments
        .last()
        .map(|&(line, _, _)| (line, 0))
        .unwrap_or((0, 0))
}

/// Search the copied rows. `cancel` is polled between logical-line
/// chunks; `max_matches` bounds the result vector.
pub fn search_rows(
    rows: &[SearchRow],
    pattern: &str,
    mode: SearchMode,
    max_matches: usize,
    cancel: &AtomicBool,
) -> SearchResults {
    let mut results = SearchResults::default();
    if pattern.is_empty() {
        return results;
    }
    let matcher = match Matcher::new(pattern, mode) {
        Some(matcher) => matcher,
        None => return results,
    };
    let logical = build_logical_lines(rows);
    for (index, line) in logical.iter().enumerate() {
        if index % CANCEL_CHECK_LINES == 0 && cancel.load(Ordering::Relaxed) {
            results.cancelled = true;
            break;
        }
        for (start_char, end_char) in matcher.find(line) {
            results.total += 1;
            if results.matches.len() >= max_matches {
                results.truncated = true;
                continue;
            }
            let (start_line, start_col) = locate(rows, &line.segments, start_char);
            let (end_line, end_col) = locate(rows, &line.segments, end_char);
            results.matches.push(SearchMatch {
                start_line,
                start_col,
                end_line,
                end_col,
            });
        }
    }
    results
}

enum Matcher {
    Plain(String),
    PlainCase(String),
    WholeWord(String),
    WholeWordCase(String),
    Regex(regex::Regex),
}

impl Matcher {
    fn new(pattern: &str, mode: SearchMode) -> Option<Self> {
        match mode {
            SearchMode::Plain => Some(Self::Plain(pattern.to_lowercase())),
            SearchMode::CaseSensitive => Some(Self::PlainCase(pattern.to_string())),
            SearchMode::WholeWord => Some(Self::WholeWord(pattern.to_lowercase())),
            SearchMode::CaseSensitiveWholeWord => Some(Self::WholeWordCase(pattern.to_string())),
            SearchMode::Regex => regex::Regex::new(pattern).ok().map(Self::Regex),
        }
    }

    /// All matches as (start, end) character offsets in `line.text`.
    fn find(&self, line: &LogicalLine) -> Vec<(usize, usize)> {
        match self {
            Matcher::Regex(regex) => {
                // Regex offsets are byte offsets; convert to chars.
                let byte_to_char: Vec<usize> = line
                    .text
                    .char_indices()
                    .map(|(byte, _)| byte)
                    .chain(std::iter::once(line.text.len()))
                    .collect();
                regex
                    .find_iter(&line.text)
                    .map(|m| {
                        (
                            byte_to_char
                                .iter()
                                .position(|&b| b == m.start())
                                .unwrap_or(0),
                            byte_to_char
                                .iter()
                                .position(|&b| b == m.end())
                                .unwrap_or(byte_to_char.len() - 1),
                        )
                    })
                    .collect()
            }
            Matcher::Plain(needle) => find_plain(&line.text.to_lowercase(), needle),
            Matcher::PlainCase(needle) => find_plain(&line.text, needle),
            Matcher::WholeWord(needle) => find_plain(&line.text.to_lowercase(), needle)
                .into_iter()
                .filter(|&(start, end)| has_word_boundaries(&line.text, start, end))
                .collect(),
            Matcher::WholeWordCase(needle) => find_plain(&line.text, needle)
                .into_iter()
                .filter(|&(start, end)| has_word_boundaries(&line.text, start, end))
                .collect(),
        }
    }
}

fn has_word_boundaries(text: &str, start: usize, end: usize) -> bool {
    let chars: Vec<char> = text.chars().collect();
    let is_word = |character: char| character.is_alphanumeric() || character == '_';
    chars
        .get(start.wrapping_sub(1))
        .is_none_or(|character| !is_word(*character))
        && chars.get(end).is_none_or(|character| !is_word(*character))
}

/// Character-offset matches of `needle` in `haystack` (both already
/// case-folded by the caller as needed).
fn find_plain(haystack: &str, needle: &str) -> Vec<(usize, usize)> {
    let mut matches = Vec::new();
    let needle_chars = needle.chars().count();
    if needle_chars == 0 {
        return matches;
    }
    let hay: Vec<char> = haystack.chars().collect();
    let needle_chars_vec: Vec<char> = needle.chars().collect();
    let mut index = 0;
    while index + needle_chars <= hay.len() {
        if hay[index..index + needle_chars] == needle_chars_vec[..] {
            matches.push((index, index + needle_chars));
            index += needle_chars;
        } else {
            index += 1;
        }
    }
    matches
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(line: i64, text: &str, wraps: bool) -> SearchRow {
        let cells = text
            .chars()
            .enumerate()
            .map(|(index, _)| (index as u16, 1u8))
            .collect();
        SearchRow {
            line,
            text: text.to_string(),
            cells,
            wraps,
        }
    }

    fn search(rows: &[SearchRow], pattern: &str, mode: SearchMode) -> SearchResults {
        search_rows(rows, pattern, mode, 1000, &AtomicBool::new(false))
    }

    #[test]
    fn plain_search_is_case_insensitive() {
        let rows = vec![row(0, "Hello World", false), row(1, "hello again", false)];
        let results = search(&rows, "hello", SearchMode::Plain);
        assert_eq!(results.total, 2);
        assert_eq!(results.matches[0].start_line, 0);
        assert_eq!(results.matches[0].start_col, 0);
        assert_eq!(results.matches[0].end_col, 5);
        assert_eq!(results.matches[1].start_line, 1);
    }

    #[test]
    fn case_sensitive_search_respects_case() {
        let rows = vec![row(0, "Hello hello", false)];
        let results = search(&rows, "Hello", SearchMode::CaseSensitive);
        assert_eq!(results.total, 1);
        assert_eq!(results.matches[0].start_col, 0);
    }

    #[test]
    fn whole_word_search_ignores_substrings() {
        let rows = vec![row(0, "cat catalog cat_2", false)];
        let results = search(&rows, "cat", SearchMode::WholeWord);
        assert_eq!(results.total, 1);
        assert_eq!(results.matches[0].start_col, 0);
    }

    #[test]
    fn regex_search_reports_matches() {
        let rows = vec![row(0, "error: code 42, code 7", false)];
        let results = search(&rows, r"code \d+", SearchMode::Regex);
        assert_eq!(results.total, 2);
        assert_eq!(results.matches[0].start_col, 7);
        assert_eq!(results.matches[0].end_col, 14);
        let invalid = search(&rows, r"(unclosed", SearchMode::Regex);
        assert_eq!(invalid.total, 0, "invalid regex yields no matches");
    }

    #[test]
    fn matches_spanning_wrapped_rows_report_both_lines() {
        let rows = vec![
            row(0, "a very long tok", true),
            row(1, "en continues", false),
        ];
        let results = search(&rows, "token", SearchMode::Plain);
        assert_eq!(results.total, 1);
        let found = results.matches[0];
        assert_eq!((found.start_line, found.start_col), (0, 12));
        assert_eq!((found.end_line, found.end_col), (1, 2));
    }

    #[test]
    fn search_can_be_cancelled_and_truncated() {
        let rows: Vec<SearchRow> = (0..1000).map(|line| row(line, "hit", false)).collect();
        let cancel = AtomicBool::new(true);
        let results = search_rows(&rows, "hit", SearchMode::Plain, 1000, &cancel);
        assert!(results.cancelled);
        assert_eq!(results.total, 0);

        let results = search_rows(&rows, "hit", SearchMode::Plain, 10, &AtomicBool::new(false));
        assert!(results.truncated);
        assert_eq!(results.matches.len(), 10);
        assert_eq!(results.total, 1000);
    }

    #[test]
    fn wide_characters_map_back_to_cell_columns() {
        // '中' occupies two cells; the search row records real columns.
        let text = "x中y";
        let cells = vec![(0u16, 1u8), (1, 2), (3, 1)];
        let rows = vec![SearchRow {
            line: 0,
            text: text.to_string(),
            cells,
            wraps: false,
        }];
        let results = search(&rows, "y", SearchMode::Plain);
        assert_eq!(results.matches[0].start_col, 3);
        assert_eq!(results.matches[0].end_col, 4);
    }
}
