//! Color transforms on premultiplied sRGB pixels; no image loading or style parsing.

/// Saturation multiplier: 0 is grayscale, 1 is unchanged, 1.8 is 180%.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Saturation(f32);

impl Saturation {
    pub fn new(amount: f32) -> Result<Self, String> {
        if !amount.is_finite() || !(0.0..=16.0).contains(&amount) {
            return Err("UI_FILTER: saturation must be finite and within 0..16".into());
        }
        Ok(Self(amount))
    }

    pub fn amount(self) -> f32 {
        self.0
    }

    /// Mutates a caller-owned intermediate; never mutate a shared image cache entry.
    pub fn apply(self, pixels: &mut vello_cpu::Pixmap) {
        for pixel in pixels.data_as_u8_slice_mut().chunks_exact_mut(4) {
            let luminance = 0.213 * f32::from(pixel[0])
                + 0.715 * f32::from(pixel[1])
                + 0.072 * f32::from(pixel[2]);
            let alpha = f32::from(pixel[3]);
            for channel in &mut pixel[..3] {
                // The saturation matrix has no offset or alpha change. Applying
                // it to premultiplied RGB then clamping to alpha is equivalent
                // to unpremultiply -> matrix -> clamp -> premultiply.
                *channel = (luminance + self.0 * (f32::from(*channel) - luminance))
                    .clamp(0.0, alpha)
                    .round() as u8;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saturation_preserves_alpha_and_clamps_premultiplied_colors() {
        let input = [
            [200, 100, 50, 255],
            [100, 50, 25, 128],
            [0, 0, 0, 0],
            [50, 50, 50, 128],
            [255, 0, 0, 255],
            [0, 255, 0, 255],
            [0, 0, 255, 255],
        ];
        for (amount, expected) in [
            (
                0.0,
                [
                    [118, 118, 118, 255],
                    [59, 59, 59, 128],
                    [0, 0, 0, 0],
                    [50, 50, 50, 128],
                    [54, 54, 54, 255],
                    [182, 182, 182, 255],
                    [18, 18, 18, 255],
                ],
            ),
            (1.0, input),
            (
                1.8,
                [
                    [255, 86, 0, 255],
                    [128, 43, 0, 128],
                    [0, 0, 0, 0],
                    [50, 50, 50, 128],
                    [255, 0, 0, 255],
                    [0, 255, 0, 255],
                    [0, 0, 255, 255],
                ],
            ),
        ] {
            let mut pixels = vello_cpu::Pixmap::new(input.len() as u16, 1);
            pixels
                .data_as_u8_slice_mut()
                .copy_from_slice(input.as_flattened());
            Saturation::new(amount).unwrap().apply(&mut pixels);
            assert_eq!(
                pixels.data_as_u8_slice(),
                expected.as_flattened(),
                "amount={amount}"
            );
        }
    }

    #[test]
    fn saturation_rejects_invalid_factors() {
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -0.1, 16.01] {
            assert!(Saturation::new(value).is_err());
        }
        assert!(Saturation::new(16.0).is_ok());
    }
}
