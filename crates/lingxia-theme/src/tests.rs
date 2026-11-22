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
        "#ffeb3b", "#00e5ff", "#b4232c", "#7c4dff", "#808080", "#000000", "#ffffff", "#019954",
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
            // A press deepens, under a dark label too; only a black fill has nowhere to go.
            let [fill, hover, pressed] = [
                Role::AccentSolid,
                Role::AccentSolidHover,
                Role::AccentSolidPressed,
            ]
            .map(|role| theme.color(role).oklch().l);
            if fill > 0.1 {
                assert!(
                    fill > hover && hover > pressed,
                    "{seed} {appearance:?}: press deepens"
                );
            }
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
fn overlays_float_on_surface_raised_a_step_lighter_in_dark() {
    let old: Pack = serde_json::from_str(
        r##"{"name":"old","version":1,"accent":{"seed":"#365cdd","derive":true},"appearance":{
            "light":{"surface-raised":"#ffffff"},
            "dark":{"surface-raised":"#202020"}}}"##,
    )
    .unwrap();
    let old = complete(old);
    assert!(
        !old.appearance
            .dark
            .contains_key(Role::SurfaceOverlay.name()),
        "the default pack's overlay is not copied"
    );
    assert_eq!(check(&old), Vec::new());
    let light = resolve(&old, &Prefs::default()).unwrap();
    assert_eq!(light.color(Role::SurfaceOverlay), hex("#ffffff"));
    for high_contrast in [false, true] {
        let prefs = Prefs {
            appearance: Appearance::Dark,
            high_contrast,
            ..Prefs::default()
        };
        let dark = resolve(&old, &prefs).unwrap();
        let (raised, overlay) = (
            dark.color(Role::SurfaceRaised).oklch(),
            dark.color(Role::SurfaceOverlay).oklch(),
        );
        assert!(
            overlay.l > raised.l + 0.03 && overlay.l < raised.l + 0.06,
            "{high_contrast}: {} against {}",
            overlay.l,
            raised.l
        );
    }

    // A raised surface too light for faint text to read on moves off it.
    let mut pale = default_pack();
    pale.appearance
        .light
        .insert("surface-raised".into(), hex("#f4f4f4"));
    let theme = resolve(&pale, &Prefs::default()).unwrap();
    assert!(
        contrast(
            theme.color(Role::TextFaint),
            theme.color(Role::SurfaceOverlay)
        ) >= 3.0
    );

    // An explicit high-contrast set is not completed; it still resolves without the role.
    let mut contrast_set = default_pack();
    let dark = contrast_set.appearance.dark.clone();
    contrast_set.appearance.high_contrast_dark = Some(dark);
    let prefs = Prefs {
        appearance: Appearance::Dark,
        high_contrast: true,
        ..Prefs::default()
    };
    assert!(resolve(&contrast_set, &prefs).is_ok());

    // A pack an overlay was written for keeps it.
    let mut own = default_pack();
    own.appearance
        .dark
        .insert("surface-overlay".into(), hex("#303846"));
    let prefs = Prefs {
        appearance: Appearance::Dark,
        ..Prefs::default()
    };
    assert_eq!(
        resolve(&own, &prefs).unwrap().color(Role::SurfaceOverlay),
        hex("#303846")
    );
}

#[test]
fn status_marks_are_fuller_in_light_and_the_status_tone_in_dark() {
    let pack = default_pack();
    let marks = [
        (Role::SuccessMark, Role::Success),
        (Role::WarningMark, Role::Warning),
        (Role::DangerMark, Role::Danger),
    ];
    let light = resolve(&pack, &Prefs::default()).unwrap();
    for (mark, status) in marks {
        let (dot, text) = (light.color(mark).oklch(), light.color(status).oklch());
        assert!(dot.l > text.l && dot.c >= text.c, "{}", mark.name());
    }
    let prefs = Prefs {
        appearance: Appearance::Dark,
        ..Prefs::default()
    };
    let dark = resolve(&pack, &prefs).unwrap();
    for (mark, status) in marks {
        assert_eq!(dark.color(mark), dark.color(status), "{}", mark.name());
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
fn a_pack_without_its_near_blacks_takes_them_from_its_own_dark_canvas() {
    let warm = warm_pack();
    assert_eq!(check(&warm), Vec::new());
    let canvas = warm.appearance.dark["canvas"].oklch();
    for appearance in [Appearance::Light, Appearance::Dark] {
        let prefs = Prefs {
            appearance,
            ..Prefs::default()
        };
        let (theme, default) = (
            resolve(&warm, &prefs).unwrap(),
            resolve(&default_pack(), &prefs).unwrap(),
        );
        for role in [Role::Scrim, Role::Media, Role::MediaCanvas] {
            let color = theme.color(role);
            if role == Role::Scrim && appearance == Appearance::Dark {
                assert_eq!(color, default.color(role), "plain black");
                continue;
            }
            assert_ne!(color, default.color(role), "{appearance:?} {}", role.name());
            assert_eq!(color.0[3], default.color(role).0[3], "{}", role.name());
            let tone = color.oklch();
            assert!(
                tone.l <= 0.19 && derive::apart(tone.h, canvas.h) < 15.0,
                "{}",
                role.name()
            );
        }
        assert!(contrast(theme.color(Role::OnMedia), theme.color(Role::MediaCanvas)) >= 4.5);
    }
    // The default pack writes its own, so it reads the same as before.
    let mut bare = default_pack();
    for colors in [&mut bare.appearance.light, &mut bare.appearance.dark] {
        for role in [Role::Scrim, Role::Media, Role::MediaCanvas] {
            colors.remove(role.name());
        }
    }
    assert_eq!(check(&bare), Vec::new());
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

#[test]
fn info_is_a_fixed_blue_and_a_blue_accent_lends_its_family() {
    let pack = default_pack();
    let family = [
        (Role::Info, Role::Accent),
        (Role::OnInfo, Role::OnAccent),
        (Role::InfoSoft, Role::AccentSoft),
        (Role::InfoMark, Role::Accent),
    ];
    for appearance in [Appearance::Light, Appearance::Dark] {
        for high_contrast in [false, true] {
            let prefs = Prefs {
                appearance,
                high_contrast,
                ..Prefs::default()
            };
            let theme = resolve(&pack, &prefs).unwrap();
            for (info, lender) in family {
                assert_eq!(theme.color(info), theme.color(lender), "{}", info.name());
            }
            // An orange accent leaves info blue, as light and as strong as the accent.
            let prefs = Prefs {
                accent: Some(hex("#b4532a")),
                ..prefs
            };
            let theme = resolve(&pack, &prefs).unwrap();
            let (info, accent) = (
                theme.color(Role::Info).oklch(),
                theme.color(Role::Accent).oklch(),
            );
            assert!((info.h - 250.0).abs() < 3.0, "{}", info.h);
            assert!((info.l - accent.l).abs() < 0.08, "{appearance:?}");
            assert!((theme.color(Role::InfoSoft).oklch().h - 250.0).abs() < 6.0);
        }
    }
    // A grey accent still gets a blue notice.
    let prefs = Prefs {
        accent: Some(hex("#808080")),
        ..Prefs::default()
    };
    let theme = resolve(&pack, &prefs).unwrap();
    assert!(theme.color(Role::Info).oklch().c >= 0.09);
}

#[test]
fn a_pack_with_its_own_info_is_held_to_the_info_pairs() {
    let mut pack = default_pack();
    for colors in [&mut pack.appearance.light, &mut pack.appearance.dark] {
        colors.insert(Role::Info.name().into(), hex("#8fb4e8"));
        colors.insert(Role::OnInfo.name().into(), hex("#ffffff"));
        colors.insert(Role::InfoSoft.name().into(), hex("#e8f0fb"));
    }
    let theme = resolve(&pack, &Prefs::default()).unwrap();
    assert_eq!(theme.color(Role::Info), hex("#8fb4e8"));
    assert_ne!(theme.color(Role::InfoMark), hex("#8fb4e8"));
    assert!(
        contrast(theme.color(Role::InfoMark), theme.color(Role::Surface)) >= 3.0,
        "its mark is built from it to stand off the surface"
    );
    let problems = check(&pack);
    assert!(
        problems
            .iter()
            .any(|problem| problem.message.contains("on-info on info")),
        "{problems:?}"
    );
}

/// Cream and brown surfaces under an orange accent, authored roles only.
fn warm_pack() -> Pack {
    let pack = serde_json::from_str(
        r##"{"name":"warm","version":1,"accent":{"seed":"#c4532a","derive":true},"appearance":{
            "light":{"canvas":"#faf6f0","surface":"#fffdf9","surface-raised":"#ffffff","surface-sunken":"#f2ebe1",
                "line":"#e7ddd0","line-strong":"#8c7f71","text":"#2a211b","text-muted":"#6a5c50","text-faint":"#978877",
                "danger":"#c2255a","success":"#2f7a48","warning":"#806a00"},
            "dark":{"canvas":"#161210","surface":"#201b17","surface-raised":"#2b2520","surface-sunken":"#110e0c",
                "line":"#3a322a","line-strong":"#7f7366","text":"#f2ece5","text-muted":"#b3a79a","text-faint":"#877b6f",
                "danger":"#f4728a","success":"#6fc48c","warning":"#e6b84a"}}}"##,
    )
    .unwrap();
    complete(pack)
}

/// The default pack with its success written on the colour its teal hue would take.
fn status_on_a_hue_pack() -> Pack {
    let mut pack = default_pack();
    pack.name = "status-on-hue".into();
    for (appearance, colors) in [
        (Appearance::Light, &mut pack.appearance.light),
        (Appearance::Dark, &mut pack.appearance.dark),
    ] {
        let prefs = Prefs {
            appearance,
            ..Prefs::default()
        };
        let teal = resolve(&default_pack(), &prefs).unwrap().color(Role::Hue6);
        colors.insert("success".into(), teal);
    }
    pack
}

#[test]
fn a_derived_hue_gives_way_to_a_status_it_reads_as() {
    let pack = status_on_a_hue_pack();
    for appearance in [Appearance::Light, Appearance::Dark] {
        let prefs = Prefs {
            appearance,
            ..Prefs::default()
        };
        let (theme, default) = (
            resolve(&pack, &prefs).unwrap(),
            resolve(&default_pack(), &prefs).unwrap(),
        );
        assert_eq!(theme.color(Role::Success), default.color(Role::Hue6));
        let apart = distance(theme.color(Role::Hue6), theme.color(Role::Success));
        assert!(
            apart >= 5.0,
            "{appearance:?}: hue-6 is {apart:.1} from success"
        );
        // Nothing else moves, the series least of all: a chart is its own context.
        for role in [Role::Hue2, Role::Hue3, Role::Hue4, Role::Hue5, Role::Hue7] {
            assert_eq!(theme.color(role), default.color(role), "{}", role.name());
        }
        for role in [Role::Chart2, Role::Chart3, Role::Chart4, Role::Chart5] {
            assert_eq!(theme.color(role), default.color(role), "{}", role.name());
        }
    }
    assert_eq!(advise(&pack), Vec::new());
}

fn every(mut visit: impl FnMut(Prefs, &str)) {
    for (appearance, name) in [(Appearance::Light, "light"), (Appearance::Dark, "dark")] {
        for high_contrast in [false, true] {
            let prefs = Prefs {
                appearance,
                high_contrast,
                ..Prefs::default()
            };
            let label = format!("{name}{}", if high_contrast { " high" } else { "" });
            visit(prefs, &label);
        }
    }
}

#[test]
fn roles_fall_in_three_tiers() {
    let named = |tier| -> Vec<&str> {
        Role::ALL
            .into_iter()
            .filter(|role| role.tier() == tier)
            .map(Role::name)
            .collect()
    };
    assert_eq!(
        named(Tier::Authored),
        [
            "canvas",
            "surface",
            "surface-raised",
            "surface-sunken",
            "line",
            "text",
            "text-muted",
            "text-faint",
            "danger",
            "success",
            "warning",
            "line-strong"
        ]
    );
    assert_eq!(
        named(Tier::Fixed),
        [
            "scrim",
            "shadow",
            "media",
            "on-media",
            "knob",
            "media-canvas"
        ]
    );
    // The default pack writes no derived role: every one is computed.
    let pack = default_pack();
    for colors in [&pack.appearance.light, &pack.appearance.dark] {
        for name in colors.keys() {
            assert_ne!(
                Role::from_name(name).unwrap().tier(),
                Tier::Derived,
                "{name}"
            );
        }
    }
}

#[test]
fn completion_copies_authored_and_fixed_roles_and_never_a_derived_one() {
    let seed: Pack = serde_json::from_str(
        r##"{"name":"seed","version":1,"appearance":{"light":{"surface-sunken":"#f2ebe1"}}}"##,
    )
    .unwrap();
    let pack = complete(seed);
    let base = default_pack();
    assert_eq!(pack.accent, base.accent, "the seed is authored");
    assert_eq!(pack.appearance.light["surface-sunken"], hex("#f2ebe1"));
    assert_eq!(
        pack.appearance.light["shadow"],
        base.appearance.light["shadow"]
    );
    assert!(
        !pack.appearance.light.contains_key("scrim"),
        "near-blacks are the pack's own"
    );
    for colors in [&pack.appearance.light, &pack.appearance.dark] {
        for name in colors.keys() {
            assert_ne!(
                Role::from_name(name).unwrap().tier(),
                Tier::Derived,
                "{name}"
            );
        }
    }
    // A pack that writes its accent without a seed keeps it.
    let own: Pack = serde_json::from_str(
        r##"{"name":"own","version":1,"appearance":{"light":{"accent":"#0b6e4f"},"dark":{"accent":"#58c9a0"}}}"##,
    )
    .unwrap();
    let own = complete(own);
    assert_eq!(own.accent, None);
    assert_eq!(check(&own), Vec::new());
    let theme = resolve(&own, &Prefs::default()).unwrap();
    assert_eq!(theme.color(Role::Accent), hex("#0b6e4f"));
    assert_eq!(theme.color(Role::Chart1), hex("#0b6e4f"));
    assert!(contrast(theme.color(Role::OnAccent), hex("#0b6e4f")) >= 4.5);
}

#[test]
fn a_pack_of_authored_roles_derives_a_whole_theme_from_its_own_colours() {
    let pack = warm_pack();
    assert_eq!(check(&pack), Vec::new());
    let base = default_pack();
    every(|prefs, label| {
        let theme = resolve(&pack, &prefs).unwrap();
        let default = resolve(&base, &prefs).unwrap();
        for &(foreground, background, ratio) in CONTRAST {
            let needed = if prefs.high_contrast && !involves_selection(foreground, background) {
                boosted(ratio)
            } else {
                f64::from(ratio)
            };
            let actual = contrast(theme.color(foreground), theme.color(background));
            assert!(
                actual + 1e-9 >= needed,
                "{label}: {} on {} is {actual:.2}",
                foreground.name(),
                background.name()
            );
        }
        // Nothing derived is the default pack's: fills are tinted from this pack's surface.
        for role in [
            Role::DangerSoft,
            Role::Hue2Soft,
            Role::Hue8,
            Role::SurfaceOverlay,
            Role::Info,
            Role::WarningMark,
        ] {
            if prefs.appearance == Appearance::Light && role == Role::SurfaceOverlay {
                continue;
            }
            assert_ne!(
                theme.color(role),
                default.color(role),
                "{label}: {}",
                role.name()
            );
        }
        // A hue keeps its contrast with the surface whatever the surface is.
        if !prefs.high_contrast {
            for role in [Role::Hue2, Role::Hue6, Role::Chart4] {
                let (ours, theirs) = (
                    contrast(theme.color(role), theme.color(Role::Surface)),
                    contrast(default.color(role), default.color(Role::Surface)),
                );
                assert!((ours - theirs).abs() < 0.2, "{label}: {}", role.name());
            }
        }
    });
}

#[test]
fn a_crowded_colour_gives_way_to_a_spare_in_both_appearances() {
    let angle = |theme: &ResolvedTheme, role: Role| theme.color(role).oklch().h;
    let near = |a: f64, b: f64| derive::apart(a, b) < 4.0;
    let warm = warm_pack();
    let base = default_pack();
    for appearance in [Appearance::Light, Appearance::Dark] {
        let prefs = Prefs {
            appearance,
            ..Prefs::default()
        };
        let theme = resolve(&warm, &prefs).unwrap();
        // The orange series sits on the orange accent and the brown hue on the warm neutral:
        // both take the blue this accent leaves empty. Pink, crowded by the accent's pastel in
        // dark, goes green in light too.
        assert!(near(angle(&theme, Role::Chart2), 267.0), "{appearance:?}");
        assert!(near(angle(&theme, Role::Hue3), 267.0), "{appearance:?}");
        assert!(near(angle(&theme, Role::Hue5), 150.0), "{appearance:?}");
        // The brown series, crowded by the accent in dark, goes violet: blue is taken, and a
        // series is never grey.
        let violet = theme.color(Role::Chart6).oklch();
        assert!(near(violet.h, 290.0) && violet.c > 0.1, "{appearance:?}");
        let default = resolve(&base, &prefs).unwrap();
        for role in [
            Role::Hue2,
            Role::Hue4,
            Role::Hue6,
            Role::Hue7,
            Role::Chart3,
            Role::Chart4,
            Role::Chart5,
            Role::Chart7,
            Role::Chart8,
        ] {
            assert!(
                near(angle(&theme, role), angle(&default, role)),
                "{appearance:?}: {} stays",
                role.name()
            );
        }
        assert!(near(angle(&theme, Role::Hue1), angle(&theme, Role::Accent)));
        assert_eq!(theme.color(Role::Chart1), theme.color(Role::Accent));
        assert!(near(
            angle(&theme, Role::Hue8),
            angle(&theme, Role::TextMuted)
        ));
    }
    // A small change of accent moves nothing: the choice has one threshold, not a slope.
    let moved = |seed: &str| {
        let prefs = Prefs {
            accent: Some(hex(seed)),
            ..Prefs::default()
        };
        let theme = resolve(&warm, &prefs).unwrap();
        (theme.color(Role::Chart2), theme.color(Role::Hue3))
    };
    assert_eq!(moved("#c4532a"), moved("#c65a2a"));
}

/// The least distance between two of `roles`, or from one of `roles` to one of `others`.
fn least(theme: &ResolvedTheme, roles: &[Role], others: Option<&[Role]>) -> (f64, Role, Role) {
    least_by(distance, theme, roles, others)
}

fn least_by(
    measure: fn(Rgba, Rgba) -> f64,
    theme: &ResolvedTheme,
    roles: &[Role],
    others: Option<&[Role]>,
) -> (f64, Role, Role) {
    let mut found = (f64::INFINITY, roles[0], roles[0]);
    for (index, &a) in roles.iter().enumerate() {
        for &b in others.unwrap_or(&roles[index + 1..]) {
            let gap = measure(theme.color(a), theme.color(b));
            if gap < found.0 {
                found = (gap, a, b);
            }
        }
    }
    found
}

#[test]
fn hues_and_series_stand_apart_under_any_accent() {
    // `hue-1` and `hue-8` are left out: both are the pack's own, the accent and the neutral, and
    // a dull accent brings them together whatever the rest does.
    let hues = [
        Role::Hue2,
        Role::Hue3,
        Role::Hue4,
        Role::Hue5,
        Role::Hue6,
        Role::Hue7,
    ];
    let own = [Role::Hue1, Role::Hue8];
    let series = [
        Role::Chart1,
        Role::Chart2,
        Role::Chart3,
        Role::Chart4,
        Role::Chart5,
        Role::Chart6,
        Role::Chart7,
        Role::Chart8,
    ];
    for pack in [default_pack(), warm_pack(), status_on_a_hue_pack()] {
        for (l, c) in [(0.6, 0.17), (0.75, 0.2), (0.5, 0.12)] {
            for step in 0..72 {
                let h = f64::from(step * 5);
                let seed = Oklch { l, c, h }.rgba(255);
                for appearance in [Appearance::Light, Appearance::Dark] {
                    let prefs = Prefs {
                        appearance,
                        accent: Some(seed),
                        ..Prefs::default()
                    };
                    let theme = resolve(&pack, &prefs).unwrap();
                    let label = format!("{} {} {appearance:?}", pack.name, seed.hex());
                    for &(foreground, background, ratio) in CONTRAST {
                        let actual = contrast(theme.color(foreground), theme.color(background));
                        assert!(
                            actual + 1e-9 >= f64::from(ratio),
                            "{label}: {} on {} is {actual:.2}",
                            foreground.name(),
                            background.name()
                        );
                    }
                    for (found, floor) in [
                        (least(&theme, &hues, None), 9.4),
                        (least(&theme, &hues, Some(&own)), 9.4),
                        // An accent that crowds two series leaves the second's spare a little
                        // nearer its neighbours: series stay colours, never grey.
                        (least(&theme, &series, None), 9.5),
                    ] {
                        let (gap, a, b) = found;
                        assert!(
                            gap >= floor,
                            "{label}: {} and {} are {gap:.2} apart",
                            a.name(),
                            b.name()
                        );
                    }
                    // Apart in hue and chroma too: lightness alone does not make two categories.
                    let plane = derive::plane;
                    for (found, floor) in [
                        (least_by(plane, &theme, &hues, None), 4.7),
                        (least_by(plane, &theme, &hues, Some(&own)), 4.7),
                        (least_by(plane, &theme, &series, None), 5.0),
                    ] {
                        let (gap, a, b) = found;
                        assert!(
                            gap >= floor,
                            "{label}: {} and {} are {gap:.2} apart in the chroma plane",
                            a.name(),
                            b.name()
                        );
                    }
                }
            }
        }
    }
}

/// The packs that ship: the default and Liuli's warm.
fn shipped_packs() -> [Pack; 2] {
    let warm = include_str!("../../../packages/liuli/themes/warm/pack.json");
    [
        default_pack(),
        complete(serde_json::from_str(warm).unwrap()),
    ]
}

#[test]
fn every_shipped_chart_colour_is_a_colour_apart_from_the_other_seven() {
    let series = [
        Role::Chart1,
        Role::Chart2,
        Role::Chart3,
        Role::Chart4,
        Role::Chart5,
        Role::Chart6,
        Role::Chart7,
        Role::Chart8,
    ];
    for pack in shipped_packs() {
        for appearance in [Appearance::Light, Appearance::Dark] {
            let prefs = Prefs {
                appearance,
                ..Prefs::default()
            };
            let theme = resolve(&pack, &prefs).unwrap();
            let label = format!("{} {appearance:?}", pack.name);
            for role in series {
                let chroma = theme.color(role).oklch().c;
                assert!(
                    chroma >= 0.08,
                    "{label}: {} is grey ({chroma:.3})",
                    role.name()
                );
            }
            for (found, floor) in [
                (least(&theme, &series, None), 10.5),
                (least_by(derive::plane, &theme, &series, None), 5.5),
            ] {
                let (gap, a, b) = found;
                assert!(
                    gap >= floor,
                    "{label}: {} and {} are {gap:.2} apart",
                    a.name(),
                    b.name()
                );
            }
        }
    }
}

#[test]
fn solid_fills_are_the_plain_roles_in_light_and_deeper_under_white_in_dark() {
    let pairs = [
        (
            Role::Accent,
            Role::OnAccent,
            Role::AccentSolid,
            Role::OnAccentSolid,
        ),
        (
            Role::Danger,
            Role::OnDanger,
            Role::DangerSolid,
            Role::OnDangerSolid,
        ),
    ];
    for pack in shipped_packs() {
        assert_eq!(check(&pack), Vec::new());
        every(|prefs, label| {
            let theme = resolve(&pack, &prefs).unwrap();
            for (plain, on_plain, solid, on_solid) in pairs {
                let label = format!("{} {label} {}", pack.name, solid.name());
                if prefs.appearance == Appearance::Light {
                    assert_eq!(theme.color(solid), theme.color(plain), "{label}");
                    assert_eq!(theme.color(on_solid), theme.color(on_plain), "{label}");
                    continue;
                }
                let (fill, tone) = (theme.color(solid), theme.color(solid).oklch());
                let need = if prefs.high_contrast { 7.0 } else { 4.6 };
                assert_eq!(theme.color(on_solid), hex("#ffffff"), "{label}");
                assert!(contrast(hex("#ffffff"), fill) >= need, "{label}");
                assert!(tone.c >= 0.12, "{label}: chroma {:.3}", tone.c);
                let hue = theme.color(plain).oklch().h;
                assert!(derive::apart(tone.h, hue) < 12.0, "{label}: hue");
            }
        });
    }
    // The default dark accent fill is a saturated blue near #3d63d6, not its pastel text tone.
    let dark = Prefs {
        appearance: Appearance::Dark,
        ..Prefs::default()
    };
    let theme = resolve(&default_pack(), &dark).unwrap();
    assert!(distance(theme.color(Role::AccentSolid), hex("#3d63d6")) < 5.0);
    // A pack may write its own fill; its text is chosen to read on it.
    let mut own = default_pack();
    own.appearance
        .dark
        .insert("danger-solid".into(), hex("#9b1c1c"));
    let theme = resolve(&own, &dark).unwrap();
    assert_eq!(theme.color(Role::DangerSolid), hex("#9b1c1c"));
    assert!(contrast(theme.color(Role::OnDangerSolid), hex("#9b1c1c")) >= 4.5);
    // A user accent recolours the fill with the rest of the accent family.
    let green = Prefs {
        accent: Some(hex("#0f9d58")),
        ..dark
    };
    let hue = resolve(&default_pack(), &green)
        .unwrap()
        .color(Role::AccentSolid)
        .oklch()
        .h;
    assert!(derive::apart(hue, hex("#0f9d58").oklch().h) < 12.0);
}

#[test]
fn status_fills_stand_apart_from_each_other_and_from_the_surfaces() {
    let fills = [
        Role::DangerSoft,
        Role::SuccessSoft,
        Role::WarningSoft,
        Role::InfoSoft,
    ];
    let surfaces = [
        Role::Canvas,
        Role::Surface,
        Role::SurfaceRaised,
        Role::SurfaceSunken,
    ];
    // Warning seeds from orange to yellow, light then dark, on both packs' surfaces.
    let mut packs = vec![default_pack(), warm_pack()];
    for (light, dark) in [
        ("#b45309", "#fbbf24"),
        ("#c2410c", "#fb923c"),
        ("#854d0e", "#facc15"),
        ("#806a00", "#e6b84a"),
    ] {
        for mut pack in [default_pack(), warm_pack()] {
            pack.name = format!("{} with {light}", pack.name);
            pack.appearance.light.insert("warning".into(), hex(light));
            pack.appearance.dark.insert("warning".into(), hex(dark));
            packs.push(pack);
        }
    }
    let accents = [
        None,
        Some("#c4532a"),
        Some("#0f9d58"),
        Some("#7c4dff"),
        Some("#ffeb3b"),
        Some("#00a3ad"),
    ];
    for pack in &packs {
        for accent in accents {
            for appearance in [Appearance::Light, Appearance::Dark] {
                let prefs = Prefs {
                    appearance,
                    accent: accent.map(hex),
                    ..Prefs::default()
                };
                let theme = resolve(pack, &prefs).unwrap();
                let band = theme.color(Role::WarningSoft).oklch().h;
                assert!((72.0..=98.0).contains(&band), "{}: {band:.1}", pack.name);
                for (found, floor) in [
                    (least(&theme, &fills, None), 2.1),
                    (least(&theme, &fills, Some(&surfaces)), 1.3),
                ] {
                    let (gap, a, b) = found;
                    assert!(
                        gap >= floor,
                        "{} {accent:?} {appearance:?}: {} and {} are {gap:.2} apart",
                        pack.name,
                        a.name(),
                        b.name()
                    );
                }
            }
        }
    }
    // The default pack's warning fill is the peach-yellow picked by hand; the warm pack's is a
    // cream-yellow well clear of its sunken surface and its danger fill.
    let light = resolve(&default_pack(), &Prefs::default()).unwrap();
    assert!(distance(light.color(Role::WarningSoft), hex("#fdf1e1")) < 0.5);
    let warm = resolve(&warm_pack(), &Prefs::default()).unwrap();
    for other in [Role::SurfaceSunken, Role::DangerSoft] {
        let gap = distance(warm.color(Role::WarningSoft), warm.color(other));
        assert!(gap >= 4.0, "{}: {gap:.2}", other.name());
    }
}

#[test]
fn a_derived_role_a_pack_writes_is_used_as_written_and_held_to_its_pairs() {
    let mut pack = warm_pack();
    for colors in [&mut pack.appearance.light, &mut pack.appearance.dark] {
        colors.insert("hue-3".into(), hex("#8a5a2b"));
        colors.insert("danger-soft".into(), hex("#ffe0e6"));
    }
    pack.appearance
        .light
        .insert("chart-2".into(), hex("#f6d9a8"));
    every(|prefs, label| {
        let theme = resolve(&pack, &prefs).unwrap();
        assert_eq!(theme.color(Role::DangerSoft), hex("#ffe0e6"), "{label}");
        // High contrast boosts what the pack wrote like any authored colour.
        if prefs.high_contrast {
            let ratio = contrast(theme.color(Role::Hue3), theme.color(Role::Surface));
            assert!(ratio >= 4.5, "{label}: {ratio:.2}");
        } else {
            assert_eq!(theme.color(Role::Hue3), hex("#8a5a2b"), "{label}");
        }
        // Its fill is tinted at its own angle, not the one the rule would have picked.
        let (hue, fill) = (
            theme.color(Role::Hue3).oklch().h,
            theme.color(Role::Hue3Soft).oklch().h,
        );
        assert!((hue - fill).abs() < 12.0, "{label}: {hue} and {fill}");
    });
    let problems: Vec<String> = check(&pack).iter().map(ToString::to_string).collect();
    assert_eq!(
        problems
            .iter()
            .filter(|problem| problem.starts_with("light: chart-2 on "))
            .count(),
        2,
        "{problems:?}"
    );
}

#[test]
fn every_target_resolves_the_same_bytes() {
    // The resolver runs on the device: these palettes are what a pack must look like on all of
    // them. Re-record with LINGXIA_UPDATE_PALETTE=1 after a deliberate change of the rules.
    let mut green = default_pack();
    green.accent = Some(Accent {
        seed: hex("#0f9d58"),
        derive: true,
    });
    let mut lines = Vec::new();
    for (name, pack) in [
        ("default", default_pack()),
        ("warm", warm_pack()),
        ("green", green),
    ] {
        every(|prefs, label| {
            let theme = resolve(&pack, &prefs).unwrap();
            let colors: Vec<String> = theme.palette.iter().map(|color| color.hex()).collect();
            lines.push(format!("{name} {label}: {}", colors.join(" ")));
        });
    }
    let recorded = format!("{}\n", lines.join("\n"));
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/src/golden.txt");
    if std::env::var_os("LINGXIA_UPDATE_PALETTE").is_some() {
        std::fs::write(path, &recorded).unwrap();
    }
    assert_eq!(
        include_str!("golden.txt").replace("\r\n", "\n"),
        recorded,
        "a palette changed; if the rules changed on purpose, re-record with LINGXIA_UPDATE_PALETTE=1 cargo test -p lingxia-theme"
    );
}

#[test]
fn the_default_hues_stand_off_their_surfaces_and_carry_text_on_their_fills() {
    let pack = default_pack();
    let hues = [
        (Role::Hue1, Role::Hue1Soft),
        (Role::Hue2, Role::Hue2Soft),
        (Role::Hue3, Role::Hue3Soft),
        (Role::Hue4, Role::Hue4Soft),
        (Role::Hue5, Role::Hue5Soft),
        (Role::Hue6, Role::Hue6Soft),
        (Role::Hue7, Role::Hue7Soft),
        (Role::Hue8, Role::Hue8Soft),
    ];
    for appearance in [Appearance::Light, Appearance::Dark] {
        let prefs = Prefs {
            appearance,
            ..Prefs::default()
        };
        let theme = resolve(&pack, &prefs).unwrap();
        for (hue, soft) in hues {
            for background in [Role::Surface, Role::SurfaceSunken, soft] {
                let ratio = contrast(theme.color(hue), theme.color(background));
                assert!(
                    ratio >= 3.0,
                    "{} on {}: {ratio:.2}",
                    hue.name(),
                    background.name()
                );
            }
            let ratio = contrast(theme.color(Role::Text), theme.color(soft));
            assert!(ratio >= 4.5, "text on {}: {ratio:.2}", soft.name());
        }
        // Neighbours in the series differ in lightness, so they stay apart without hue.
        for pair in hues.windows(2) {
            let (a, b) = (
                theme.color(pair[0].0).oklch(),
                theme.color(pair[1].0).oklch(),
            );
            assert!(
                (a.l - b.l).abs() >= 0.04,
                "{} and {}",
                pair[0].0.name(),
                pair[1].0.name()
            );
        }
    }
}

#[test]
fn advice_names_a_status_colour_that_reads_as_a_hue() {
    assert_eq!(advise(&default_pack()), Vec::new());
    let advice: Vec<String> = advise(&warm_pack())
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(
        advice,
        [
            "light: warning is 3.2 from hue-4; under 5 they read as one colour",
            "light: accent is 4.10:1 on surface-sunken; as link text it needs 4.5:1",
            "light: accent is 4.16:1 on accent-soft; as link text it needs 4.5:1",
        ]
    );
    // A solid fill lost on the surface is named too.
    let mut faint = default_pack();
    faint
        .appearance
        .dark
        .insert("danger-solid".into(), hex("#2a2f3c"));
    faint
        .appearance
        .dark
        .insert("on-danger-solid".into(), hex("#ffffff"));
    let advice: Vec<String> = advise(&faint).iter().map(ToString::to_string).collect();
    assert!(
        advice
            .iter()
            .any(|line| line.starts_with("dark: danger-solid is ")
                && line.ends_with("on surface; a filled control's edge needs 3.0:1")),
        "{advice:?}"
    );
    assert_eq!(check(&warm_pack()), Vec::new(), "advice is not a failure");
}

#[test]
fn motion_defaults_are_the_default_packs_and_bad_values_are_named() {
    let base = default_pack();
    assert_eq!(base.motion, Motion::default());
    assert_eq!(
        (base.motion.fast, base.motion.base, base.motion.slow),
        (150, 200, 300)
    );
    let partial: Pack =
        serde_json::from_str(r##"{"name":"quick","version":1,"motion":{"fast":90,"spring":0.3}}"##)
            .unwrap();
    assert_eq!(
        partial.motion,
        Motion {
            fast: 90,
            spring: 0.3,
            ..Motion::default()
        }
    );
    for easing in [
        "linear",
        "ease-out",
        "cubic-bezier(0.2, 0, 0, 1)",
        "cubic-bezier(.3,-0.4,1,1.6)",
    ] {
        assert!(is_easing(easing), "{easing}");
    }
    let mut pack = base;
    pack.motion.easing = "standard".into();
    pack.motion.exit = "cubic-bezier(1.2, 0, 1, 1)".into();
    pack.motion.spring = 1.5;
    pack.motion.slow = 60_000;
    let problems: Vec<String> = check(&pack).iter().map(ToString::to_string).collect();
    for start in [
        "pack: motion.easing must be",
        "pack: motion.exit must be",
        "pack: motion.spring is",
        "pack: motion.slow must be",
    ] {
        assert!(
            problems.iter().any(|p| p.starts_with(start)),
            "{start}: {problems:?}"
        );
    }
    assert_eq!(problems.len(), 4, "{problems:?}");
}

#[test]
fn hover_and_pressed_solids_go_deeper_and_a_pack_cannot_write_them() {
    let shades = [
        (
            Role::AccentSolid,
            Role::AccentSolidHover,
            Role::AccentSolidPressed,
        ),
        (
            Role::DangerSolid,
            Role::DangerSolidHover,
            Role::DangerSolidPressed,
        ),
    ];
    let label = |solid: Role| {
        if solid == Role::AccentSolid {
            Role::OnAccentSolid
        } else {
            Role::OnDangerSolid
        }
    };
    for pack in shipped_packs() {
        every(|prefs, name| {
            let theme = resolve(&pack, &prefs).unwrap();
            for (solid, hover, pressed) in shades {
                let tag = format!("{} {name} {}", pack.name, solid.name());
                let [fill, hover, pressed] = [solid, hover, pressed].map(|role| theme.color(role));
                let lightness = |color: Rgba| color.oklch().l;
                for (from, to) in [(fill, hover), (hover, pressed)] {
                    let step = lightness(from) - lightness(to);
                    assert!(step >= 0.04, "{tag}: a state steps {step:.4} deeper");
                }
                let chroma = |color: Rgba| color.oklch().c / color.oklch().l;
                for shade in [hover, pressed] {
                    let hue = (fill.oklch().h - shade.oklch().h).abs();
                    assert!(hue.min(360.0 - hue) < 2.0, "{tag}: hue held");
                    let ratio = chroma(shade) / chroma(fill);
                    assert!(
                        (ratio - 1.0).abs() < 0.04,
                        "{tag}: chroma {ratio:.3} of the fill's"
                    );
                    assert!(contrast(theme.color(label(solid)), shade) >= 4.5, "{tag}");
                }
            }
        });
    }
    // The default pack's shades.
    for (appearance, expected) in [
        (
            Appearance::Light,
            ["#2f52c6", "#2948b0", "#b52e2e", "#a22828"],
        ),
        (
            Appearance::Dark,
            ["#385dd2", "#3152bc", "#b73a3f", "#a43337"],
        ),
    ] {
        let prefs = Prefs {
            appearance,
            ..Prefs::default()
        };
        let theme = resolve(&default_pack(), &prefs).unwrap();
        let got = [
            Role::AccentSolidHover,
            Role::AccentSolidPressed,
            Role::DangerSolidHover,
            Role::DangerSolidPressed,
        ]
        .map(|role| theme.color(role));
        assert_eq!(got, expected.map(hex), "{appearance:?}");
    }
    let mut own = default_pack();
    own.appearance
        .light
        .insert("accent-solid-pressed".into(), hex("#000000"));
    assert!(
        check(&own)
            .iter()
            .any(|problem| problem.message.contains("accent-solid-pressed is derived")),
        "{:?}",
        check(&own)
    );
    let theme = resolve(&own, &Prefs::default()).unwrap();
    assert_eq!(theme.color(Role::AccentSolidPressed), hex("#2948b0"));
}

#[test]
fn a_derived_accent_keeps_the_solids_the_pack_wrote_until_the_user_picks_an_accent() {
    let pack: Pack = serde_json::from_str(
        r##"{"name":"brand","version":1,"accent":{"seed":"#1b4fe0","derive":true},"appearance":{
            "light":{"accent-solid":"#1b4fe0","on-accent-solid":"#ffffff"},
            "dark":{"accent":"#5285ff","on-accent":"#0a1420","accent-solid":"#5285ff","on-accent-solid":"#0a1420"}}}"##,
    )
    .unwrap();
    let pack = complete(pack);
    let dark = Prefs {
        appearance: Appearance::Dark,
        ..Prefs::default()
    };
    let theme = resolve(&pack, &dark).unwrap();
    for (role, written) in [
        (Role::Accent, "#5285ff"),
        (Role::OnAccent, "#0a1420"),
        (Role::AccentSolid, "#5285ff"),
        (Role::OnAccentSolid, "#0a1420"),
    ] {
        assert_eq!(theme.color(role), hex(written), "{}", role.name());
    }
    let light = resolve(&pack, &Prefs::default()).unwrap();
    assert_eq!(light.color(Role::AccentSolid), hex("#1b4fe0"));
    // A user accent replaces the seed, and the whole family follows it.
    let picked = resolve(
        &pack,
        &Prefs {
            accent: Some(hex("#019954")),
            ..dark
        },
    )
    .unwrap();
    assert_ne!(picked.color(Role::AccentSolid), hex("#5285ff"));
    assert_ne!(picked.color(Role::Accent), hex("#5285ff"));
}

#[test]
fn a_written_solid_whose_text_does_not_read_is_caught_at_build() {
    let pack: Pack = serde_json::from_str(
        r##"{"name":"brand","version":1,"accent":{"seed":"#1b4fe0","derive":true},"appearance":{
            "dark":{"accent-solid":"#5285ff","on-accent-solid":"#ffffff"}}}"##,
    )
    .unwrap();
    let problems = check(&complete(pack));
    assert!(
        problems.iter().any(|problem| problem.appearance == "dark"
            && problem
                .message
                .starts_with("on-accent-solid on accent-solid is ")),
        "{problems:?}"
    );
}
