use rong::{FromJSObj, IntoJSObj};
use serde::{Deserialize, Serialize};

#[derive(FromJSObj, Clone)]
pub(super) struct JSChooseMediaOptions {
    #[rename = "count"]
    pub(super) count: Option<u32>,
    #[rename = "mediaType"]
    pub(super) media_type: Option<Vec<String>>,
    #[rename = "sourceType"]
    pub(super) source_type: Option<Vec<String>>,
    pub(super) camera: Option<String>,
    #[rename = "maxDuration"]
    pub(super) max_duration: Option<f64>,
}

#[derive(Debug, Clone, IntoJSObj)]
pub(super) struct ChosenMediaEntry {
    #[rename = "tempFilePath"]
    pub(super) path: String,
    #[rename = "fileType"]
    pub(super) kind: String,
    #[rename = "isOriginal"]
    pub(super) is_original: bool,
}

#[derive(Deserialize, Serialize, Hash, Clone)]
pub(super) struct MediaKey {
    pub(super) uri: String,
    #[serde(rename = "fileType", default = "default_kind")]
    pub(super) kind: String,
    #[serde(rename = "isOriginal", default = "default_is_original")]
    pub(super) is_original: bool,
}

fn default_kind() -> String {
    "image".to_string()
}

fn default_is_original() -> bool {
    true
}
