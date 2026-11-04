//! System font fallbacks from the OHOS font configuration. fontique has no OHOS backend
//! (it would take the Linux fontconfig one), so every face is registered from its file:
//! the fallback groups name families per language, and the font directories hold them.
use crate::sys;
use lingxia_view::text::fonts::{GenericFamily, SystemFonts};
use skrifa::{FontRef, MetadataProvider, string::StringId};
use std::collections::HashMap;
use std::ffi::CStr;
use std::path::{Path, PathBuf};

/// Where the configuration API is missing or fails: the families HarmonyOS ships.
const FALLBACK_DIR: &str = "/system/fonts";

/// The family HarmonyOS draws its UI with, should the configuration name none.
const DEFAULT_FAMILY: &str = "HarmonyOS Sans";

/// Configuration and file family names differ in spacing and case ("HarmonyOS-Sans").
fn key(family: &str) -> String {
    family
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

struct Configuration {
    dirs: Vec<PathBuf>,
    /// `(language, family)` fallbacks in configuration order.
    chain: Vec<(String, String)>,
    /// Generic family sets: the family and its aliases, the system default first.
    generics: Vec<Vec<String>>,
}

pub(crate) fn system_fonts() -> SystemFonts {
    let config = configuration().unwrap_or_else(|| Configuration {
        dirs: vec![FALLBACK_DIR.into()],
        chain: Vec::new(),
        generics: Vec::new(),
    });
    let faces = index(&config.dirs);
    let find = |family: &str| faces.get(&key(family)).cloned();
    let mut picked = Vec::new();
    for (lang, family) in config.chain {
        match find(&family) {
            Some((file, postscript)) => picked.push((lang, file, postscript)),
            None => log::debug!(target: "lingxia_view", "UI_FONT: no file for {family} ({lang})"),
        }
    }
    let mut fonts = SystemFonts::from_files(picked);
    let resolve = |names: &[String]| names.iter().find_map(|name| find(name));
    let sans = config
        .generics
        .iter()
        .find_map(|names| resolve(names))
        .or_else(|| find(DEFAULT_FAMILY));
    if let Some((file, postscript)) = sans {
        for family in [
            GenericFamily::SansSerif,
            GenericFamily::SystemUi,
            GenericFamily::UiSansSerif,
        ] {
            fonts = fonts.with_generic(family, file.clone(), postscript.clone());
        }
    } else {
        log::warn!(target: "lingxia_view", "UI_FONT: no default face");
    }
    let mono = config
        .generics
        .iter()
        .filter(|names| names.iter().any(|name| key(name).contains("mono")))
        .find_map(|names| resolve(names));
    if let Some((file, postscript)) = mono {
        fonts = fonts.with_generic(GenericFamily::Monospace, file, postscript);
    }
    fonts
}

fn configuration() -> Option<Configuration> {
    let mut error = 0;
    // SAFETY: the returned tree is read here and released below.
    let info = unsafe { sys::OH_Drawing_GetSystemFontConfigInfo(&mut error) };
    if info.is_null() || error != 0 {
        return None;
    }
    let text = |p: *mut std::ffi::c_char| {
        (!p.is_null()).then(|| unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned())
    };
    // SAFETY: sizes and pointers come from the same configuration record.
    let config = unsafe {
        let info = &*info;
        let mut generics = Vec::new();
        for g in 0..info.font_generic_info_size {
            let generic = &*info.font_generic_info_set.add(g);
            let mut names: Vec<String> = text(generic.family_name).into_iter().collect();
            for a in 0..generic.alias_info_size {
                names.extend(text((*generic.alias_info_set.add(a)).family_name));
            }
            generics.push(names);
        }
        let dirs = (0..info.font_dir_size)
            .filter_map(|i| text(*info.font_dir_set.add(i)).map(PathBuf::from))
            .collect();
        let mut chain = Vec::new();
        for g in 0..info.fallback_group_size {
            let group = &*info.fallback_group_set.add(g);
            // Named groups serve one generic family; the unnamed one is the default chain.
            if text(group.group_name).is_some_and(|name| !name.is_empty()) {
                continue;
            }
            for f in 0..group.fallback_info_size {
                let fallback = &*group.fallback_info_set.add(f);
                if let (Some(lang), Some(family)) =
                    (text(fallback.language), text(fallback.family_name))
                {
                    chain.push((lang, family));
                }
            }
        }
        Configuration {
            dirs,
            chain,
            generics,
        }
    };
    unsafe { sys::OH_Drawing_DestroySystemFontConfigInfo(info) };
    Some(config)
}

/// Family name to its regular face's file and PostScript name.
fn index(dirs: &[PathBuf]) -> HashMap<String, (PathBuf, String)> {
    let mut faces = HashMap::new();
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for path in entries.flatten().map(|e| e.path()) {
            if !matches!(
                path.extension().and_then(|e| e.to_str()),
                Some("ttf" | "otf" | "ttc")
            ) {
                continue;
            }
            for (family, postscript) in names(&path) {
                faces
                    .entry(key(&family))
                    .or_insert_with(|| (path.clone(), postscript));
            }
        }
    }
    faces
}

/// `(family, PostScript name)` of every face in a file.
fn names(path: &Path) -> Vec<(String, String)> {
    let Some(data) = std::fs::File::open(path)
        .ok()
        // SAFETY: system font files are read-only for the life of the process.
        .and_then(|file| unsafe { memmap2::Mmap::map(&file) }.ok())
    else {
        return Vec::new();
    };
    let count = skrifa::raw::FileRef::new(&data).map_or(1, |file| file.fonts().count());
    (0..count as u32)
        .filter_map(|index| {
            let font = FontRef::from_index(&data, index).ok()?;
            let read = |id| {
                font.localized_strings(id)
                    .english_or_first()
                    .map(|s| s.chars().collect::<String>())
            };
            let family =
                read(StringId::TYPOGRAPHIC_FAMILY_NAME).or_else(|| read(StringId::FAMILY_NAME))?;
            Some((family, read(StringId::POSTSCRIPT_NAME)?))
        })
        .collect()
}
