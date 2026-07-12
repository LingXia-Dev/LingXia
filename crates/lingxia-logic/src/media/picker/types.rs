use rong::FromJSObject;
use serde::{Deserialize, Serialize};

#[derive(FromJSObject, Clone)]
pub(super) struct JSChooseMediaOptions {
    #[js_name = "count"]
    pub(super) count: Option<u32>,
    #[js_name = "mediaType"]
    pub(super) media_type: Option<Vec<String>>,
    #[js_name = "sourceType"]
    pub(super) source_type: Option<Vec<String>>,
    pub(super) camera: Option<String>,
    #[js_name = "maxDuration"]
    pub(super) max_duration: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct ChosenMediaEntry {
    #[serde(rename = "tempFilePath")]
    pub(super) path: String,
    #[serde(rename = "fileType")]
    pub(super) kind: String,
    #[serde(rename = "isOriginal")]
    pub(super) is_original: bool,
}

#[derive(Deserialize, Serialize, Hash, Clone)]
pub(super) struct MediaKey {
    pub(super) uri: String,
    #[serde(rename = "fileType", default = "default_kind")]
    pub(super) kind: String,
    #[serde(rename = "isOriginal", default = "default_is_original")]
    pub(super) is_original: bool,
    #[serde(rename = "fileExt", default)]
    pub(super) file_ext: Option<String>,
}

fn default_kind() -> String {
    "image".to_string()
}

fn default_is_original() -> bool {
    true
}
