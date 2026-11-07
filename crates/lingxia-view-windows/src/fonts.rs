//! Fallback families: fontique enumerates the DirectWrite collection, but picks no CJK face of
//! its own, so these name the ones Windows UI text uses per script.
use lingxia_view::text::fonts::SystemFonts;

const FALLBACKS: [(&str, &str); 6] = [
    ("und-Hani", "Microsoft YaHei UI"),
    ("zh-Hans", "Microsoft YaHei UI"),
    ("zh-Hant", "Microsoft JhengHei UI"),
    ("ja", "Yu Gothic UI"),
    ("ko", "Malgun Gothic"),
    ("und-Thai", "Leelawadee UI"),
];

pub(crate) fn system_fonts() -> SystemFonts {
    SystemFonts::from_families(
        FALLBACKS
            .iter()
            .map(|(lang, family)| ((*lang).to_owned(), (*family).to_owned()))
            .collect(),
        Some("Consolas".to_owned()),
    )
}
