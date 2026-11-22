use rong::{FromJSObject, JSObject};
use serde::{Deserialize, Serialize};

#[derive(FromJSObject, Clone, Default)]
#[ts_skip]
pub(super) struct JSPickMediaOptions {
    pub(super) sources: Option<Vec<String>>,
    #[js_name = "limit"]
    pub(super) count: Option<u32>,
    pub(super) types: Option<Vec<String>>,
    pub(super) camera: Option<JSObject>,
    pub(super) signal: Option<JSObject>,
}

#[derive(FromJSObject, Clone)]
#[ts_skip]
pub(super) struct JSCaptureOptions {
    #[js_name = "type"]
    pub(super) kind: String,
    pub(super) facing: Option<String>,
    #[js_name = "maxDurationMs"]
    pub(super) max_duration_ms: Option<f64>,
    pub(super) signal: Option<JSObject>,
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
    /// The item's own file name, when the platform knows it.
    #[serde(rename = "fileName", default)]
    pub(super) file_name: Option<String>,
}

fn default_kind() -> String {
    "image".to_string()
}

fn default_is_original() -> bool {
    true
}
