use serde_json::Value;

/// Host-owned font-size mapping in logical units, independent of display density.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct TextScale(Vec<(f32, f32)>);

impl TextScale {
    pub(crate) fn decode(value: Option<&Value>) -> Result<Self, String> {
        let Some(value) = value else {
            return Ok(Self::default());
        };
        let invalid = || "UI_ENVIRONMENT: invalid text scale mapping".to_owned();
        let rows = value.as_array().ok_or_else(invalid)?;
        if !(2..=512).contains(&rows.len()) {
            return Err(invalid());
        }
        let mut points = Vec::with_capacity(rows.len());
        for row in rows {
            let row = row
                .as_array()
                .filter(|row| row.len() == 2)
                .ok_or_else(invalid)?;
            let x = row[0].as_f64().ok_or_else(invalid)? as f32;
            let y = row[1].as_f64().ok_or_else(invalid)? as f32;
            if !x.is_finite()
                || !y.is_finite()
                || !(0.0..=16384.0).contains(&x)
                || !(0.0..=65536.0).contains(&y)
                || (x > 0.0 && !(0.25..=4.0).contains(&(y / x)))
            {
                return Err(invalid());
            }
            if let Some(&(px, py)) = points.last() {
                if x <= px || y <= py {
                    return Err(invalid());
                }
            } else if x != 0.0 || y != 0.0 {
                return Err(invalid());
            }
            points.push((x, y));
        }
        if points.last().unwrap().0 != 16384.0 {
            return Err(invalid());
        }
        Ok(Self(points))
    }

    pub(crate) fn map(&self, size: f32) -> f32 {
        if self.0.is_empty() || size <= 0.0 {
            return size;
        }
        let upper = self
            .0
            .partition_point(|&(x, _)| x < size)
            .min(self.0.len() - 1);
        let (x1, y1) = self.0[upper];
        let (x0, y0) = self.0[upper.saturating_sub(1)];
        if x1 == x0 {
            return y1;
        }
        y0 + (size - x0) / (x1 - x0) * (y1 - y0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn nonlinear_mapping_interpolates_and_rejects_unbounded_snapshots() {
        let mapping =
            TextScale::decode(Some(&json!([[0, 0], [14, 28], [28, 42], [16384, 24576]]))).unwrap();
        assert_eq!(mapping.map(14.0), 28.0);
        assert_eq!(mapping.map(21.0), 35.0);
        assert_eq!(mapping.map(28.0), 42.0);
        assert_eq!(TextScale::default().map(17.0), 17.0);
        for invalid in [
            json!(null),
            json!([]),
            json!([[1, 1], [16384, 16384]]),
            json!([[0, 0], [14, 0], [16384, 16384]]),
            json!([[0, 0], [14, 100], [16384, 16384]]),
            json!([[0, 0], [14, 28], [13, 29], [16384, 16384]]),
            json!([[0, 0], [14, 28]]),
        ] {
            assert!(TextScale::decode(Some(&invalid)).is_err());
        }
    }
}
