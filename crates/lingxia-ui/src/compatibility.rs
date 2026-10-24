use rong::Source;
use serde::Deserialize;

const PREFIX: &str = "// LingXia UI module; ";
const ABI: u32 = 1;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Manifest {
    ui_abi: u32,
    min_field: usize,
    required_features: Vec<String>,
    compiler: String,
}

fn features() -> &'static [&'static str] {
    if cfg!(any(
        target_os = "android",
        target_os = "windows",
        all(target_os = "linux", not(target_env = "ohos"))
    )) {
        &["candidate-transactions", "bounded-jobs"]
    } else {
        &["candidate-transactions"]
    }
}

pub(crate) fn validate(source: &str) -> Result<(), String> {
    let Some(header) = source
        .lines()
        .next()
        .and_then(|line| line.strip_prefix(PREFIX))
    else {
        return Ok(()); // Legacy modules predate capability manifests.
    };
    if header.starts_with("schema ") {
        return Ok(());
    }
    if header.len() > 4096 {
        return Err("UI_COMPAT: manifest exceeds 4096 bytes".into());
    }
    let manifest: Manifest = serde_json::from_str(header)
        .map_err(|error| format!("UI_COMPAT: invalid manifest: {error}"))?;
    if manifest.ui_abi != ABI {
        return Err(format!(
            "UI_COMPAT: UI ABI {} required, host provides {ABI}",
            manifest.ui_abi
        ));
    }
    if manifest.min_field >= crate::schema::FIELDS.len() {
        return Err(format!(
            "UI_COMPAT: wire field {} is unavailable",
            manifest.min_field
        ));
    }
    if manifest.compiler.is_empty()
        || manifest.compiler.len() > 128
        || manifest.required_features.len() > 32
    {
        return Err("UI_COMPAT: invalid compiler or feature inventory".into());
    }
    for feature in manifest.required_features {
        if !features().contains(&feature.as_str()) {
            return Err(format!("UI_COMPAT: host lacks {feature}"));
        }
    }
    Ok(())
}

pub(crate) fn install(context: &rong::JSContext) -> Result<(), String> {
    let host = serde_json::json!({
        "uiAbi": ABI, "maxField": crate::schema::FIELDS.len() - 1, "features": features(),
    });
    context.eval::<()>(Source::from_bytes(format!(
        "Object.defineProperty(globalThis,'__lingxiaUiHost',{{value:Object.freeze({{...{host},features:Object.freeze({})}})}});",
        serde_json::to_string(features()).unwrap()
    ))).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compatibility_rejects_unknown_requirements_before_executing_code() {
        let source = |abi, field, features| {
            format!(
                "{PREFIX}{}\nthrow new Error('must not execute');",
                serde_json::json!({"uiAbi":abi,"minField":field,"requiredFeatures":features,"compiler":"0.25.1"})
            )
        };
        assert!(validate(&source(1, 102, vec!["candidate-transactions"])).is_ok());
        for invalid in [
            source(2, 102, vec![]),
            source(1, 999, vec![]),
            source(1, 102, vec!["unknown"]),
        ] {
            assert!(
                crate::ViewSession::new(&invalid)
                    .err()
                    .unwrap()
                    .starts_with("UI_COMPAT:")
            );
        }
        assert!(validate(&format!("{PREFIX}{{bad json")).is_err());
        assert!(validate("// LingXia UI module; schema 0.25\n").is_ok());
    }
}
