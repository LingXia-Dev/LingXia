//! Calendar customs of a display language (CLDR week data and preferred hour cycle), for the
//! `env` facts a host does not report from its own settings.

// Regions whose week starts on Sunday or Saturday; every other region starts on Monday.
const SUNDAY: &[&str] = &[
    "AG", "AS", "BR", "BS", "BT", "BW", "BZ", "CA", "CO", "DM", "DO", "ET", "GT", "GU", "HK", "HN",
    "ID", "IL", "IN", "JM", "JP", "KE", "KH", "KR", "LA", "MH", "MM", "MO", "MT", "MX", "MZ", "NI",
    "NP", "PA", "PE", "PH", "PK", "PR", "PT", "PY", "SA", "SG", "SV", "TH", "TT", "TW", "UM", "US",
    "VE", "VI", "WS", "YE", "ZA", "ZW",
];
const SATURDAY: &[&str] = &[
    "AE", "AF", "BH", "DJ", "DZ", "EG", "IQ", "IR", "JO", "KW", "LY", "OM", "QA", "SD", "SY",
];
// Regions whose clock customarily runs 1–12 with a day period.
const TWELVE_HOUR: &[&str] = &[
    "AE", "AU", "BD", "BH", "CA", "CO", "DO", "EG", "GT", "HK", "HN", "IN", "IQ", "JO", "KR", "KW",
    "MX", "MY", "NI", "NZ", "OM", "PA", "PH", "PK", "PR", "QA", "SA", "SV", "TW", "US", "VE",
];

/// The region of a BCP 47 tag, or the region a bare language usually means.
fn region(locale: &str) -> String {
    let mut parts = locale.split(['-', '_']);
    let language = parts.next().unwrap_or_default().to_ascii_lowercase();
    let mut script = None;
    for part in parts {
        if part.len() == 2 && part.chars().all(|c| c.is_ascii_alphabetic()) {
            return part.to_ascii_uppercase();
        }
        if part.len() == 4 {
            script = Some(part.to_ascii_lowercase());
        }
    }
    match (language.as_str(), script.as_deref()) {
        ("zh", Some("hant")) => "TW",
        ("zh", _) => "CN",
        ("ja", _) => "JP",
        ("ko", _) => "KR",
        ("he" | "iw", _) => "IL",
        ("ar", _) => "EG",
        ("hi", _) => "IN",
        ("pt", _) => "BR",
        ("en", _) | ("", _) => "US",
        // Most other languages' home regions start on Monday with a 24-hour clock.
        _ => "",
    }
    .into()
}

/// `h12` or `h23` for `locale`.
pub(crate) fn hour_cycle(locale: &str) -> &'static str {
    if TWELVE_HOUR.contains(&region(locale).as_str()) {
        "h12"
    } else {
        "h23"
    }
}

/// The first day of the week for `locale`, 1 = Sunday to 7 = Saturday.
pub(crate) fn first_weekday(locale: &str) -> u8 {
    let region = region(locale);
    if SUNDAY.contains(&region.as_str()) {
        1
    } else if SATURDAY.contains(&region.as_str()) {
        7
    } else {
        2
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn languages_follow_their_regions_week_and_clock() {
        for (locale, cycle, first) in [
            ("en-US", "h12", 1),
            ("en", "h12", 1),
            ("en-GB", "h23", 2),
            ("zh-Hans-CN", "h23", 2),
            ("zh-Hant", "h12", 1),
            ("ja", "h23", 1),
            ("de-DE", "h23", 2),
            ("ar-SA", "h12", 1),
            ("ar-EG", "h12", 7),
            ("fr", "h23", 2),
        ] {
            assert_eq!(super::hour_cycle(locale), cycle, "{locale}");
            assert_eq!(super::first_weekday(locale), first, "{locale}");
        }
    }
}
