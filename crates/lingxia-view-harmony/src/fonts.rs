//! System fonts from the OHOS font configuration. fontique has no OHOS backend (it would
//! take the Linux fontconfig one), so the configuration's directories, fallback chain and
//! generic families go to the core, which resolves them to face files.
use crate::sys;
use lingxia_view::text::fonts::{GenericFamily, SystemFonts};
use std::ffi::CStr;
use std::path::PathBuf;

/// Where the configuration API is missing or fails.
const FALLBACK_DIR: &str = "/system/fonts";
/// The family HarmonyOS draws its UI with, should the configuration name none.
const DEFAULT_FAMILY: &str = "HarmonyOS Sans";

#[derive(Default)]
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
        ..Default::default()
    });
    let mut sans: Vec<String> = config.generics.first().cloned().unwrap_or_default();
    sans.push(DEFAULT_FAMILY.into());
    let mono: Vec<String> = config
        .generics
        .iter()
        .filter(|names| {
            names
                .iter()
                .any(|n| n.to_ascii_lowercase().contains("mono"))
        })
        .flatten()
        .cloned()
        .collect();
    let mut generics: Vec<(GenericFamily, Vec<String>)> = [
        GenericFamily::SansSerif,
        GenericFamily::SystemUi,
        GenericFamily::UiSansSerif,
    ]
    .into_iter()
    .map(|family| (family, sans.clone()))
    .collect();
    if !mono.is_empty() {
        generics.push((GenericFamily::Monospace, mono));
    }
    SystemFonts::from_font_dirs(&config.dirs, &config.chain, &generics)
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
