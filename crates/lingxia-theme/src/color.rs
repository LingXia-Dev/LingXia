//! sRGB colours, WCAG contrast and OKLCH lightness moves.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Straight-alpha sRGB colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Rgba(pub [u8; 4]);

impl Rgba {
    pub fn parse(text: &str) -> Option<Rgba> {
        let hex = text.strip_prefix('#')?;
        if !matches!(hex.len(), 6 | 8) || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return None;
        }
        let n = u32::from_str_radix(hex, 16).ok()?;
        Some(Rgba(if hex.len() == 8 {
            n.to_be_bytes()
        } else {
            let [_, r, g, b] = n.to_be_bytes();
            [r, g, b, 255]
        }))
    }

    pub fn hex(self) -> String {
        let [r, g, b, a] = self.0;
        if a == 255 {
            format!("#{r:02x}{g:02x}{b:02x}")
        } else {
            format!("#{r:02x}{g:02x}{b:02x}{a:02x}")
        }
    }

    /// WCAG 2 relative luminance, ignoring alpha.
    pub fn luminance(self) -> f64 {
        let [r, g, b, _] = self.0.map(|channel| linear(f64::from(channel) / 255.0));
        0.2126 * r + 0.7152 * g + 0.0722 * b
    }

    pub fn oklch(self) -> Oklch {
        let [r, g, b, _] = self.0.map(|channel| linear(f64::from(channel) / 255.0));
        let l = (0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b).cbrt();
        let m = (0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b).cbrt();
        let s = (0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b).cbrt();
        let lab_l = 0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s;
        let a = 1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s;
        let b = 0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s;
        Oklch {
            l: lab_l,
            c: a.hypot(b),
            h: b.atan2(a).to_degrees().rem_euclid(360.0),
        }
    }
}

/// WCAG contrast ratio of two opaque colours, 1–21.
pub fn contrast(a: Rgba, b: Rgba) -> f64 {
    let (x, y) = (a.luminance(), b.luminance());
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Oklch {
    pub l: f64,
    pub c: f64,
    pub h: f64,
}

impl Oklch {
    /// The nearest in-gamut sRGB colour, reducing chroma at fixed lightness and hue.
    pub fn rgba(self, alpha: u8) -> Rgba {
        let l = self.l.clamp(0.0, 1.0);
        let mut low = 0.0;
        let mut high = self.c.max(0.0);
        if let Some(rgb) = srgb(l, high, self.h) {
            return pack(rgb, alpha);
        }
        for _ in 0..24 {
            let mid = (low + high) / 2.0;
            if srgb(l, mid, self.h).is_some() {
                low = mid;
            } else {
                high = mid;
            }
        }
        pack(srgb(l, low, self.h).unwrap_or([l, l, l]), alpha)
    }
}

fn pack(rgb: [f64; 3], alpha: u8) -> Rgba {
    let [r, g, b] = rgb.map(|channel| (encode(channel) * 255.0).round().clamp(0.0, 255.0) as u8);
    Rgba([r, g, b, alpha])
}

fn srgb(l: f64, c: f64, h: f64) -> Option<[f64; 3]> {
    let (a, b) = (c * h.to_radians().cos(), c * h.to_radians().sin());
    let l_ = (l + 0.3963377774 * a + 0.2158037573 * b).powi(3);
    let m_ = (l - 0.1055613458 * a - 0.0638541728 * b).powi(3);
    let s_ = (l - 0.0894841775 * a - 1.2914855480 * b).powi(3);
    let rgb = [
        4.0767416621 * l_ - 3.3077115913 * m_ + 0.2309699292 * s_,
        -1.2684380046 * l_ + 2.6097574011 * m_ - 0.3413193965 * s_,
        -0.0041960863 * l_ - 0.7034186147 * m_ + 1.7076147010 * s_,
    ];
    rgb.iter()
        .all(|channel| (-1e-4..=1.0001).contains(channel))
        .then(|| rgb.map(|channel| channel.clamp(0.0, 1.0)))
}

fn linear(channel: f64) -> f64 {
    if channel <= 0.04045 {
        channel / 12.92
    } else {
        ((channel + 0.055) / 1.055).powf(2.4)
    }
}

fn encode(channel: f64) -> f64 {
    if channel <= 0.0031308 {
        channel * 12.92
    } else {
        1.055 * channel.powf(1.0 / 2.4) - 0.055
    }
}

/// `foreground` moved in OKLCH lightness, away from `background`, until it reaches `ratio`.
pub fn ensure_contrast(foreground: Rgba, background: Rgba, ratio: f64) -> Rgba {
    if contrast(foreground, background) >= ratio {
        return foreground;
    }
    let mut color = foreground.oklch();
    let darker = background.luminance() > 0.18;
    for _ in 0..200 {
        color.l = (color.l + if darker { -0.005 } else { 0.005 }).clamp(0.0, 1.0);
        let candidate = color.rgba(foreground.0[3]);
        if contrast(candidate, background) >= ratio || color.l == 0.0 || color.l == 1.0 {
            return candidate;
        }
    }
    color.rgba(foreground.0[3])
}

impl Serialize for Rgba {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.hex())
    }
}

impl<'de> Deserialize<'de> for Rgba {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Rgba::parse(&text).ok_or_else(|| {
            serde::de::Error::custom(format!("expected #rrggbb or #rrggbbaa, got {text}"))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contrast_matches_wcag_reference_values() {
        let black = Rgba::parse("#000000").unwrap();
        let white = Rgba::parse("#ffffff").unwrap();
        assert!((contrast(black, white) - 21.0).abs() < 1e-9);
        let gray = Rgba::parse("#767676").unwrap();
        assert!((contrast(gray, white) - 4.54).abs() < 0.01);
    }

    #[test]
    fn oklch_round_trips_and_parses_alpha() {
        for hex in ["#365cdd", "#c93434", "#0f131a", "#ffffff", "#2f9e44"] {
            let color = Rgba::parse(hex).unwrap();
            assert_eq!(color.oklch().rgba(255), color, "{hex}");
        }
        assert_eq!(Rgba::parse("#0f131a80").unwrap().0, [15, 19, 26, 128]);
        assert_eq!(Rgba::parse("#0f131a80").unwrap().hex(), "#0f131a80");
        assert!(Rgba::parse("#12345").is_none() && Rgba::parse("123456").is_none());
    }

    #[test]
    fn ensure_contrast_moves_away_from_the_background() {
        let white = Rgba::parse("#ffffff").unwrap();
        let pale = Rgba::parse("#9db0f0").unwrap();
        let fixed = ensure_contrast(pale, white, 4.5);
        assert!(contrast(fixed, white) >= 4.5);
        assert!(fixed.luminance() < pale.luminance());
        let navy = Rgba::parse("#0f131a").unwrap();
        let dim = Rgba::parse("#2a3550").unwrap();
        let lifted = ensure_contrast(dim, navy, 4.5);
        assert!(contrast(lifted, navy) >= 4.5 && lifted.luminance() > dim.luminance());
    }
}
