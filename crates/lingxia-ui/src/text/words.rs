//! Word selection that never lands on the space between words.
use std::ops::Range;

/// `range` from a word hit, or the word nearest `at` when the hit is only whitespace.
pub(crate) fn snap(text: &str, range: Range<usize>, at: usize) -> Range<usize> {
    let gap = text.get(range.clone()).unwrap_or("");
    if gap.chars().any(|c| !c.is_whitespace()) || text.trim().is_empty() {
        return range;
    }
    let start = range.start.min(text.len());
    let end = range.end.min(text.len());
    let before = text[..start].trim_end();
    let after_start = end + (text[end..].len() - text[end..].trim_start().len());
    let previous = (!before.is_empty()).then(|| {
        let word_start = before
            .char_indices()
            .rev()
            .find(|(_, c)| c.is_whitespace())
            .map(|(i, c)| i + c.len_utf8())
            .unwrap_or(0);
        word_start..before.len()
    });
    let next = (after_start < text.len()).then(|| {
        let word_end = text[after_start..]
            .char_indices()
            .find(|(_, c)| c.is_whitespace())
            .map(|(i, _)| after_start + i)
            .unwrap_or(text.len());
        after_start..word_end
    });
    match (previous, next) {
        (Some(previous), Some(next)) => {
            if at.saturating_sub(previous.end) <= next.start.saturating_sub(at) {
                previous
            } else {
                next
            }
        }
        (Some(word), None) | (None, Some(word)) => word,
        (None, None) => range,
    }
}

#[cfg(test)]
mod tests {
    use super::snap;

    #[test]
    fn a_hit_on_whitespace_takes_the_nearest_word() {
        let text = "select  the   word";
        assert_eq!(snap(text, 6..8, 6), 0..6);
        assert_eq!(snap(text, 6..8, 8), 8..11);
        assert_eq!(snap(text, 11..14, 13), 14..18);
        assert_eq!(snap(text, 0..6, 3), 0..6);
        assert_eq!(snap("  lead", 0..2, 0), 2..6);
        assert_eq!(snap("tail  ", 4..6, 6), 0..4);
        assert_eq!(snap("   ", 0..3, 1), 0..3);
    }
}
