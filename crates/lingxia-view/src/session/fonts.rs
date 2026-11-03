//! Faces a page's theme declared with `@font-face`: the host fetches each file by its lxapp
//! path and registers it in this page's font collection only.
use super::{Source, ViewSession};

/// One declared face; the file's own metadata picks it by weight and style.
#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
pub struct FontFace {
    pub family: String,
    pub src: String,
}

impl ViewSession {
    /// The faces the page module declared, in declaration order.
    pub fn declared_fonts(&self) -> Result<Vec<FontFace>, String> {
        let json: String = self.execute(|| {
            self.context
                .eval(Source::from_bytes(
                    "typeof __lingxiaFonts === 'function' ? __lingxiaFonts() : '[]'",
                ))
                .map_err(|e| e.to_string())
        })?;
        serde_json::from_str(&json).map_err(|e| e.to_string())
    }

    /// Register a loaded face file under its declared name; text shaped since re-lays out.
    pub fn register_font(&mut self, family: &str, bytes: Vec<u8>) -> Result<(), String> {
        let collection = &mut self.renderer.fonts.collection;
        let registered = collection.register_fonts(parley::fontique::Blob::from(bytes), None);
        if registered.is_empty() {
            return Err("not a TrueType or OpenType font".into());
        }
        let names = registered
            .iter()
            .filter_map(|(id, _)| collection.family_name(*id).map(str::to_owned))
            .collect::<Vec<_>>();
        let faces = self
            .renderer
            .font_faces
            .entry(family.to_owned())
            .or_default();
        for name in names {
            if !faces.contains(&name) {
                faces.push(name);
            }
        }
        self.renderer.clear_text_cache();
        self.retained = None;
        crate::layout::list_layout::invalidate_measurements(&mut self.lists);
        Ok(())
    }
}
