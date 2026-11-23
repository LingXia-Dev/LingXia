//! Which languages write right to left, and the isolates that keep a formatted argument's
//! direction from reordering the message around it.

/// Primary language subtags written right to left when the tag names no script.
pub const LANGUAGES: &[&str] = &[
    "ar", "ckb", "dv", "fa", "he", "iw", "ks", "ps", "sd", "ug", "ur", "yi",
];

/// Script subtags written right to left.
pub const SCRIPTS: &[&str] = &[
    "arab", "hebr", "thaa", "syrc", "nkoo", "adlm", "rohg", "mand", "samr",
];

/// Code point ranges of right-to-left scripts, inclusive.
pub const RANGES: &[(u32, u32)] = &[
    (0x0590, 0x08FF),
    (0xFB1D, 0xFDFF),
    (0xFE70, 0xFEFF),
    (0x10800, 0x10FFF),
    (0x1E800, 0x1EFFF),
];

/// FIRST STRONG ISOLATE: a text argument takes the direction of its own first letter.
pub const FSI: char = '\u{2068}';
/// LEFT-TO-RIGHT ISOLATE: a number, date or time reads left to right inside any message.
pub const LRI: char = '\u{2066}';
/// POP DIRECTIONAL ISOLATE: ends either.
pub const PDI: char = '\u{2069}';

/// Whether a BCP 47 tag writes right to left, from its script or else its primary language.
pub fn right_to_left(tag: &str) -> bool {
    let mut parts = tag.split(['-', '_']);
    let language = parts.next().unwrap_or("").to_ascii_lowercase();
    match parts.find(|part| part.len() == 4) {
        Some(script) => SCRIPTS.contains(&script.to_ascii_lowercase().as_str()),
        None => LANGUAGES.contains(&language.as_str()),
    }
}

/// Whether `text` has a character of a right-to-left script.
pub fn has_right_to_left(text: &str) -> bool {
    text.chars().any(|c| {
        let c = u32::from(c);
        RANGES.iter().any(|(from, to)| (*from..=*to).contains(&c))
    })
}

/// `text` between `open` and PDI when the message is right to left or `text` has such a letter.
pub(crate) fn isolate(text: String, open: char, rtl: bool) -> String {
    if rtl || has_right_to_left(&text) {
        format!("{open}{text}{PDI}")
    } else {
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_script_subtag_decides_before_the_language() {
        for tag in ["he", "iw-IL", "ar_EG", "ks", "az-Arab", "ur-PK"] {
            assert!(right_to_left(tag), "{tag}");
        }
        for tag in ["en-US", "ks-Deva", "zh-Hans-CN", "", "pa-Guru"] {
            assert!(!right_to_left(tag), "{tag}");
        }
    }

    /// The WebView bridge keeps its own copy for engines without `Intl.Locale#getTextInfo`.
    #[test]
    fn the_bridge_lists_the_same_languages() {
        let bridge = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../packages/lingxia-bridge/src/runtime-env.ts"),
        )
        .unwrap();
        let line = bridge
            .lines()
            .find(|line| line.starts_with("const RTL_LANGUAGES"))
            .expect("RTL_LANGUAGES in runtime-env.ts");
        let mut listed: Vec<&str> = line.split('\'').skip(1).step_by(2).collect();
        listed.sort_unstable();
        let mut ours = LANGUAGES.to_vec();
        ours.sort_unstable();
        assert_eq!(listed, ours);
    }
}
