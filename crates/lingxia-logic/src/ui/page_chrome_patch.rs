use crate::i18n::js_invalid_parameter_error;
use rong::{JSObject, JSResult};
use serde::de::DeserializeOwned;

pub(super) fn parse_patch<T>(object: &JSObject, path: &str) -> JSResult<T>
where
    T: DeserializeOwned,
{
    let json = object.to_json_string().map_err(|_| {
        js_invalid_parameter_error(format!("{path}: expected a JSON-compatible object"))
    })?;
    serde_json::from_str(&json)
        .map_err(|error| js_invalid_parameter_error(format!("{path}: {error}")))
}
