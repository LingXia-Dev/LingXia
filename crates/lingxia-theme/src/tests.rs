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
            assert!(
                actual + 1e-9 >= boosted(ratio),
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
