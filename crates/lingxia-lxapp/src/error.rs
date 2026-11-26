#[cfg(feature = "js-appservice")]
use rong::RongJSError;
#[cfg(feature = "js-appservice")]
use rong::error::{ErrorData, ErrorNumber};
use serde_json::Value;

/// A Rong host error at the boundary: a numeric code becomes its code and reason, and
/// `reason` and `detail` in its data move to their own fields.
pub(crate) fn host_error(
    code: String,
    message: String,
    data: Option<Value>,
) -> lingxia_error::Error {
    let mut data = data;
    let (mut reason, mut detail) = (None, None);
    if let Some(fields) = data.as_mut().and_then(Value::as_object_mut) {
        if let Some(Value::String(raised)) = fields.remove("reason") {
            reason = Some(raised);
        }
        if let Some(Value::String(text)) = fields.remove("detail") {
            detail = Some(text);
        }
    }
    let message = detail.unwrap_or(message);
    let mut error = match code.parse::<u32>() {
        Ok(number) => lingxia_error::Error::business(number, message),
        Err(_) => lingxia_error::Error::new(code, message),
    };
    if let Some(reason) = reason {
        error = error.with_reason(reason);
    }
    match data {
        Some(Value::Object(fields)) if fields.is_empty() => error,
        Some(data) => error.with_data(data),
        None => error,
    }
}

/// Why a Logic evaluation produced no value.
#[derive(Debug, Clone)]
pub enum EvalError {
    /// The script threw: a coded throw keeps its code, anything else is `E_INTERNAL`.
    Thrown(lingxia_error::Error),
    /// The Logic runtime could not run the script.
    Unavailable(lingxia_error::Error),
}

impl std::fmt::Display for EvalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Thrown(error) | Self::Unavailable(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for EvalError {}

impl From<EvalError> for lingxia_error::Error {
    fn from(error: EvalError) -> Self {
        match error {
            EvalError::Thrown(error) | EvalError::Unavailable(error) => error,
        }
    }
}

/// A Logic runtime error: a host error keeps its code, anything else is `E_INTERNAL`.
#[cfg(feature = "js-appservice")]
pub fn rong_error(error: RongJSError) -> lingxia_error::Error {
    if let Some(host) = error.as_host_error() {
        let data = host.data.as_ref().map(error_data_to_json);
        return host_error(host.code.to_string(), host.message.clone(), data);
    }
    lingxia_error::Error::internal(error.to_string())
}

#[cfg(feature = "js-appservice")]
fn error_data_to_json(data: &ErrorData) -> Value {
    match data {
        ErrorData::Null => Value::Null,
        ErrorData::Bool(v) => Value::Bool(*v),
        ErrorData::String(v) => Value::String(v.clone()),
        ErrorData::Number(n) => match n {
            ErrorNumber::I64(v) => Value::Number(serde_json::Number::from(*v)),
            ErrorNumber::U64(v) => Value::Number(serde_json::Number::from(*v)),
            ErrorNumber::F64(bits) => {
                let num = f64::from_bits(*bits);
                match serde_json::Number::from_f64(num) {
                    Some(value) => Value::Number(value),
                    None => Value::String(num.to_string()),
                }
            }
        },
        ErrorData::Array(items) => Value::Array(items.iter().map(error_data_to_json).collect()),
        ErrorData::Object(obj) => Value::Object(
            obj.iter()
                .map(|(k, v)| (k.clone(), error_data_to_json(v)))
                .collect(),
        ),
    }
}

const REQUIRES_RUNTIME_UPGRADE_CODE: u32 = 6002;

/// This host's runtime is below the package's `minRuntime`.
pub fn requires_runtime_upgrade(detail: impl Into<String>) -> lingxia_error::Error {
    lingxia_error::Error::business(REQUIRES_RUNTIME_UPGRADE_CODE, detail)
}

pub fn is_requires_runtime_upgrade(error: &lingxia_error::Error) -> bool {
    error.reason() == Some("hostUpgradeRequired")
}

#[cfg(test)]
mod tests {
    use super::{host_error, is_requires_runtime_upgrade, requires_runtime_upgrade};

    #[test]
    fn a_host_error_number_becomes_its_code_and_an_e_code_keeps_its_own() {
        let numeric = requires_runtime_upgrade("old");
        assert_eq!(numeric.code(), "E_NOT_SUPPORTED");
        assert!(is_requires_runtime_upgrade(&numeric));
        let coded = host_error(
            "E_INVALID_ARG".to_string(),
            "localized".to_string(),
            Some(serde_json::json!({
                "detail": "duplicate route",
                "reason": "tabAlreadyOpen",
                "target": "pages/home",
            })),
        );
        assert_eq!(coded.code(), "E_INVALID_ARG");
        assert_eq!(coded.message(), "duplicate route");
        assert_eq!(coded.reason(), Some("tabAlreadyOpen"));
        assert_eq!(
            coded.data(),
            Some(&serde_json::json!({ "target": "pages/home" }))
        );
    }
}
