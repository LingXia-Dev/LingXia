use rong::Source;
use serde::Deserialize;

const PREFIX: &str = "// LingXia UI module; ";
const ABI: u32 = crate::protocol::UI_ABI;

// Unknown fields are ignored: an older host must load a newer module, and a field it has to
// understand goes into `requiredFeatures`.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Manifest {
    pub(crate) ui_abi: u32,
    pub(crate) min_field: usize,
    pub(crate) required_features: Vec<String>,
    pub(crate) compiler: String,
    #[serde(default)]
    pub(crate) chunk_build_id: Option<String>,
    #[serde(default)]
    pub(crate) catalogue_chunk: Option<String>,
    #[serde(default)]
    pub(crate) catalogue_source: Option<String>,
    #[serde(default)]
    pub(crate) catalogue_languages: Vec<String>,
}

pub(crate) fn parse_manifest(source: &str) -> Result<Option<Manifest>, String> {
    let Some(header) = source
        .lines()
        .next()
        .and_then(|line| line.strip_prefix(PREFIX))
    else {
        return Ok(None); // Legacy modules predate capability manifests.
    };
    if header.starts_with("schema ") {
        return Ok(None);
    }
    if header.len() > 4096 {
        return Err("UI_COMPAT: manifest exceeds 4096 bytes".into());
    }
    let manifest: Manifest = serde_json::from_str(header)
        .map_err(|error| format!("UI_COMPAT: invalid manifest: {error}"))?;
    Ok(Some(manifest))
}

pub(crate) fn validate(source: &str) -> Result<(), String> {
    let Some(manifest) = parse_manifest(source)? else {
        return Ok(());
    };
    if manifest.ui_abi != ABI {
        return Err(format!(
            "UI_COMPAT: UI ABI {} required, host provides {ABI}",
            manifest.ui_abi
        ));
    }
    if manifest.min_field >= crate::protocol::FIELDS.len() {
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
    for feature in &manifest.required_features {
        if !super::features()
            .iter()
            .any(|known| known.name() == feature)
        {
            return Err(format!("UI_COMPAT: host lacks {feature}"));
        }
    }
    Ok(())
}

pub(crate) fn install(context: &super::JSContext) -> Result<(), String> {
    let features: Vec<&str> = super::features()
        .iter()
        .map(|feature| feature.name())
        .collect();
    let host = serde_json::json!({
        "uiAbi": ABI, "maxField": crate::protocol::FIELDS.len() - 1, "features": features,
        "version": env!("CARGO_PKG_VERSION"),
    });
    context.eval::<()>(Source::from_bytes(format!(
        "Object.defineProperty(globalThis,'__lingxiaViewHost',{{value:Object.freeze({{...{host},features:Object.freeze({})}})}});",
        serde_json::to_string(&features).unwrap()
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
        let bounded = validate(&source(1, 102, vec!["bounded-jobs"]));
        assert_eq!(
            bounded.is_ok(),
            super::super::supports(super::super::Feature::BoundedJobs)
        );
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

    #[test]
    fn a_manifest_field_this_host_does_not_know_still_loads() {
        let manifest = serde_json::json!({"uiAbi":ABI,"minField":0,"requiredFeatures":[],"compiler":"9.0.0","laterField":{"any":1}});
        let module = format!(
            "{PREFIX}{manifest}\n{}",
            crate::session::tests::source("function render(){return h('text',{},'hi')}")
        );
        let mut page = crate::ViewSession::new(&module).unwrap();
        assert!(page.layout(200, 100, 1.0).is_ok());
    }
}
