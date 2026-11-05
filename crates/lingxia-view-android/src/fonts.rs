//! System font discovery: Android's `fonts.xml` names each language's fallback face by file.
use lingxia_view::text::fonts::{SystemFonts, family_name};
use std::path::Path;

/// Reads `<root>/etc/fonts.xml` against the faces in `<root>/fonts`.
pub(crate) fn load(root: &Path) -> SystemFonts {
    std::fs::read_to_string(root.join("etc/fonts.xml"))
        .map(|xml| from_xml(&xml, &root.join("fonts")))
        .unwrap_or_default()
}

/// fontique maps only lang families whose faces carry a postScriptName, so
/// vendor CJK families fall through to tofu; this names every face from its file.
fn from_xml(xml: &str, dir: &Path) -> SystemFonts {
    let (fallbacks, monospace) = parse(xml, |file, index| family_name(&dir.join(file), index));
    SystemFonts::from_families(fallbacks, monospace)
}

/// `(language, family)` fallbacks in configuration order, and the monospace family.
fn parse(
    xml: &str,
    mut resolve: impl FnMut(&str, u32) -> Option<String>,
) -> (Vec<(String, String)>, Option<String>) {
    let mut fallbacks = Vec::new();
    let mut monospace = None;
    let Ok(doc) = roxmltree::Document::parse(xml) else {
        return (fallbacks, monospace);
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
                fallbacks.push((lang.to_owned(), face.clone()));
            }
        } else if family.attribute("name") == Some("monospace") {
            monospace = Some(face);
        }
    }
    (fallbacks, monospace)
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = r#"<familyset version="23">
  <family name="sans-serif"><font weight="400">Sans.ttf</font></family>
  <family name="monospace"><font weight="400">Mono.ttf</font></family>
  <family lang="zh-Hans"><font weight="400">Missing.ttf</font></family>
  <family lang="zh-Hans"><font weight="400">SC.ttf</font></family>
  <family lang="zh-Hant,zh-Bopo"><font weight="400">TC.ttf</font></family>
  <family lang="ja">
    <font weight="400" index="1" fallbackFor="serif">Serif.ttc</font>
    <font weight="400" index="0" postScriptName="NotoSansCJKjp-Regular">CJK.ttc</font>
  </family>
  <family lang="und-Zsye"><font weight="400">Emoji.ttf</font></family>
</familyset>"#;

    #[test]
    fn lang_families_resolve_from_file_names() {
        let (fallbacks, monospace) = parse(XML, |file, index| {
            (file != "Missing.ttf").then(|| format!("{file}#{index}"))
        });
        let pairs: Vec<(&str, &str)> = fallbacks
            .iter()
            .map(|(l, f)| (l.as_str(), f.as_str()))
            .collect();
        assert_eq!(
            pairs,
            [
                ("zh-Hans", "SC.ttf#0"),
                ("zh-Hant", "TC.ttf#0"),
                ("zh-Bopo", "TC.ttf#0"),
                ("ja", "CJK.ttc#0"),
                ("und-Zsye", "Emoji.ttf#0"),
            ]
        );
        assert_eq!(monospace.as_deref(), Some("Mono.ttf#0"));
    }
}
