use super::*;
use parley::fontique::{Blob, CollectionOptions};
use parley::{GenericFamily as Generic, LayoutContext, StyleProperty};

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
            let family = family_name(&han_dir.join("Hiragino Sans GB.ttc"), 0).unwrap();
            SystemFonts::from_families(vec![("zh-Hans".into(), family)], None)
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

/// Every face of a collection gets its `hvgl` record renamed and nothing else, and the renamed
/// directory stays sorted.
#[test]
fn hide_hvgl_renames_the_record_in_every_face() {
    fn face(tags: &[&[u8; 4]]) -> Vec<u8> {
        let mut face = vec![0, 1, 0, 0];
        face.extend((tags.len() as u16).to_be_bytes());
        face.extend([0; 6]);
        for tag in tags {
            face.extend(*tag);
            face.extend([0; 12]);
        }
        face
    }
    let faces = [
        face(&[b"cmap", b"head", b"hvgl", b"name"]),
        face(&[b"glyf", b"head"]),
    ];
    let mut data = b"ttcf".to_vec();
    data.extend([0, 1, 0, 0]);
    data.extend((faces.len() as u32).to_be_bytes());
    let mut offset = 12 + 4 * faces.len();
    for face in &faces {
        data.extend((offset as u32).to_be_bytes());
        offset += face.len();
    }
    for face in &faces {
        data.extend(face);
    }
    let tags = |data: &[u8], at: usize, count: usize| -> Vec<[u8; 4]> {
        (0..count)
            .map(|i| data[at + 12 + 16 * i..][..4].try_into().unwrap())
            .collect()
    };
    let before = data.clone();
    super::hide_hvgl(&mut data);
    let first = 12 + 4 * faces.len();
    let renamed = tags(&data, first, 4);
    assert_eq!(renamed, [*b"cmap", *b"head", super::HIDDEN_HVGL, *b"name"]);
    assert!(renamed.is_sorted());
    assert_eq!(tags(&data, first + faces[0].len(), 2), [*b"glyf", *b"head"]);
    let changed = before.iter().zip(&data).filter(|(a, b)| a != b).count();
    assert_eq!(changed, 1, "only the tag's last byte differs");
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

/// A generic family named by file is the default face where no system collection has one.
#[test]
fn a_generic_family_by_file_is_the_default_face() {
    let path = Path::new("/System/Library/Fonts/Supplemental/Arial.ttf");
    let Ok(data) = std::fs::read(path) else {
        eprintln!("skipped: host fonts unavailable");
        return;
    };
    let postscript = postscript_name(&data, 0).expect("Arial has a PostScript name");
    let mut fonts = FontContext {
        collection: Collection::new(CollectionOptions {
            shared: false,
            system_fonts: false,
        }),
        source_cache: Default::default(),
    };
    assert!(
        fonts
            .collection
            .generic_families(GenericFamily::SansSerif)
            .next()
            .is_none()
    );
    SystemFonts::default()
        .with_generic(
            GenericFamily::SansSerif,
            path.to_path_buf(),
            postscript.clone(),
        )
        .with_generic(
            GenericFamily::SansSerif,
            path.to_path_buf(),
            "Missing-Regular".into(),
        )
        .apply(&mut fonts.collection);
    let ids: Vec<_> = fonts
        .collection
        .generic_families(GenericFamily::SansSerif)
        .collect();
    assert_eq!(ids.len(), 1, "the missing face is skipped");
    let mut layouts = LayoutContext::<()>::new();
    let text = "Home";
    let mut builder = layouts.ranged_builder(&mut fonts, text, 1.0, true);
    builder.push_default(Generic::SansSerif);
    let mut layout = builder.build(text);
    layout.break_all_lines(None);
    let glyphs: Vec<u32> = layout
        .lines()
        .flat_map(|line| line.items().collect::<Vec<_>>())
        .filter_map(|item| match item {
            parley::PositionedLayoutItem::GlyphRun(run) => {
                Some(run.glyphs().map(|g| g.id).collect::<Vec<_>>())
            }
            _ => None,
        })
        .flatten()
        .collect();
    assert_eq!(glyphs.len(), 4);
    assert!(glyphs.iter().all(|id| *id != 0), "no notdef glyphs");
}

/// Families named by a configuration resolve to the faces in a font directory, whatever the
/// configuration's spelling.
#[test]
fn configured_families_resolve_from_font_dirs() {
    let arial = Path::new("/System/Library/Fonts/Supplemental/Arial.ttf");
    if !arial.exists() {
        eprintln!("skipped: host fonts unavailable");
        return;
    }
    let dir = std::env::temp_dir().join(format!("lingxia-font-dirs-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::copy(arial, dir.join("Arial.ttf")).unwrap();
    let fonts = SystemFonts::from_font_dirs(
        std::slice::from_ref(&dir),
        &[
            ("en".into(), "ARIAL".into()),
            ("ja".into(), "Missing Sans".into()),
        ],
        &[(
            GenericFamily::SansSerif,
            vec!["Missing".into(), "ari-al".into()],
        )],
    );
    assert_eq!(fonts.faces, [("en".to_owned(), "ArialMT".to_owned())]);
    assert_eq!(
        fonts.generics,
        [(GenericFamily::SansSerif, "ArialMT".to_owned())]
    );
    assert_eq!(fonts.files, [dir.join("Arial.ttf")]);
    let _ = std::fs::remove_dir_all(&dir);
}
