//! The lxapp's own messages for text shown natively: `lx.app.t`, toasts and dialogs Logic opens.
//! The build ships each `i18n/<tag>.json` beside the compiled page catalogues.

use super::{LxApp, display_language};
use crate::error::LxAppError;
use lingxia_i18n::Catalogue;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

type Key = (String, String, String);
static CATALOGUES: Mutex<Option<HashMap<Key, Arc<Catalogue>>>> = Mutex::new(None);

impl LxApp {
    /// `key` in the lxapp's language, else in its source language (the first it declares).
    pub fn message(&self, key: &str, args: &serde_json::Value) -> Result<String, LxAppError> {
        let source = self.config().languages.first().cloned().ok_or_else(|| {
            LxAppError::UnsupportedOperation("lxapp.json declares no languages".into())
        })?;
        let tag = self.content_language(&display_language());
        for tag in [tag, source] {
            if let Some(entry) = self
                .catalogue(&tag)?
                .and_then(|catalogue| catalogue.entries.get(key).cloned())
            {
                return Ok(lingxia_i18n::format::format(&entry.message, &tag, args));
            }
        }
        Err(LxAppError::ResourceNotFound(format!("no message '{key}'")))
    }

    /// Binds manifest text naming a key to the key's text in every declared language.
    pub(crate) fn bind_text(&self, text: &mut super::localized_text::LocalizedText) {
        let super::localized_text::LocalizedText::Message(key) = text else {
            return;
        };
        let key = key.clone();
        let languages = self.config().languages.clone();
        text.bind(&languages, |tag| {
            let catalogue = self.catalogue(tag).ok().flatten()?;
            let entry = catalogue.entries.get(&key)?;
            Some(lingxia_i18n::format::format(
                &entry.message,
                tag,
                &serde_json::Value::Null,
            ))
        });
    }

    /// Drops this lxapp's cached catalogues: a dev reload rewrites them under the same version.
    pub(crate) fn forget_catalogues(&self) {
        let mut cache = CATALOGUES.lock().unwrap_or_else(|error| error.into_inner());
        if let Some(cache) = cache.as_mut() {
            cache.retain(|(appid, _, _), _| *appid != self.appid);
        }
    }

    fn catalogue(&self, tag: &str) -> Result<Option<Arc<Catalogue>>, LxAppError> {
        let key = (
            self.appid.clone(),
            self.get_lxapp_info().version,
            tag.to_string(),
        );
        let mut cache = CATALOGUES.lock().unwrap_or_else(|error| error.into_inner());
        if let Some(found) = cache.get_or_insert_with(HashMap::new).get(&key) {
            return Ok(Some(found.clone()));
        }
        let Ok(bytes) = self.read_bytes(&format!("i18n/{tag}.json")) else {
            return Ok(None);
        };
        let text = String::from_utf8(bytes)
            .map_err(|_| LxAppError::InvalidJsonFile(format!("i18n/{tag}.json is not UTF-8")))?;
        let catalogue = Catalogue::parse(&text, false).map_err(|problems| {
            LxAppError::InvalidJsonFile(format!(
                "i18n/{tag}.json: {}",
                problems
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("; ")
            ))
        })?;
        let catalogue = Arc::new(catalogue);
        cache
            .get_or_insert_with(HashMap::new)
            .insert(key, catalogue.clone());
        Ok(Some(catalogue))
    }
}
