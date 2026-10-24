use std::{
    collections::VecDeque,
    sync::Arc,
    time::{Duration, Instant},
};

pub(super) const MAX_HISTORY_BYTES: usize = 256 * 1024;
const MAX_STEPS: usize = 64;
const MERGE_WINDOW: Duration = Duration::from_secs(1);

#[derive(Clone, PartialEq, Eq)]
pub(super) struct Point {
    pub text: Arc<str>,
    pub selection: [usize; 2],
}

#[derive(Clone)]
struct Step {
    before: Point,
    after: Point,
}

#[derive(Clone, Default)]
pub(super) struct History {
    undo: VecDeque<Step>,
    redo: VecDeque<Step>,
    pub pending: Option<Point>,
    typing: Option<Instant>,
}

impl History {
    pub fn bytes(&self) -> usize {
        self.undo
            .iter()
            .chain(&self.redo)
            .map(|s| s.before.text.len() + s.after.text.len())
            .sum::<usize>()
            + self.pending.as_ref().map_or(0, |p| p.text.len())
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    pub fn boundary(&mut self) {
        self.typing = None;
    }

    pub fn trim(&mut self) {
        while self.bytes() > MAX_HISTORY_BYTES || self.undo.len() + self.redo.len() > MAX_STEPS {
            if self.undo.pop_front().is_none() && self.redo.pop_front().is_none() {
                break;
            }
        }
    }

    pub fn record(&mut self, before: Point, after: Point, typing: bool, now: Instant) {
        if before.text == after.text {
            return;
        }
        self.redo.clear();
        let merge = typing
            && self
                .typing
                .is_some_and(|last| now.saturating_duration_since(last) <= MERGE_WINDOW)
            && self.undo.back().is_some_and(|s| s.after == before);
        if merge {
            self.undo.back_mut().unwrap().after = after;
        } else {
            self.undo.push_back(Step { before, after });
        }
        self.typing = typing.then_some(now);
        self.trim();
    }

    pub fn undo(&mut self) -> Option<Point> {
        self.boundary();
        if let Some(before) = self.pending.take() {
            return Some(before);
        }
        let step = self.undo.pop_back()?;
        let result = step.before.clone();
        self.redo.push_back(step);
        Some(result)
    }

    pub fn redo(&mut self) -> Option<Point> {
        self.boundary();
        if self.pending.is_some() {
            return None;
        }
        let step = self.redo.pop_back()?;
        let result = step.after.clone();
        self.undo.push_back(step);
        Some(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn point(text: &str) -> Point {
        Point {
            text: text.into(),
            selection: [text.len(); 2],
        }
    }

    #[test]
    fn typing_merges_only_contiguous_edits_within_the_time_window() {
        let mut h = History::default();
        let now = Instant::now();
        h.record(point(""), point("a"), true, now);
        h.record(
            point("a"),
            point("ab"),
            true,
            now + Duration::from_millis(500),
        );
        h.record(
            point("ab"),
            point("abc"),
            true,
            now + Duration::from_secs(2),
        );
        assert_eq!(h.undo().unwrap().text.as_ref(), "ab");
        assert_eq!(h.undo().unwrap().text.as_ref(), "");
        assert_eq!(h.redo().unwrap().text.as_ref(), "ab");
        h.record(
            point("ab"),
            point("abz"),
            true,
            now + Duration::from_secs(3),
        );
        assert!(!h.can_redo());
        assert_eq!(h.undo().unwrap().text.as_ref(), "ab");
    }

    #[test]
    fn budgets_cover_both_stacks_and_pending_composition() {
        let mut h = History::default();
        let now = Instant::now();
        for i in 0..100 {
            h.record(
                point(&i.to_string()),
                point(&(i + 1).to_string()),
                false,
                now,
            );
        }
        assert_eq!(h.undo.len(), MAX_STEPS);
        for _ in 0..30 {
            h.undo();
        }
        assert_eq!(h.undo.len() + h.redo.len(), MAX_STEPS);
        h.record(
            point(&"a".repeat(65536)),
            point(&"b".repeat(65536)),
            false,
            now,
        );
        h.record(
            point(&"b".repeat(65536)),
            point(&"c".repeat(65536)),
            false,
            now,
        );
        assert!(h.bytes() <= MAX_HISTORY_BYTES);
        h.pending = Some(point(&"c".repeat(65536)));
        h.trim();
        assert!(h.bytes() <= MAX_HISTORY_BYTES);
        assert_eq!(h.undo.len(), 1);
        h.undo();
        assert_eq!(h.undo().unwrap().text.as_ref(), "b".repeat(65536));
    }
}
