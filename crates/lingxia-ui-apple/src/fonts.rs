//! The faces CoreText picks for core's fallback samples, by file: Apple keeps its CJK
//! faces (PingFang) out of the collection fontique scans.
use lingxia_ui::text::fonts::{FALLBACK_SAMPLES, SystemFonts};
use objc2_core_foundation::{CFRange, CFString, CFURL, CFURLPathStyle};
use objc2_core_text::{CTFont, kCTFontURLAttribute};

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
            log::debug!(target: "lingxia_ui", "fallback {lang}: {face} at {}", path.display());
            faces.push((lang.to_string(), path, face));
        }
    }
    SystemFonts::from_files(faces)
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
