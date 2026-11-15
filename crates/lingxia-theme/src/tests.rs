use super::*;

fn hex(text: &str) -> Rgba {
    Rgba::parse(text).unwrap()
}

#[test]
fn the_default_pack_proves_every_pair_in_both_appearances() {
    let pack = default_pack();
    assert_eq!(check(&pack), vec![]);
    let light = resolve(&pack, &Prefs::default()).unwrap();
    assert_eq!(
        light.color(Role::Accent),
        hex("#365cdd"),
        "a passing seed is kept"
    );
    for appearance in [Appearance::Light, Appearance::Dark] {
        let prefs = Prefs {
            appearance,
            high_contrast: true,
            ..Prefs::default()
        };
        let theme = resolve(&pack, &prefs).unwrap();
        for &(foreground, background, ratio) in CONTRAST {
            let actual = contrast(theme.color(foreground), theme.color(background));
            let needed = if involves_selection(foreground, background) {
                f64::from(ratio)
            } else {
                boosted(ratio)
            };
            assert!(
                actual + 1e-9 >= needed,
                "{appearance:?} high contrast {} on {}: {actual:.2}",
                foreground.name(),
                background.name()
            );
        }
    }
}

#[test]
fn any_user_accent_derives_a_passing_family() {
    let pack = default_pack();
    for seed in [
        "#ffeb3b", "#00e5ff", "#b4232c", "#7c4dff", "#808080", "#000000", "#ffffff",
    ] {
        for appearance in [Appearance::Light, Appearance::Dark] {
            let prefs = Prefs {
                appearance,
                accent: Some(hex(seed)),
                ..Prefs::default()
            };
            let theme = resolve(&pack, &prefs).unwrap();
            for &(foreground, background, ratio) in CONTRAST {
                let actual = contrast(theme.color(foreground), theme.color(background));
                assert!(
                    actual + 1e-9 >= f64::from(ratio),
                    "{seed} {appearance:?}: {} on {} is {actual:.2}",
                    foreground.name(),
                    background.name()
                );
            }
            assert_eq!(theme.color(Role::Focus), theme.color(Role::Accent));
        }
    }
}

#[test]
fn check_names_each_failing_pair_unknown_role_and_missing_role() {
    let mut pack = default_pack();
    pack.appearance
        .light
        .insert("text-muted".into(), hex("#b0b6c2"));
    pack.appearance.light.insert("brand".into(), hex("#123456"));
    pack.appearance.dark.remove("line");
    pack.version = 2;
    let problems: Vec<String> = check(&pack).iter().map(ToString::to_string).collect();
    assert!(
        problems.contains(&"pack: version 2 is unsupported; use 1".to_string()),
        "{problems:?}"
    );
    assert!(problems.contains(&"light: unknown role brand".to_string()));
    assert!(
        problems.iter().any(
            |p| p.starts_with("light: text-muted on surface is ") && p.ends_with("needs 4.5:1")
        )
    );
    assert!(problems.contains(&"dark: THEME: lingxia-default has no line".to_string()));
}

#[test]
fn explicit_high_contrast_colours_win_and_are_held_to_boosted_pairs() {
    let mut pack = default_pack();
    let mut strong = pack.appearance.light.clone();
    strong.insert("text-muted".into(), hex("#596579"));
    pack.appearance.high_contrast_light = Some(strong);
    let prefs = Prefs {
        high_contrast: true,
        ..Prefs::default()
    };
    assert_eq!(
        resolve(&pack, &prefs).unwrap().color(Role::TextMuted),
        hex("#596579")
    );
    assert!(
        check(&pack)
            .iter()
            .any(|p| p.appearance == "high-contrast-light"
                && p.message.starts_with("text-muted on surface"))
    );
}

#[test]
fn design_tokens_import_replaces_named_roles_and_rejects_other_types() {
    let base = default_pack();
    let tokens = serde_json::json!({"color": {"light": {"canvas": {"$type": "color", "$value": "#fafafa"}}}});
    let pack = Pack::from_dtcg(&tokens, &base).unwrap();
    assert_eq!(pack.appearance.light["canvas"], hex("#fafafa"));
    assert_eq!(pack.appearance.dark, base.appearance.dark);
    let wrong = serde_json::json!({"color": {"light": {"canvas": {"$type": "dimension", "$value": "4px"}}}});
    assert!(Pack::from_dtcg(&wrong, &base).is_err());
    assert!(Pack::from_dtcg(&serde_json::json!({}), &base).is_err());
}

#[test]
fn roles_are_kebab_names_in_slot_order() {
    assert_eq!(Role::ALL.len(), ROLE_COUNT);
    for (slot, role) in Role::ALL.into_iter().enumerate() {
        assert_eq!(role as usize, slot);
        assert_eq!(Role::from_name(role.name()), Some(role));
    }
    assert_eq!(Role::from_name("chart-5"), Some(Role::Chart5));
}

#[test]
fn a_partial_pack_completes_from_the_default_and_derives_its_accent() {
    let pack: Pack = serde_json::from_str(
        r##"{"name":"brand","version":1,"accent":{"seed":"#0f9d58","derive":true}}"##,
    )
    .unwrap();
    assert!(
        !check(&pack).is_empty(),
        "missing roles stay visible before completion"
    );
    let pack = complete(pack);
    assert_eq!(check(&pack), Vec::new());
    let base = default_pack();
    assert_eq!(
        (&pack.typography, &pack.shape, &pack.space, &pack.motion),
        (&base.typography, &base.shape, &base.space, &base.motion)
    );
    for appearance in [Appearance::Light, Appearance::Dark] {
        let prefs = Prefs {
            appearance,
            ..Prefs::default()
        };
        let (ours, theirs) = (
            resolve(&pack, &prefs).unwrap(),
            resolve(&base, &prefs).unwrap(),
        );
        assert_ne!(ours.color(Role::Accent), theirs.color(Role::Accent));
        assert_eq!(ours.color(Role::Surface), theirs.color(Role::Surface));
    }
}

#[test]
fn a_pack_without_surface_overlay_floats_overlays_on_its_own_surface_raised() {
    let old: Pack = serde_json::from_str(
        r##"{"name":"old","version":1,"accent":{"seed":"#365cdd","derive":true},"appearance":{
            "light":{"surface-raised":"#fafafa"},
            "dark":{"surface-raised":"#202020"}}}"##,
    )
    .unwrap();
    let old = complete(old);
    assert_eq!(check(&old), Vec::new());
    for (appearance, raised) in [
        (Appearance::Light, "#fafafa"),
        (Appearance::Dark, "#202020"),
    ] {
        let prefs = Prefs {
            appearance,
            ..Prefs::default()
        };
        let theme = resolve(&old, &prefs).unwrap();
        assert_eq!(theme.color(Role::SurfaceOverlay), hex(raised));
    }

    // An explicit high-contrast set is not completed; it still resolves without the role.
    let mut contrast_set = default_pack();
    let mut dark = contrast_set.appearance.dark.clone();
    dark.remove(Role::SurfaceOverlay.name());
    contrast_set.appearance.high_contrast_dark = Some(dark);
    let prefs = Prefs {
        appearance: Appearance::Dark,
        high_contrast: true,
        ..Prefs::default()
    };
    let theme = resolve(&contrast_set, &prefs).unwrap();
    assert_eq!(
        theme.color(Role::SurfaceOverlay),
        theme.color(Role::SurfaceRaised)
    );

    // The showcase pack sets only an accent, so its overlays take the default pack's.
    let showcase: Pack = serde_json::from_str(include_str!(
        "../../../examples/lingxia-showcase-gpu/lxapp/theme/pack.json"
    ))
    .unwrap();
    assert!(showcase.appearance.light.is_empty() && showcase.appearance.dark.is_empty());
    let showcase = complete(showcase);
    for (appearance, overlay) in [
        (Appearance::Light, "#ffffff"),
        (Appearance::Dark, "#2d3546"),
    ] {
        let prefs = Prefs {
            appearance,
            ..Prefs::default()
        };
        assert_eq!(
            resolve(&showcase, &prefs)
                .unwrap()
                .color(Role::SurfaceOverlay),
            hex(overlay)
        );
    }
}

#[test]
fn a_pack_without_status_marks_draws_them_in_its_status_colours() {
    let mut pack = default_pack();
    let marks = [
        (Role::SuccessMark, Role::Success),
        (Role::WarningMark, Role::Warning),
        (Role::DangerMark, Role::Danger),
    ];
    for colors in [&mut pack.appearance.light, &mut pack.appearance.dark] {
        for (mark, _) in marks {
            colors.remove(mark.name());
        }
    }
    assert_eq!(
        check(&pack),
        Vec::new(),
        "it is not held to the marks' pairs"
    );
    for appearance in [Appearance::Light, Appearance::Dark] {
        for high_contrast in [false, true] {
            let prefs = Prefs {
                appearance,
                high_contrast,
                ..Prefs::default()
            };
            let theme = resolve(&pack, &prefs).unwrap();
            for (mark, status) in marks {
                assert_eq!(theme.color(mark), theme.color(status), "{}", mark.name());
            }
        }
    }
}

#[test]
fn a_pack_without_a_knob_keeps_the_white_thumb() {
    let pale: Pack = serde_json::from_str(
        r##"{"name":"pale","version":1,"accent":{"seed":"#365cdd","derive":true},"appearance":{"light":{"on-accent":"#101010"}}}"##,
    )
    .unwrap();
    let mut packs = vec![complete(pale)];
    let mut contrast_set = default_pack();
    let mut dark = contrast_set.appearance.dark.clone();
    dark.remove(Role::Knob.name());
    contrast_set.appearance.high_contrast_dark = Some(dark);
    packs.push(contrast_set);
    for pack in &packs {
        for appearance in [Appearance::Light, Appearance::Dark] {
            for high_contrast in [false, true] {
                let prefs = Prefs {
                    appearance,
                    high_contrast,
                    ..Prefs::default()
                };
                let theme = resolve(pack, &prefs).unwrap();
                assert_eq!(theme.color(Role::Knob), hex("#ffffff"), "{}", pack.name);
            }
        }
    }
}

#[test]
fn the_selection_highlight_stands_off_the_page_and_keeps_text_readable() {
    for appearance in [Appearance::Light, Appearance::Dark] {
        for high_contrast in [false, true] {
            let theme = resolve(
                &default_pack(),
                &Prefs {
                    appearance,
                    high_contrast,
                    ..Prefs::default()
                },
            )
            .unwrap();
            let selection = theme.color(Role::Selection);
            for background in [Role::Surface, Role::Canvas] {
                assert!(
                    contrast(selection, theme.color(background)) >= 3.0,
                    "{appearance:?} {high_contrast}"
                );
            }
            assert!(
                contrast(theme.color(Role::Text), selection) >= 4.5,
                "{appearance:?} {high_contrast}"
            );
        }
    }
}

#[test]
fn media_roles_ignore_appearance_and_an_app_pack_may_override_them() {
    let pack = default_pack();
    let dark = Prefs {
        appearance: Appearance::Dark,
        ..Prefs::default()
    };
    let light = resolve(&pack, &Prefs::default()).unwrap();
    let night = resolve(&pack, &dark).unwrap();
    for role in [Role::Media, Role::OnMedia] {
        assert_eq!(light.color(role), night.color(role), "{}", role.name());
    }
    let brand: Pack = serde_json::from_str(
        r##"{"name":"brand","version":1,"accent":{"seed":"#365cdd","derive":true},"appearance":{"light":{"media":"#101820cc","on-media":"#fff8e7"},"dark":{}}}"##,
    )
    .unwrap();
    let brand = complete(brand);
    assert_eq!(check(&brand), vec![]);
    let theme = resolve(&brand, &Prefs::default()).unwrap();
    assert_eq!(theme.color(Role::OnMedia), hex("#fff8e7"));
    assert_eq!(
        resolve(&brand, &dark).unwrap().color(Role::OnMedia),
        hex("#ffffff")
    );
}

#[test]
fn the_resolved_default_palette_is_recorded_for_javascript() {
    // contracts/view/palette.json feeds @lingxia/view/theme; re-record with LINGXIA_UPDATE_PALETTE=1.
    let pack = default_pack();
    let appearances =
        [("light", Appearance::Light), ("dark", Appearance::Dark)].map(|(name, appearance)| {
            let prefs = Prefs {
                appearance,
                ..Prefs::default()
            };
            let theme = resolve(&pack, &prefs).unwrap();
            let roles: Vec<String> = Role::ALL
                .iter()
                .map(|role| {
                    let [r, g, b, a] = theme.color(*role).0;
                    format!("    \"{}\": \"#{r:02x}{g:02x}{b:02x}{a:02x}\"", role.name())
                })
                .collect();
            format!("  \"{name}\": {{\n{}\n  }}", roles.join(",\n"))
        });
    let recorded = format!("{{\n{}\n}}\n", appearances.join(",\n"));
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../contracts/view/palette.json"
    );
    if std::env::var_os("LINGXIA_UPDATE_PALETTE").is_some() {
        std::fs::write(path, &recorded).unwrap();
    }
    let committed = std::fs::read_to_string(path).unwrap_or_default();
    assert!(
        committed.replace("\r\n", "\n") == recorded,
        "contracts/view/palette.json is stale; re-record with LINGXIA_UPDATE_PALETTE=1 cargo test -p lingxia-theme, then run node contracts/view/generate.mjs"
    );
}

#[test]
fn a_pack_without_a_media_canvas_uses_its_darkest_surface() {
    let mut pack = default_pack();
    for colors in [&mut pack.appearance.light, &mut pack.appearance.dark] {
        colors.remove(Role::MediaCanvas.name());
    }
    assert_eq!(
        check(&pack),
        Vec::new(),
        "it is not held to the media pairs on a borrowed surface"
    );
    for appearance in [Appearance::Light, Appearance::Dark] {
        let prefs = Prefs {
            appearance,
            ..Prefs::default()
        };
        let theme = resolve(&pack, &prefs).unwrap();
        let darkest = [
            Role::Canvas,
            Role::Surface,
            Role::SurfaceRaised,
            Role::SurfaceSunken,
        ]
        .into_iter()
        .map(|role| theme.color(role))
        .min_by(|a, b| a.luminance().total_cmp(&b.luminance()))
        .unwrap();
        assert_eq!(theme.color(Role::MediaCanvas), darkest);
    }
}

#[test]
fn the_default_media_canvas_is_opaque_and_carries_on_media_text() {
    for appearance in [Appearance::Light, Appearance::Dark] {
        let prefs = Prefs {
            appearance,
            ..Prefs::default()
        };
        let theme = resolve(&default_pack(), &prefs).unwrap();
        let canvas = theme.color(Role::MediaCanvas);
        assert_eq!(canvas.0[3], 255, "opaque");
        assert!(contrast(theme.color(Role::OnMedia), canvas) >= 4.5);
    }
}
