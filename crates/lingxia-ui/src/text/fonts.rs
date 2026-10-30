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
    /// Face files the system collection may not expose, registered before the fallbacks.
    files: Vec<std::path::PathBuf>,
    /// Fallback faces in those files by PostScript name, per language tag.
    faces: Vec<(String, String)>,
}

/// Languages whose script needs its own fallback face: a sample of the script, then named
/// families to try after the face the platform picks for the sample. Apple's picked Han face
/// (PingFang UI) registers no face here, and iOS ships no other Chinese face, so Han falls back
/// to Japanese Hiragino last: most Han renders, simplified-only characters do not.
pub const FALLBACK_SAMPLES: [(&str, &str, &[&str]); 4] = [
    (
        "zh-Hans",
        "汉字",
        &["Hiragino Sans GB", "Heiti SC", "Songti SC", "Hiragino Sans"],
    ),
    (
        "zh-Hant",
        "漢字",
        &["Heiti TC", "Songti TC", "Hiragino Sans"],
    ),
    ("ja", "かな漢字", &["Hiragino Sans"]),
    ("ko", "한글", &["Apple SD Gothic Neo"]),
];

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

    /// Fallback faces by file, as `(language, file, PostScript name)`: the platform names the
    /// face its own text system picks for each of [`FALLBACK_SAMPLES`], which the system
    /// collection may hide, and whose family names may differ from the platform's.
    pub fn from_files(faces: Vec<(String, std::path::PathBuf, String)>) -> Self {
        let mut fonts = Self::default();
        for (lang, file, face) in faces {
            if !fonts.files.contains(&file) {
                fonts.files.push(file);
            }
            // Han text shapes as `Hani` whatever its locale; the first Han face keys it too.
            if lang.starts_with("zh-Hans") {
                fonts.faces.push(("und-Hani".into(), face.clone()));
            }
            fonts.faces.push((lang, face));
        }
        fonts
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
        let mut by_postscript: std::collections::HashMap<String, parley::fontique::FamilyId> =
            Default::default();
        for path in &self.files {
            let Some(blob) = std::fs::File::open(path)
                .ok()
                .and_then(|file| map_face_file(&file))
            else {
                log::warn!(target: "lingxia_ui", "UI_FONT: cannot read {}", path.display());
                continue;
            };
            crate::text::outlines::note_file(blob.id(), path);
            for (id, infos) in collection.register_fonts(blob.clone(), None) {
                for info in infos {
                    if let Some(name) = postscript_name(blob.as_ref(), info.index()) {
                        by_postscript.entry(name).or_insert(id);
                    }
                }
            }
        }
        // A platform may name a named instance ("PingFangSC-Regular") of a variable face the
        // file lists under its default ("PingFangSC-Medium"): the same PostScript family.
        let family_of = |name: &str| name.split('-').next().unwrap_or(name).to_owned();
        let mut by_family: Vec<(&String, parley::fontique::FamilyId)> =
            by_postscript.iter().map(|(name, id)| (name, *id)).collect();
        by_family.sort_by_key(|(name, _)| (name.starts_with('.'), (*name).clone()));
        let faces = self.faces.iter().filter_map(|(lang, face)| {
            let id = by_postscript.get(face).copied().or_else(|| {
                let wanted = family_of(face.trim_start_matches('.'));
                by_family
                    .iter()
                    .find(|(name, _)| family_of(name.trim_start_matches('.')) == wanted)
                    .map(|(_, id)| *id)
            });
            if id.is_none() {
                log::warn!(target: "lingxia_ui", "UI_FONT: no face {face} for {lang}");
            }
            Some((lang, id?))
        });
        let named = self.fallbacks.iter().filter_map(|(lang, name)| {
            Some((lang, collection.family_by_name(name).map(|f| f.id())?))
        });
        let chain: Vec<_> = faces.chain(named).map(|(l, id)| (l.clone(), id)).collect();
        for (lang, id) in chain {
            for (script, locale) in keys(&lang) {
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

/// The tag `hvgl` tables are renamed to: the tag just below `hvgl`, so a sorted table directory
/// stays sorted for binary search.
pub(crate) const HIDDEN_HVGL: [u8; 4] = *b"hvgk";

/// Maps a face file for registration. fontique drops every face carrying an `hvgl` table, since
/// it cannot outline them; our renderer outlines those through the platform, so such a file is
/// mapped copy-on-write and the table renamed, which copies only the directory's pages.
fn map_face_file(file: &std::fs::File) -> Option<parley::fontique::Blob<u8>> {
    // SAFETY: system font files are read-only for the life of the process.
    let data = unsafe { memmap2::Mmap::map(file) }.ok()?;
    if hvgl_records(&data).is_empty() {
        return Some(parley::fontique::Blob::new(std::sync::Arc::new(data)));
    }
    drop(data);
    // SAFETY: as above; the copy-on-write mapping is private, the file is never written.
    let mut data = unsafe { memmap2::MmapOptions::new().map_copy(file) }.ok()?;
    hide_hvgl(&mut data);
    Some(parley::fontique::Blob::new(std::sync::Arc::new(data)))
}

/// Renames every `hvgl` table record in a font file or collection to [`HIDDEN_HVGL`].
pub(crate) fn hide_hvgl(data: &mut [u8]) {
    for record in hvgl_records(data) {
        data[record..record + 4].copy_from_slice(&HIDDEN_HVGL);
    }
}

/// Offsets of the `hvgl` table records in each face's table directory.
fn hvgl_records(data: &[u8]) -> Vec<usize> {
    let u16_at = |at: usize| Some(u16::from_be_bytes(data.get(at..at + 2)?.try_into().ok()?));
    let u32_at = |at: usize| Some(u32::from_be_bytes(data.get(at..at + 4)?.try_into().ok()?));
    let faces: Vec<usize> = if data.get(..4) == Some(b"ttcf") {
        let count = u32_at(8).unwrap_or(0) as usize;
        (0..count)
            .filter_map(|i| u32_at(12 + 4 * i).map(|offset| offset as usize))
            .collect()
    } else {
        vec![0]
    };
    let mut records = Vec::new();
    for face in faces {
        let tables = u16_at(face + 4).unwrap_or(0) as usize;
        for i in 0..tables {
            let record = face + 12 + 16 * i;
            if data.get(record..record + 4) == Some(b"hvgl") {
                records.push(record);
            }
        }
    }
    records
}

/// A face's PostScript name, which names one face across platforms and name tables.
fn postscript_name(data: &[u8], index: u32) -> Option<String> {
    let font = FontRef::from_index(data, index).ok()?;
    font.localized_strings(StringId::POSTSCRIPT_NAME)
        .english_or_first()
        .map(|name| name.chars().collect())
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
