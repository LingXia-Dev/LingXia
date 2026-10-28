use super::*;
use parley::fontique::{Blob, CollectionOptions};
use parley::{GenericFamily as Generic, LayoutContext, StyleProperty};

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
    let fonts = SystemFonts::parse(XML, |file, index| {
        (file != "Missing.ttf").then(|| format!("{file}#{index}"))
    });
    let pairs: Vec<(&str, &str)> = fonts
        .fallbacks
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
    assert_eq!(fonts.monospace.as_deref(), Some("Mono.ttf#0"));
}

#[test]
fn han_is_keyed_by_locale() {
    let hani = Script::from_bytes(*b"Hani");
    let defaults = |lang: &str| {
        keys(lang)
            .into_iter()
            .map(|(s, l)| {
                let key = FallbackKey::new(s, l.as_ref());
                (
                    key.script(),
                    key.is_default(),
                    key.locale_str().map(str::to_owned),
                )
            })
            .collect::<Vec<_>>()
    };
    assert!(defaults("zh-Hans").contains(&(hani, true, Some("zh-CN".into()))));
    assert!(defaults("zh-Hant").contains(&(hani, false, Some("zh-TW".into()))));
    assert!(defaults("ja").contains(&(hani, false, Some("ja".into()))));
    assert!(defaults("ja").contains(&(Script::from_bytes(*b"Kana"), true, None)));
    assert_eq!(
        defaults("und-Zsye"),
        [(Script::from_bytes(*b"Zsye"), true, None)]
    );
}

/// Shapes Latin from the generic family and Han only through the installed fallback.
#[test]
fn mixed_latin_and_han_have_no_notdef() {
    let latin = Path::new("/System/Library/Fonts/Supplemental/Arial.ttf");
    let han_dir = Path::new("/System/Library/Fonts");
    if !latin.exists() || !han_dir.join("Hiragino Sans GB.ttc").exists() {
        eprintln!("skipped: host fonts unavailable");
        return;
    }
    let shape = |install: bool| {
        let mut fonts = FontContext {
            collection: Collection::new(CollectionOptions {
                shared: false,
                system_fonts: false,
            }),
            source_cache: Default::default(),
        };
        let data = std::fs::read(latin).unwrap();
        let (family, _) = fonts
            .collection
            .register_fonts(Blob::new(std::sync::Arc::new(data)), None)
            .remove(0);
        fonts
            .collection
            .set_generic_families(GenericFamily::SansSerif, std::iter::once(family));
        // Android's system scan registers every face; only the fallback mapping is under test.
        let han = std::fs::read(han_dir.join("Hiragino Sans GB.ttc")).unwrap();
        fonts
            .collection
            .register_fonts(Blob::new(std::sync::Arc::new(han)), None);
        if install {
            SystemFonts::from_android_xml(
                r#"<familyset><family lang="zh-Hans"><font index="0">Hiragino Sans GB.ttc</font></family></familyset>"#,
                han_dir,
            )
            .apply(&mut fonts.collection);
        }
        let mut layouts = LayoutContext::<()>::new();
        let text = "Hello 你好";
        let mut builder = layouts.ranged_builder(&mut fonts, text, 1.0, true);
        builder.push_default(Generic::SansSerif);
        builder.push_default(StyleProperty::Locale(locale("zh-Hans-CN")));
        let mut layout = builder.build(text);
        layout.break_all_lines(None);
        let mut glyphs = Vec::new();
        for line in layout.lines() {
            for item in line.items() {
                if let parley::PositionedLayoutItem::GlyphRun(run) = item {
                    glyphs.extend(run.glyphs().map(|g| g.id));
                }
            }
        }
        glyphs
    };
    assert!(
        shape(false).contains(&0),
        "the fixture must need the fallback"
    );
    let glyphs = shape(true);
    assert_eq!(glyphs.len(), 8);
    assert!(!glyphs.contains(&0), "{glyphs:?}");
}

/// A fallback face given by file is registered and keys its language, so Han text has no tofu
/// even where the system collection does not expose the face.
#[test]
fn a_fallback_face_by_file_shapes_han() {
    let han = Path::new("/System/Library/Fonts/Hiragino Sans GB.ttc");
    if !han.exists() {
        eprintln!("skipped: host fonts unavailable");
        return;
    }
    let mut fonts = FontContext {
        collection: Collection::new(CollectionOptions {
            shared: false,
            system_fonts: false,
        }),
        source_cache: Default::default(),
    };
    SystemFonts::from_files(vec![(
        "zh-Hans".into(),
        han.to_path_buf(),
        "HiraginoSansGB-W3".into(),
    )])
    .apply(&mut fonts.collection);
    let mut layouts = LayoutContext::<()>::new();
    let text = "首页";
    let mut builder = layouts.ranged_builder(&mut fonts, text, 1.0, true);
    builder.push_default(Generic::SansSerif);
    builder.push_default(StyleProperty::Locale(locale("zh-Hans-CN")));
    let mut layout = builder.build(text);
    layout.break_all_lines(None);
    let mut glyphs = 0;
    for line in layout.lines() {
        for item in line.items() {
            if let parley::PositionedLayoutItem::GlyphRun(run) = item {
                for glyph in run.glyphs() {
                    assert_ne!(glyph.id, 0, "no notdef glyphs");
                    glyphs += 1;
                }
            }
        }
    }
    assert_eq!(glyphs, 2);
}

/// A face whose outlines live only in `hvgl` registers by file, shapes Han from its own tables
/// and draws through the platform's outlines.
#[test]
fn an_hvgl_face_shapes_and_draws_through_platform_outlines() {
    use crate::text::outlines::{self, GlyphOutlines};
    use vello_cpu::kurbo::{BezPath, Rect, Shape};
    let path = Path::new(
        "/System/Library/PrivateFrameworks/FontServices.framework/Versions/A/Resources/Reserved/PingFangUI.ttc",
    );
    let Ok(data) = std::fs::read(path) else {
        eprintln!("skipped: no PingFang UI on this host");
        return;
    };
    assert!(postscript_name(&data, 0).is_some());
    // CoreText names an instance of the variable face, not the face the file lists.
    let postscript = "PingFangSC-Regular".to_owned();
    struct Square;
    impl GlyphOutlines for Square {
        fn outline(
            &self,
            _: &outlines::FontData,
            file: Option<&Path>,
            _: u16,
            _: &[i16],
        ) -> Option<BezPath> {
            file?;
            Some(Rect::new(0.0, 0.0, 1000.0, 1000.0).to_path(0.1))
        }
    }
    outlines::set_glyph_outlines(Box::new(Square));
    let mut fonts = FontContext {
        collection: Collection::new(CollectionOptions {
            shared: false,
            system_fonts: false,
        }),
        source_cache: Default::default(),
    };
    SystemFonts::from_files(vec![("zh-Hans".into(), path.to_path_buf(), postscript)])
        .apply(&mut fonts.collection);
    let mut layouts = LayoutContext::<()>::new();
    let text = "首页轻输语";
    let mut builder = layouts.ranged_builder(&mut fonts, text, 1.0, true);
    builder.push_default(Generic::SansSerif);
    builder.push_default(StyleProperty::Locale(locale("zh-Hans-CN")));
    let mut layout = builder.build(text);
    layout.break_all_lines(None);
    let mut drawn = 0;
    for line in layout.lines() {
        for item in line.items() {
            if let parley::PositionedLayoutItem::GlyphRun(run) = item {
                let font = run.run().font().clone();
                assert!(
                    outlines::external(&font),
                    "PingFang UI outlines through the platform"
                );
                let glyphs: Vec<_> = run
                    .glyphs()
                    .map(|g| vello_cpu::Glyph {
                        id: g.id,
                        x: g.x,
                        y: g.y,
                    })
                    .collect();
                assert!(glyphs.iter().all(|g| g.id != 0), "no notdef glyphs");
                let path = outlines::run_path(&font, 16.0, &[], 0.0, &glyphs).unwrap();
                assert!(path.bounding_box().width() > 0.0);
                drawn += glyphs.len();
            }
        }
    }
    assert_eq!(drawn, 5);
}
