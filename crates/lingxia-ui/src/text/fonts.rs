//! System font fallback chains read from the platform's font configuration.
use parley::FontContext;
use parley::fontique::{Collection, FallbackKey, GenericFamily, Language, Script};
use skrifa::{FontRef, MetadataProvider, string::StringId};
use std::path::Path;
use std::sync::OnceLock;

/// Fallback family names per language tag, in configuration order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SystemFonts {
    fallbacks: Vec<(String, String)>,
    monospace: Option<String>,
}

static SOURCE: OnceLock<fn() -> SystemFonts> = OnceLock::new();
static SYSTEM: OnceLock<SystemFonts> = OnceLock::new();

/// Registers the platform loader; it runs once, on the first renderer that needs fonts.
pub fn set_system_fonts(load: fn() -> SystemFonts) {
    let _ = SOURCE.set(load);
}

/// Whether a BCP 47 tag writes right to left, from its script or primary language.
pub(crate) fn right_to_left(tag: &str) -> bool {
    let mut parts = tag.split(['-', '_']);
    let language = parts.next().unwrap_or("").to_ascii_lowercase();
    let script = parts
        .find(|part| part.len() == 4)
        .map(str::to_ascii_lowercase);
    match script.as_deref() {
        Some("arab" | "hebr" | "thaa" | "syrc" | "nkoo" | "adlm" | "rohg" | "mand" | "samr") => {
            true
        }
        Some(_) => false,
        None => matches!(
            language.as_str(),
            "ar" | "fa" | "he" | "iw" | "ur" | "ps" | "sd" | "ug" | "yi" | "dv" | "ckb" | "ks"
        ),
    }
}

pub(crate) fn context() -> FontContext {
    let mut fonts = FontContext::new();
    if let Some(system) = SOURCE.get().map(|load| SYSTEM.get_or_init(load)) {
        system.apply(&mut fonts.collection);
    }
    fonts
}

impl SystemFonts {
    /// Reads `<root>/etc/fonts.xml` against the faces in `<root>/fonts`.
    pub fn load_android(root: &Path) -> Self {
        std::fs::read_to_string(root.join("etc/fonts.xml"))
            .map(|xml| Self::from_android_xml(&xml, &root.join("fonts")))
            .unwrap_or_default()
    }

    /// fontique maps only lang families whose faces carry a postScriptName, so
    /// vendor CJK families fall through to tofu; this names every face from its file.
    pub fn from_android_xml(xml: &str, dir: &Path) -> Self {
        Self::parse(xml, |file, index| family_name(&dir.join(file), index))
    }

    fn parse(xml: &str, mut resolve: impl FnMut(&str, u32) -> Option<String>) -> Self {
        let mut fonts = Self::default();
        let Ok(doc) = roxmltree::Document::parse(xml) else {
            return fonts;
        };
        for family in doc
            .root_element()
            .children()
            .filter(|n| n.has_tag_name("family"))
        {
            let face = family
                .children()
                .filter(|n| n.has_tag_name("font") && n.attribute("fallbackFor").is_none())
                .find_map(|font| {
                    let file = font.text()?.trim();
                    let index = font.attribute("index").and_then(|i| i.parse().ok());
                    resolve(file, index.unwrap_or(0))
                });
            let Some(face) = face else { continue };
            if let Some(langs) = family.attribute("lang") {
                for lang in langs.split(',').map(str::trim).filter(|l| !l.is_empty()) {
                    fonts.fallbacks.push((lang.to_owned(), face.clone()));
                }
            } else if family.attribute("name") == Some("monospace") {
                fonts.monospace = Some(face);
            }
        }
        fonts
    }

    fn apply(&self, collection: &mut Collection) {
        for (lang, name) in &self.fallbacks {
            let Some(id) = collection.family_by_name(name).map(|f| f.id()) else {
                continue;
            };
            for (script, locale) in keys(lang) {
                collection.append_fallbacks(
                    FallbackKey::new(script, locale.as_ref()),
                    std::iter::once(id),
                );
            }
        }
        if collection
            .generic_families(GenericFamily::Monospace)
            .next()
            .is_none()
            && let Some(id) = self
                .monospace
                .as_deref()
                .and_then(|name| collection.family_by_name(name))
                .map(|f| f.id())
        {
            collection.set_generic_families(GenericFamily::Monospace, std::iter::once(id));
        }
    }
}

/// Han text shapes as `Hani` whatever its locale, so locale-scripted tags also key `Hani`.
fn keys(lang: &str) -> Vec<(Script, Option<Language>)> {
    let script = |s: &str| Script::from_bytes(s.as_bytes().try_into().unwrap_or(*b"Zzzz"));
    if let Some(only) = lang.strip_prefix("und-") {
        return vec![(script(only), None)];
    }
    let Ok(locale) = Language::parse(lang) else {
        return Vec::new();
    };
    let mut keys = Vec::new();
    match (locale.language(), locale.script()) {
        ("zh", Some(s @ ("Hans" | "Hant"))) => {
            keys.push((script(s), Some(locale)));
            keys.push((script("Hani"), Some(locale)));
        }
        ("ja", _) => {
            keys.push((script("Hani"), Some(locale)));
            keys.extend(["Hira", "Kana"].map(|s| (script(s), None)));
        }
        ("ko", _) => {
            keys.push((script("Hani"), Some(locale)));
            keys.push((script("Hang"), None));
        }
        (_, Some(s)) => keys.push((script(s), Some(locale))),
        _ => {}
    }
    keys
}

/// The family fontique registers for a face: typographic family, else family.
fn family_name(path: &Path, index: u32) -> Option<String> {
    let file = std::fs::File::open(path).ok()?;
    // SAFETY: system font files are read-only for the life of the process.
    let data = unsafe { memmap2::Mmap::map(&file) }.ok()?;
    let font = FontRef::from_index(&data, index).ok()?;
    [StringId::TYPOGRAPHIC_FAMILY_NAME, StringId::FAMILY_NAME]
        .into_iter()
        .find_map(|id| font.localized_strings(id).english_or_first())
        .map(|name| name.chars().collect())
}

/// The parley locale for a BCP 47 display language.
pub(crate) fn locale(display_language: &str) -> Option<Language> {
    Language::parse(display_language).ok()
}

#[cfg(test)]
#[path = "fonts_tests.rs"]
mod tests;
