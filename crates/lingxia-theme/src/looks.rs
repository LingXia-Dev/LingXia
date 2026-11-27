//! A pack resolved for its four looks, for host chrome and WebView pages that read roles
//! without a View renderer.

use crate::{Appearance, Pack, Prefs, ROLE_COUNT, Rgba, Role};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// One theme resolved for its four looks.
#[derive(Debug)]
pub struct ThemeLooks {
    light: [Rgba; ROLE_COUNT],
    dark: [Rgba; ROLE_COUNT],
    high_contrast_light: [Rgba; ROLE_COUNT],
    high_contrast_dark: [Rgba; ROLE_COUNT],
}

impl ThemeLooks {
    fn resolve(pack: &Pack) -> Option<Self> {
        let look = |appearance, high_contrast| {
            let prefs = Prefs {
                appearance,
                high_contrast,
                ..Prefs::default()
            };
            crate::resolve(pack, &prefs).ok().map(|theme| theme.palette)
        };
        Some(Self {
            light: look(Appearance::Light, false)?,
            dark: look(Appearance::Dark, false)?,
            high_contrast_light: look(Appearance::Light, true)?,
            high_contrast_dark: look(Appearance::Dark, true)?,
        })
    }

    pub fn palette(&self, dark: bool, high_contrast: bool) -> &[Rgba; ROLE_COUNT] {
        match (dark, high_contrast) {
            (false, false) => &self.light,
            (true, false) => &self.dark,
            (false, true) => &self.high_contrast_light,
            (true, true) => &self.high_contrast_dark,
        }
    }

    pub fn role(&self, role: Role, dark: bool, high_contrast: bool) -> Rgba {
        self.palette(dark, high_contrast)[role as usize]
    }
}

/// The looks of a pack's JSON, cached by its text; the default pack for `None` or a pack that
/// does not resolve.
pub fn looks(pack: Option<&str>) -> Arc<ThemeLooks> {
    static CACHE: Mutex<Option<HashMap<Option<String>, Arc<ThemeLooks>>>> = Mutex::new(None);
    let key = pack.map(str::to_owned);
    let mut cache = CACHE.lock().unwrap_or_else(|error| error.into_inner());
    let cache = cache.get_or_insert_with(HashMap::new);
    if let Some(looks) = cache.get(&key) {
        return looks.clone();
    }
    let looks = pack
        .and_then(|text| serde_json::from_str::<Pack>(text).ok())
        .and_then(|pack| ThemeLooks::resolve(&pack))
        .or_else(|| ThemeLooks::resolve(&crate::default_pack()))
        .map(Arc::new)
        .expect("the default theme resolves");
    cache.insert(key, looks.clone());
    looks
}
