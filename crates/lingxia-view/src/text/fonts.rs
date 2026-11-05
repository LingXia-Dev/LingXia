//! System font fallback chains, loaded and named from the files or directories an adapter finds.
use parley::FontContext;
/// The families [`SystemFonts::with_generic`] names.
pub use parley::fontique::GenericFamily;
use parley::fontique::{Collection, FallbackKey, Language, Script};
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
    /// Faces in those files that stand for a generic family, by PostScript name.
    generics: Vec<(GenericFamily, String)>,
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
    /// Fallback faces by file, as `(language, file, PostScript name)`: the platform names the
    /// face its own text system picks per script, which the system collection may hide, and whose family names may differ from the platform's.
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

    /// Names `postscript` in `file` as `family`'s face, ahead of any the system collection has:
    /// a platform without a system collection has no default face otherwise.
    pub fn with_generic(
        mut self,
        family: GenericFamily,
        file: std::path::PathBuf,
        postscript: String,
    ) -> Self {
        if !self.files.contains(&file) {
            self.files.push(file);
        }
        self.generics.push((family, postscript));
        self
    }

    /// Fallbacks and generic families named by family, resolved to faces in the font files of
    /// `dirs`: for a platform whose configuration names families but has no system collection.
    /// `generics` lists candidate families per generic family, the first found wins.
    pub fn from_font_dirs(
        dirs: &[std::path::PathBuf],
        fallbacks: &[(String, String)],
        generics: &[(GenericFamily, Vec<String>)],
    ) -> Self {
        let faces = scan_dirs(dirs);
        let find = |family: &str| faces.get(&family_key(family)).cloned();
        let picked = fallbacks
            .iter()
            .filter_map(|(lang, family)| {
                let found = find(family);
                if found.is_none() {
                    log::debug!(target: "lingxia_view", "UI_FONT: no file for {family} ({lang})");
                }
                let (file, postscript) = found?;
                Some((lang.clone(), file, postscript))
            })
            .collect();
        let mut fonts = Self::from_files(picked);
        for (family, candidates) in generics {
            match candidates.iter().find_map(|name| find(name)) {
                Some((file, postscript)) => fonts = fonts.with_generic(*family, file, postscript),
                None => log::warn!(target: "lingxia_view", "UI_FONT: no face for {family:?}"),
            }
        }
        fonts
    }

    /// Fallback families by name, as `(language, family)`, and the monospace family: for a
    /// platform whose configuration names families rather than faces.
    pub fn from_families(fallbacks: Vec<(String, String)>, monospace: Option<String>) -> Self {
        Self {
            fallbacks,
            monospace,
            ..Self::default()
        }
    }

    fn apply(&self, collection: &mut Collection) {
        let mut by_postscript: std::collections::HashMap<String, parley::fontique::FamilyId> =
            Default::default();
        for path in &self.files {
            let Some(blob) = std::fs::File::open(path)
                .ok()
                .and_then(|file| map_face_file(&file))
            else {
                log::warn!(target: "lingxia_view", "UI_FONT: cannot read {}", path.display());
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
                log::warn!(target: "lingxia_view", "UI_FONT: no face {face} for {lang}");
            }
            Some((lang, id?))
        });
        let named = self.fallbacks.iter().filter_map(|(lang, name)| {
            Some((lang, collection.family_by_name(name).map(|f| f.id())?))
        });
        let chain: Vec<_> = faces.chain(named).map(|(l, id)| (l.clone(), id)).collect();
        let mut generics: Vec<(GenericFamily, Vec<parley::fontique::FamilyId>)> = Vec::new();
        for (family, face) in &self.generics {
            let Some(id) = by_postscript.get(face).copied() else {
                log::warn!(target: "lingxia_view", "UI_FONT: no face {face} for {family:?}");
                continue;
            };
            match generics.iter_mut().find(|(f, _)| f == family) {
                Some((_, ids)) => ids.push(id),
                None => generics.push((*family, vec![id])),
            }
        }
        for (family, ids) in generics {
            let rest: Vec<_> = collection
                .generic_families(family)
                .filter(|id| !ids.contains(id))
                .collect();
            collection.set_generic_families(family, ids.into_iter().chain(rest));
        }
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

/// The family fontique registers for the face at `index` in `path`: typographic family, else
/// family.
pub fn family_name(path: &Path, index: u32) -> Option<String> {
    let file = std::fs::File::open(path).ok()?;
    // SAFETY: system font files are read-only for the life of the process.
    let data = unsafe { memmap2::Mmap::map(&file) }.ok()?;
    let font = FontRef::from_index(&data, index).ok()?;
    [StringId::TYPOGRAPHIC_FAMILY_NAME, StringId::FAMILY_NAME]
        .into_iter()
        .find_map(|id| font.localized_strings(id).english_or_first())
        .map(|name| name.chars().collect())
}

/// Configuration and file family names differ in spacing and case ("HarmonyOS-Sans").
fn family_key(family: &str) -> String {
    family
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// Family key to the file and PostScript name of its first face found in `dirs`.
fn scan_dirs(
    dirs: &[std::path::PathBuf],
) -> std::collections::HashMap<String, (std::path::PathBuf, String)> {
    let mut faces = std::collections::HashMap::new();
    for path in dirs
        .iter()
        .filter_map(|dir| std::fs::read_dir(dir).ok())
        .flatten()
        .flatten()
        .map(|e| e.path())
    {
        if !matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("ttf" | "otf" | "ttc")
        ) {
            continue;
        }
        let Ok(file) = std::fs::File::open(&path) else {
            continue;
        };
        // SAFETY: system font files are read-only for the life of the process.
        let Ok(data) = (unsafe { memmap2::Mmap::map(&file) }) else {
            continue;
        };
        let count = skrifa::raw::FileRef::new(&data).map_or(1, |file| file.fonts().count());
        for index in 0..count as u32 {
            let Ok(font) = FontRef::from_index(&data, index) else {
                continue;
            };
            let read = |id| {
                font.localized_strings(id)
                    .english_or_first()
                    .map(|s| s.chars().collect::<String>())
            };
            let family =
                read(StringId::TYPOGRAPHIC_FAMILY_NAME).or_else(|| read(StringId::FAMILY_NAME));
            if let (Some(family), Some(postscript)) = (family, read(StringId::POSTSCRIPT_NAME)) {
                faces
                    .entry(family_key(&family))
                    .or_insert_with(|| (path.clone(), postscript));
            }
        }
    }
    faces
}

/// The parley locale for a BCP 47 display language.
pub(crate) fn locale(display_language: &str) -> Option<Language> {
    Language::parse(display_language).ok()
}

#[cfg(test)]
#[path = "fonts_tests.rs"]
mod tests;
