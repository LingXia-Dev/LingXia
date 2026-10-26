//! Rich text: a `<text>` whose children are `<span>` and `<link>` runs, shaped
//! as one paragraph with per-run styles.
use crate::protocol::{Record, tag};
use crate::text::style::{Collapse, TextStyle};
use std::ops::Range;

/// One paragraph's display text and the byte range each run record covers.
pub(crate) struct Paragraph {
    pub text: String,
    /// Run record index and its range in `text`, in order.
    pub runs: Vec<(usize, Range<usize>)>,
}

impl Paragraph {
    /// The run a byte offset belongs to.
    pub(crate) fn run_at(&self, brush: crate::text::RunBrush) -> Option<usize> {
        self.runs
            .get(usize::from(brush).checked_sub(1)?)
            .map(|(record, _)| *record)
    }
}

/// Collapses whitespace across run boundaries the way CSS does inside one block.
struct Collapser {
    mode: Collapse,
    out: String,
    pending: bool,
}

impl Collapser {
    fn push(&mut self, text: &str) {
        if self.mode == Collapse::None {
            self.out.push_str(text);
            return;
        }
        for c in text.chars() {
            let newline = matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}');
            if newline && self.mode == Collapse::Spaces {
                self.pending = false;
                self.out
                    .truncate(self.out.trim_end_matches([' ', '\t']).len());
                self.out.push('\n');
            } else if c.is_whitespace() {
                self.pending |= !self.out.is_empty() && !self.out.ends_with('\n');
            } else {
                if self.pending {
                    self.out.push(' ');
                }
                self.pending = false;
                self.out.push(c);
            }
        }
    }
}

/// The paragraph of every `<text>` with run children; `None` elsewhere.
pub(crate) fn paragraphs(records: &[Record], styles: &[TextStyle]) -> Vec<Option<Paragraph>> {
    let mut runs: Vec<Vec<usize>> = vec![Vec::new(); records.len()];
    for (index, record) in records.iter().enumerate() {
        if matches!(record.tag, tag::SPAN | tag::LINK)
            && let Some(parent) = record.parent
        {
            runs[parent].push(index);
        }
    }
    runs.into_iter()
        .enumerate()
        .map(|(index, members)| {
            if members.is_empty() {
                return None;
            }
            let mut collapser = Collapser {
                mode: styles[index].collapse,
                out: String::new(),
                pending: false,
            };
            let mut ranges = Vec::with_capacity(members.len());
            for member in members {
                let start = collapser.out.len();
                collapser.push(&styles[member].transform.apply(&records[member].text));
                ranges.push((member, start..collapser.out.len()));
            }
            Some(Paragraph {
                text: collapser.out,
                runs: ranges,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whitespace_collapses_across_run_boundaries() {
        let mut collapser = Collapser {
            mode: Collapse::All,
            out: String::new(),
            pending: false,
        };
        let mut ends = Vec::new();
        for part in ["  Hello ", " bold ", "world  "] {
            collapser.push(part);
            ends.push(collapser.out.len());
        }
        assert_eq!(collapser.out, "Hello bold world");
        assert_eq!(ends, [5, 10, 16]);
    }
}
