//! The faces CoreText picks for the fallback samples, by file: Apple keeps its CJK
//! faces (PingFang) out of the collection fontique scans. The system UI face is named the same
//! way: fontique maps `sans-serif` to Helvetica, which matches no native text and has no Medium,
//! and `monospace` and `serif` to Courier and Times; the system's own designs replace all three.
use lingxia_view::text::fonts::{GenericFamily, SystemFonts};
use objc2_core_foundation::{
    CFDictionary, CFRange, CFRetained, CFString, CFType, CFURL, CFURLPathStyle,
};
use objc2_core_graphics::CGFont;
use objc2_core_text::{CTFont, CTFontUIFontType, kCTFontTraitsAttribute, kCTFontURLAttribute};

/// Languages whose script needs its own fallback face: a sample of the script, then named
/// families to try after the face the platform picks for the sample. Apple's picked Han face
/// (PingFang UI) registers no face here, and iOS ships no other Chinese face, so Han falls back
/// to Japanese Hiragino last: most Han renders, simplified-only characters do not.
const FALLBACK_SAMPLES: [(&str, &str, &[&str]); 4] = [
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

pub(crate) fn system_fonts() -> SystemFonts {
    // SAFETY: CoreText calls with valid strings; a null matrix is the identity.
    let base =
        unsafe { CTFont::with_name(&CFString::from_str("Helvetica"), 12.0, std::ptr::null()) };
    let mut faces = Vec::new();
    for (lang, sample, families) in FALLBACK_SAMPLES {
        let text = CFString::from_str(sample);
        let range = CFRange {
            location: 0,
            length: text.length(),
        };
        let language = CFString::from_str(lang);
        let picked = unsafe { base.for_string_with_language(&text, range, Some(&language)) };
        let named = families.iter().filter_map(|family| {
            let font =
                unsafe { CTFont::with_name(&CFString::from_str(family), 12.0, std::ptr::null()) };
            // CoreText substitutes a missing family; only the asked-for one counts.
            (unsafe { font.family_name() }.to_string() == *family).then_some(font)
        });
        for font in std::iter::once(picked).chain(named) {
            let Some(path) = file(&font) else { continue };
            let face = unsafe { font.post_script_name() }.to_string();
            log::debug!(target: "lingxia_view", "fallback {lang}: {face} at {}", path.display());
            faces.push((lang.to_string(), path, face));
        }
    }
    let mut fonts = SystemFonts::from_files(faces);
    // SAFETY: CoreText call with no language; the font is released when dropped.
    let Some(ui) =
        (unsafe { CTFont::new_ui_font_for_language(CTFontUIFontType::System, 12.0, None) })
    else {
        log::warn!(target: "lingxia_view", "UI_FONT: no system UI font");
        return fonts;
    };
    let dir = file(&ui).and_then(|path| Some(path.parent()?.to_path_buf()));
    // Variable faces: weights select their `wght` axis, not synthetic bold.
    let designs = [
        (
            Some(ui.clone()),
            "system ui",
            &[
                GenericFamily::SansSerif,
                GenericFamily::SystemUi,
                GenericFamily::UiSansSerif,
            ][..],
        ),
        (
            design(&ui, "NSCTFontUIFontDesignMonospaced"),
            "monospaced",
            &[GenericFamily::Monospace, GenericFamily::UiMonospace][..],
        ),
        (
            design(&ui, "NSCTFontUIFontDesignSerif"),
            "serif",
            &[GenericFamily::Serif, GenericFamily::UiSerif][..],
        ),
    ];
    for (font, name, families) in designs {
        match font
            .as_deref()
            .and_then(|font| located(font, dir.as_deref()))
        {
            Some((path, face)) => {
                log::debug!(target: "lingxia_view", "{name}: {face} at {}", path.display());
                for family in families {
                    fonts = fonts.with_generic(*family, path.clone(), face.clone());
                }
            }
            None => {
                log::warn!(target: "lingxia_view", "UI_FONT: no file for the system {name} font")
            }
        }
    }
    fonts
}

/// The system UI font in another of its designs, as AppKit's and UIKit's `withDesign` build it.
fn design(ui: &CTFont, design: &str) -> Option<CFRetained<CTFont>> {
    let key = CFString::from_static_str("NSCTFontUIFontDesignTrait");
    let value = CFString::from_str(design);
    let traits = CFDictionary::<CFString, CFString>::from_slices(&[&*key], &[&*value]);
    // SAFETY: a CFString key with a dictionary value, as CoreText takes traits.
    let attributes = CFDictionary::<CFString, CFType>::from_slices(
        &[unsafe { kCTFontTraitsAttribute }],
        &[traits.as_ref() as &CFType],
    );
    let descriptor = unsafe {
        ui.font_descriptor()
            .copy_with_attributes(attributes.as_opaque())
    };
    let font = unsafe { CTFont::with_font_descriptor(&descriptor, 12.0, std::ptr::null()) };
    // An OS without the design hands back the UI face itself.
    (postscript(&font) != postscript(ui)).then_some(font)
}

/// The face's file and the PostScript name its name table gives. A design CoreText serves
/// without a URL (SF Mono) is found by its name, as the file or one of its instances, beside the
/// system UI face.
fn located(font: &CTFont, dir: Option<&std::path::Path>) -> Option<(std::path::PathBuf, String)> {
    let face = postscript(font)?;
    if let Some(path) = file(font) {
        return Some((path, face));
    }
    std::fs::read_dir(dir?)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        // Files are named for their face (SFNSMono.ttf holds .SFNSMono-Regular); read only those.
        .filter(|path| {
            let compact = |name: &str| name.replace(['.', '-', ' '], "").to_lowercase();
            path.file_stem()
                .and_then(|stem| stem.to_str())
                .is_some_and(|stem| compact(&face).starts_with(&compact(stem)))
        })
        .find_map(|path| {
            let data = std::fs::read(&path).ok()?;
            let font = skrifa::FontRef::from_index(&data, 0).ok()?;
            use skrifa::MetadataProvider;
            let name = |id| -> Option<String> {
                Some(
                    font.localized_strings(id)
                        .english_or_first()?
                        .chars()
                        .collect(),
                )
            };
            let own = name(skrifa::string::StringId::POSTSCRIPT_NAME)?;
            // A variable file may name another instance first (.SFNSMono-Light); its weights
            // come from the axis, so the file's own name registers it.
            let holds = own == face
                || font.named_instances().iter().any(|instance| {
                    instance.postscript_name_id().and_then(name).as_deref() == Some(face.as_str())
                });
            holds.then_some((path, own))
        })
}

/// The face's own PostScript name: CoreText names the UI designs by alias, and the graphics
/// font appends its variation instance (`.SFNS-Regular_wdth_opsz…`).
fn postscript(font: &CTFont) -> Option<String> {
    let graphics = unsafe { font.graphics_font(std::ptr::null_mut()) };
    let name = CGFont::post_script_name(Some(&graphics))?.to_string();
    Some(name.split('_').next().unwrap_or(&name).to_owned())
}

fn file(font: &CTFont) -> Option<std::path::PathBuf> {
    let url = unsafe { font.attribute(kCTFontURLAttribute) }?
        .downcast::<CFURL>()
        .ok()?;
    let path = url.file_system_path(CFURLPathStyle::CFURLPOSIXPathStyle)?;
    Some(std::path::PathBuf::from(path.to_string()))
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_generics_are_the_system_designs() {
        let fonts = format!("{:?}", super::system_fonts());
        for family in [
            "SansSerif",
            "SystemUi",
            "UiSansSerif",
            "Monospace",
            "UiMonospace",
            "Serif",
            "UiSerif",
        ] {
            assert!(fonts.contains(family), "{family} missing: {fonts}");
        }
        for face in [".SFNS-", ".SFNSMono-", ".NewYork-"] {
            assert!(fonts.contains(face), "{face} missing: {fonts}");
        }
    }

    #[test]
    fn every_sample_resolves_to_a_readable_face() {
        let fonts = format!("{:?}", super::system_fonts());
        for lang in ["zh-Hans", "zh-Hant", "ja", "ko"] {
            assert!(
                fonts.contains(&format!("\"{lang}\"")),
                "{lang} missing: {fonts}"
            );
        }
    }
}
