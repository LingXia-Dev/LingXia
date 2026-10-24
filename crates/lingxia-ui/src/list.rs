//! Row geometry for windowed lists. Owners commit a candidate index with its View frame.
use std::{collections::HashSet, ops::Range};

pub const MAX_ROWS: usize = 100_000;
const MAX_HEIGHT: f64 = 16_384.0;

pub(crate) fn item_key(value: &str) -> String {
    format!("key:{}", serde_json::to_string(value).unwrap())
}

pub(crate) fn decode_item_keys(source: &str) -> Result<Vec<String>, String> {
    if source.len() > 262_144 {
        return Err("UI_LIST: itemKeys budget exceeded".into());
    }
    let keys: Vec<String> =
        serde_json::from_str(source).map_err(|_| "UI_LIST: invalid itemKeys")?;
    if keys.len() > MAX_ROWS {
        return Err("UI_LIST: too many itemKeys".into());
    }
    let mut seen = HashSet::new();
    for key in &keys {
        if item_key(key).len() > 256 || !seen.insert(key) {
            return Err("UI_LIST: invalid or duplicate itemKey".into());
        }
    }
    Ok(keys)
}

#[derive(Clone, Debug)]
pub struct ListMetrics {
    estimate: f64,
    measured: Vec<Option<f64>>,
    sums: Vec<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Window {
    pub rows: Range<usize>,
    pub leading: f64,
    pub trailing: f64,
    pub total: f64,
    pub offset: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Start,
    End,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anchor {
    End,
    Row { index: usize, align: Align },
}

impl ListMetrics {
    pub fn new(count: usize, estimate: f64) -> Result<Self, String> {
        validate_count(count)?;
        validate_estimate(estimate)?;
        let mut result = Self {
            estimate,
            measured: vec![None; count],
            sums: Vec::new(),
        };
        result.rebuild();
        Ok(result)
    }

    pub fn len(&self) -> usize {
        self.measured.len()
    }

    pub fn is_empty(&self) -> bool {
        self.measured.is_empty()
    }

    pub fn total(&self) -> f64 {
        self.prefix(self.len())
    }

    /// Half-open row window, with caller-selected overscan on both sides.
    pub fn window(&self, offset: f64, viewport: f64, overscan: f64) -> Result<Window, String> {
        validate_viewport(offset, viewport)?;
        if !overscan.is_finite() || overscan < 0.0 {
            return Err("UI_LIST: invalid overscan".into());
        }
        let total = self.total();
        let offset = self.clamp_offset(offset, viewport);
        let start = (offset - overscan).max(0.0);
        let end = (offset + viewport + overscan).min(total);
        let rows = if viewport == 0.0 || end <= start {
            let index = self.partition(start, true);
            index..index
        } else {
            self.partition(start, true)..(self.partition(end, false) + 1).min(self.len())
        };
        Ok(Window {
            leading: self.prefix(rows.start),
            trailing: (total - self.prefix(rows.end)).max(0.0),
            rows,
            total,
            offset,
        })
    }

    /// Apply a measured batch atomically, keeping the first visible row at the same pixel.
    pub fn measure(
        &mut self,
        rows: &[(usize, f64)],
        offset: f64,
        viewport: f64,
    ) -> Result<f64, String> {
        validate_viewport(offset, viewport)?;
        if rows.len() > self.len() {
            return Err("UI_LIST: too many measurements".into());
        }
        let mut seen = HashSet::new();
        for &(index, height) in rows {
            if index >= self.len()
                || !height.is_finite()
                || !(0.0..=MAX_HEIGHT).contains(&height)
                || !seen.insert(index)
            {
                return Err("UI_LIST: invalid measurement".into());
            }
        }
        let position = self.position(offset, viewport);
        for &(index, height) in rows {
            let previous = self.height(index);
            self.measured[index] = Some(height);
            if previous == height {
                continue;
            }
            let mut node = index + 1;
            while node <= self.len() {
                self.rebuild_node(node);
                node += low_bit(node);
            }
        }
        Ok(self.restore(position, viewport))
    }

    /// Existing row indices retain measurements; removed rows cannot revive on growth.
    pub fn resize(&mut self, count: usize, offset: f64, viewport: f64) -> Result<f64, String> {
        validate_count(count)?;
        validate_viewport(offset, viewport)?;
        if count == self.len() {
            return Ok(self.clamp_offset(offset, viewport));
        }
        let position = self.position(offset, viewport);
        self.measured.resize(count, None);
        self.rebuild();
        Ok(self.restore(position, viewport))
    }

    /// Width/font changes invalidate measurements while preserving the first visible row index.
    pub fn invalidate(&mut self, estimate: f64, offset: f64, viewport: f64) -> Result<f64, String> {
        validate_estimate(estimate)?;
        validate_viewport(offset, viewport)?;
        let position = self.position(offset, viewport);
        self.estimate = estimate;
        self.measured.fill(None);
        self.rebuild();
        Ok(self.restore(position, viewport))
    }

    /// Call only when the declarative anchor (including its sequence) changes.
    pub fn anchor_offset(&self, anchor: Anchor, viewport: f64) -> Result<f64, String> {
        validate_viewport(0.0, viewport)?;
        let offset = match anchor {
            Anchor::End => self.total(),
            Anchor::Row { index, align } => {
                if index >= self.len() {
                    return Err("UI_LIST: anchor index out of range".into());
                }
                match align {
                    Align::Start => self.prefix(index),
                    Align::End => self.prefix(index + 1) - viewport,
                }
            }
        };
        Ok(self.clamp_offset(offset, viewport))
    }

    fn height(&self, index: usize) -> f64 {
        self.measured[index].unwrap_or(self.estimate)
    }

    fn clamp_offset(&self, offset: f64, viewport: f64) -> f64 {
        offset.clamp(0.0, (self.total() - viewport).max(0.0))
    }

    fn position(&self, offset: f64, viewport: f64) -> (usize, f64) {
        let offset = self.clamp_offset(offset, viewport);
        let index = self.partition(offset, true);
        (index, (offset - self.prefix(index)).max(0.0))
    }

    fn restore(&self, (index, within): (usize, f64), viewport: f64) -> f64 {
        let offset = if index < self.len() {
            self.prefix(index) + within.min(self.height(index))
        } else {
            self.total()
        };
        self.clamp_offset(offset, viewport)
    }

    fn prefix(&self, mut count: usize) -> f64 {
        let mut sum = 0.0;
        while count > 0 {
            sum += self.sums[count];
            count -= low_bit(count);
        }
        sum
    }

    // Largest prefix whose height is <= target (or < target for the trailing edge).
    fn partition(&self, target: f64, inclusive: bool) -> usize {
        let mut index = 0;
        let mut sum = 0.0;
        let mut step = self.len().next_power_of_two();
        while step > 0 {
            let next = index + step;
            if next <= self.len() {
                let candidate = sum + self.sums[next];
                if candidate < target || (inclusive && candidate == target) {
                    index = next;
                    sum = candidate;
                }
            }
            step >>= 1;
        }
        index
    }

    fn rebuild(&mut self) {
        self.sums.resize(self.len() + 1, 0.0);
        for node in 1..=self.len() {
            self.rebuild_node(node);
        }
    }

    fn rebuild_node(&mut self, node: usize) {
        // Sum nonnegative children instead of accumulating deltas: repeated measuring
        // and collapsing rows must not leave negative or phantom extents from cancellation.
        let lower = node - low_bit(node);
        let mut cursor = node - 1;
        let mut sum = self.height(node - 1);
        while cursor > lower {
            sum += self.sums[cursor];
            cursor -= low_bit(cursor);
        }
        self.sums[node] = sum;
    }
}

fn low_bit(value: usize) -> usize {
    value & value.wrapping_neg()
}

fn validate_count(count: usize) -> Result<(), String> {
    if count > MAX_ROWS {
        return Err("UI_LIST: row count exceeds limit".into());
    }
    Ok(())
}

fn validate_estimate(estimate: f64) -> Result<(), String> {
    if !estimate.is_finite() || estimate <= 0.0 || estimate > MAX_HEIGHT {
        return Err("UI_LIST: invalid estimated height".into());
    }
    Ok(())
}

fn validate_viewport(offset: f64, viewport: f64) -> Result<(), String> {
    if !offset.is_finite() || !viewport.is_finite() || viewport < 0.0 {
        return Err("UI_LIST: invalid viewport".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thousand_rows_only_request_the_visible_window_and_buffer() {
        let list = ListMetrics::new(1000, 100.0).unwrap();
        assert_eq!(
            list.window(12_345.0, 600.0, 600.0).unwrap(),
            Window {
                rows: 117..136,
                leading: 11_700.0,
                trailing: 86_400.0,
                total: 100_000.0,
                offset: 12_345.0,
            }
        );
        assert_eq!(list.window(100.0, 100.0, 0.0).unwrap().rows, 1..2);
        assert_eq!(list.window(-50.0, 100.0, 0.0).unwrap().rows, 0..1);
        assert_eq!(list.window(f64::MAX, 100.0, 0.0).unwrap().rows, 999..1000);
        assert!(list.window(100.0, 0.0, 600.0).unwrap().rows.is_empty());
    }

    #[test]
    fn measuring_above_the_viewport_preserves_the_visible_row_and_inset() {
        let mut list = ListMetrics::new(1000, 100.0).unwrap();
        let offset = list
            .measure(&[(0, 200.0), (4, 50.0)], 12_345.0, 600.0)
            .unwrap();
        assert_eq!(offset, 12_395.0);
        assert_eq!(list.position(offset, 600.0), (123, 45.0));
        let offset = list.measure(&[(123, 20.0)], offset, 600.0).unwrap();
        assert_eq!(list.position(offset, 600.0), (124, 0.0));
        assert_eq!(list.invalidate(80.0, offset, 600.0).unwrap(), 124.0 * 80.0);
    }

    #[test]
    fn anchor_alignment_and_count_changes_use_measured_geometry() {
        let mut list = ListMetrics::new(10, 100.0).unwrap();
        list.measure(&[(0, 200.0), (8, 300.0)], 0.0, 200.0).unwrap();
        assert_eq!(
            list.anchor_offset(
                Anchor::Row {
                    index: 8,
                    align: Align::Start
                },
                200.0
            )
            .unwrap(),
            900.0
        );
        assert_eq!(
            list.anchor_offset(
                Anchor::Row {
                    index: 8,
                    align: Align::End
                },
                200.0
            )
            .unwrap(),
            1000.0
        );
        assert_eq!(list.anchor_offset(Anchor::End, 200.0).unwrap(), 1100.0);
        assert_eq!(list.resize(5, 1100.0, 200.0).unwrap(), 400.0);
        list.resize(10, 400.0, 200.0).unwrap();
        assert_eq!(list.total(), 1100.0);
        assert_eq!(list.measured[0], Some(200.0));
        assert_eq!(list.measured[8], None);
        assert_eq!(list.resize(0, 400.0, 200.0).unwrap(), 0.0);
        assert_eq!(list.anchor_offset(Anchor::End, 200.0).unwrap(), 0.0);
        assert!(
            list.anchor_offset(
                Anchor::Row {
                    index: 0,
                    align: Align::Start
                },
                200.0
            )
            .is_err()
        );
        assert_eq!(list.window(0.0, 200.0, 200.0).unwrap().rows, 0..0);
    }

    #[test]
    fn invalid_updates_leave_measurements_and_geometry_unchanged() {
        let mut list = ListMetrics::new(4, 100.0).unwrap();
        let before = list.window(20.0, 80.0, 0.0).unwrap();
        for rows in [
            vec![(0, 50.0), (4, 1.0)],
            vec![(0, 50.0), (1, f64::NAN)],
            vec![(0, 50.0), (0, 60.0)],
            vec![(0, -1.0)],
        ] {
            assert!(list.measure(&rows, 20.0, 80.0).is_err());
            assert_eq!(list.window(20.0, 80.0, 0.0).unwrap(), before);
            assert!(list.measured.iter().all(Option::is_none));
        }
        assert!(list.resize(MAX_ROWS + 1, 20.0, 80.0).is_err());
        assert!(list.invalidate(0.0, 20.0, 80.0).is_err());
        assert!(list.measure(&[(0, 50.0)], f64::NAN, 80.0).is_err());
        assert_eq!(list.window(20.0, 80.0, 0.0).unwrap(), before);
        assert!(ListMetrics::new(MAX_ROWS + 1, 100.0).is_err());
        assert!(ListMetrics::new(1, f64::INFINITY).is_err());
        assert!(list.window(0.0, f64::NAN, 0.0).is_err());
        assert!(list.window(0.0, 100.0, -1.0).is_err());
    }

    #[test]
    fn collapsed_rows_do_not_leave_phantom_extent() {
        let mut list = ListMetrics::new(1000, 0.1).unwrap();
        for index in 0..1000 {
            list.measure(&[(index, 0.0)], 0.0, 10.0).unwrap();
        }
        assert_eq!(list.total(), 0.0);
        assert!(list.window(0.0, 10.0, 10.0).unwrap().rows.is_empty());
        list.measure(&[(500, 5.0)], 0.0, 10.0).unwrap();
        assert_eq!(list.window(0.0, 10.0, 0.0).unwrap().rows, 500..501);
    }

    #[test]
    fn variable_windows_match_a_linear_reference_after_repeated_measurement() {
        let mut list = ListMetrics::new(67, 10.0).unwrap();
        let mut heights = vec![10.0; 67];
        let mut random = 13_u64;
        for _ in 0..2000 {
            random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
            let index = (random >> 32) as usize % heights.len();
            let height = ((random >> 16) % 161) as f64 / 8.0;
            heights[index] = height;
            list.measure(&[(index, height)], 0.0, 20.0).unwrap();
            let total: f64 = heights.iter().sum();
            assert_eq!(list.total(), total);
            let viewport = 1.0 + (random % 80) as f64;
            let overscan = ((random >> 8) % 20) as f64;
            let requested = ((random >> 24) % 2000) as f64;
            let offset = requested.min((total - viewport).max(0.0));
            let start = (offset - overscan).max(0.0);
            let end = (offset + viewport + overscan).min(total);
            let mut y = 0.0;
            let mut visible = Vec::new();
            for (i, &h) in heights.iter().enumerate() {
                if h > 0.0 && y < end && y + h > start {
                    visible.push(i);
                }
                y += h;
            }
            let expected = *visible.first().unwrap()..visible.last().unwrap() + 1;
            let window = list.window(requested, viewport, overscan).unwrap();
            assert_eq!(window.rows, expected);
            assert_eq!(
                window.leading,
                heights[..expected.start].iter().sum::<f64>()
            );
            assert_eq!(window.trailing, heights[expected.end..].iter().sum::<f64>());
        }
    }
}
