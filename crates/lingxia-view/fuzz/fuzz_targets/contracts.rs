//! JSON contracts with strict decoders: theme packs, palette pushes and the environment.
#![no_main]

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(data) else {
        return;
    };
    let _ = lingxia_view::protocol::check_environment(&value);
    for dark in [false, true] {
        let _ = lingxia_view::theme::Palette::decode(&value, dark);
    }
    if let Ok(pack) = serde_json::from_value::<lingxia_theme::Pack>(value) {
        let pack = lingxia_theme::complete(pack);
        let _ = lingxia_theme::check(&pack);
        for appearance in [
            lingxia_theme::Appearance::Light,
            lingxia_theme::Appearance::Dark,
        ] {
            for high_contrast in [false, true] {
                let prefs = lingxia_theme::Prefs {
                    appearance,
                    high_contrast,
                    ..Default::default()
                };
                let _ = lingxia_theme::resolve(&pack, &prefs);
            }
        }
    }
});
