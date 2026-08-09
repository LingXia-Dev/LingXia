//! Font selection.
//!
//! The framework ships no font, so a configured family may simply not be
//! installed. Selection is therefore a list of candidates resolved against
//! what the machine actually has, and the resolution reports which candidate
//! won — a silent downgrade to a fallback is the failure users cannot
//! diagnose on their own.

use serde::{Deserialize, Serialize};

/// How bold text is rendered. Terminals traditionally brighten the color
/// instead of using a heavier face, and both behaviours are still expected.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BoldStyle {
    /// Use the font's bold face.
    #[default]
    Weight,
    /// Keep the regular face and use the bright ANSI color.
    Bright,
    /// Both.
    Both,
}

impl BoldStyle {
    pub fn uses_bold_face(self) -> bool {
        matches!(self, Self::Weight | Self::Both)
    }

    pub fn uses_bright_color(self) -> bool {
        matches!(self, Self::Bright | Self::Both)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct FontConfig {
    /// Ordered candidates; the first installed one wins. A list rather than
    /// one name because nothing is bundled.
    pub family: Vec<String>,
    pub size: f32,
    /// Multiplier on the font's natural line height.
    pub line_height: f32,
    /// Shape runs with the font's ligatures. Fonts without them are
    /// unaffected.
    pub ligatures: bool,
    pub bold: BoldStyle,
}

impl Default for FontConfig {
    fn default() -> Self {
        Self {
            // One list, not one per platform: the first installed candidate
            // wins, so the same order lands on the coding font when it is
            // there and on each system's own default when it is not.
            family: vec![
                "JetBrains Mono".to_string(),
                "SF Mono".to_string(),
                "Cascadia Code".to_string(),
                "Menlo".to_string(),
                "Consolas".to_string(),
            ],
            size: 13.0,
            line_height: 1.0,
            ligatures: true,
            bold: BoldStyle::Weight,
        }
    }
}

/// One installed family, as reported by the platform.
///
/// `monospace` is not taken from the font's own trait bit alone: many patched
/// coding fonts leave it unset, so hosts confirm it by measuring advances.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledFont {
    pub family: String,
    pub monospace: bool,
    /// The font has `liga`/`calt` features, i.e. `!=` can become one glyph.
    pub ligatures: bool,
    /// The font carries Nerd Font/powerline glyphs (U+E0B0 and friends).
    pub nerd_icons: bool,
}

/// Which candidate the platform actually resolved to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedFont {
    pub family: String,
    /// Candidates that were tried and not found, in order.
    pub missing: Vec<String>,
    /// No candidate matched and the platform default was used.
    pub fell_back: bool,
}

/// Pick the first installed candidate.
///
/// Non-monospace families are skipped: a proportional font does not merely
/// look wrong in a terminal, it breaks the grid every cell depends on.
pub fn resolve(config: &FontConfig, installed: &[InstalledFont]) -> ResolvedFont {
    let mut missing = Vec::new();
    for candidate in &config.family {
        match installed
            .iter()
            .find(|font| font.family.eq_ignore_ascii_case(candidate))
        {
            Some(font) if font.monospace => {
                return ResolvedFont {
                    family: font.family.clone(),
                    missing,
                    fell_back: false,
                };
            }
            // Installed but proportional: as unusable as absent, and worth
            // reporting the same way.
            Some(_) | None => missing.push(candidate.clone()),
        }
    }
    ResolvedFont {
        family: installed
            .iter()
            .find(|font| font.monospace)
            .map(|font| font.family.clone())
            .unwrap_or_default(),
        missing,
        fell_back: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn font(family: &str, monospace: bool) -> InstalledFont {
        InstalledFont {
            family: family.to_string(),
            monospace,
            ligatures: false,
            nerd_icons: false,
        }
    }

    #[test]
    fn first_installed_candidate_wins() {
        let config = FontConfig {
            family: vec![
                "Missing Mono".into(),
                "JetBrains Mono".into(),
                "Menlo".into(),
            ],
            ..FontConfig::default()
        };
        let resolved = resolve(
            &config,
            &[font("Menlo", true), font("JetBrains Mono", true)],
        );
        assert_eq!(resolved.family, "JetBrains Mono");
        assert_eq!(resolved.missing, vec!["Missing Mono".to_string()]);
        assert!(!resolved.fell_back);
    }

    #[test]
    fn proportional_families_are_skipped_and_reported() {
        let config = FontConfig {
            family: vec!["Helvetica".into(), "Menlo".into()],
            ..FontConfig::default()
        };
        let resolved = resolve(&config, &[font("Helvetica", false), font("Menlo", true)]);
        assert_eq!(resolved.family, "Menlo");
        assert_eq!(
            resolved.missing,
            vec!["Helvetica".to_string()],
            "a proportional font is as unusable as a missing one"
        );
    }

    #[test]
    fn matching_ignores_case() {
        let config = FontConfig {
            family: vec!["jetbrains mono".into()],
            ..FontConfig::default()
        };
        let resolved = resolve(&config, &[font("JetBrains Mono", true)]);
        assert_eq!(resolved.family, "JetBrains Mono", "the installed spelling");
        assert!(!resolved.fell_back);
    }

    #[test]
    fn nothing_matching_falls_back_and_says_so() {
        let config = FontConfig {
            family: vec!["Nope".into()],
            ..FontConfig::default()
        };
        let resolved = resolve(&config, &[font("Menlo", true)]);
        assert_eq!(resolved.family, "Menlo");
        assert!(resolved.fell_back, "the host picked, not the config");
        assert_eq!(resolved.missing, vec!["Nope".to_string()]);
    }
}
