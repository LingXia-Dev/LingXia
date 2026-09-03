use crate::lxapp::security::NetworkSecurity;
use serde_json::Value;

/// Validate every URL a video node or video command may load.
pub fn validate_media_urls(
    urls: &[String],
    trusted_domains: &[String],
    dev_session: bool,
) -> Result<(), String> {
    let mut security = NetworkSecurity::new();
    security.set_domains(trusted_domains);
    for url in urls {
        let host = media_url_host(url)?;
        if let Some(host) = host
            && !security.is_domain_allowed_in(&host, dev_session)
        {
            return Err(format!(
                "media URL is not in security.network.trustedDomains: {url}"
            ));
        }
    }
    Ok(())
}

pub fn media_urls_from_props(props: &Value) -> Vec<String> {
    let mut urls = Vec::new();
    push_string(&mut urls, props.get("src"));
    push_string(&mut urls, props.get("poster"));
    if let Some(watermark) = props.get("watermark") {
        push_string(
            &mut urls,
            watermark
                .get("resource")
                .and_then(|resource| resource.get("url").or_else(|| resource.get("src"))),
        );
        push_string(&mut urls, watermark.get("url"));
    }
    if let Some(Value::Array(qualities)) = props.get("qualities") {
        for item in qualities {
            push_string(&mut urls, item.get("url").or_else(|| item.get("src")));
        }
    }
    urls
}

pub fn media_urls_from_command_options(options: &Value) -> Vec<String> {
    let mut urls = Vec::new();
    for key in ["url", "src", "uri"] {
        push_string(&mut urls, options.get(key));
    }
    urls
}

fn push_string(out: &mut Vec<String>, value: Option<&Value>) {
    if let Some(Value::String(text)) = value
        && !text.is_empty()
    {
        out.push(text.clone());
    }
}

fn media_url_host(url: &str) -> Result<Option<String>, String> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("lx:")
        || lower.starts_with("lingxia:")
        || lower.starts_with("data:")
        || lower.starts_with("blob:")
    {
        return Ok(None);
    }
    let rest = if let Some(rest) = trimmed.strip_prefix("//") {
        rest
    } else if let Some(scheme_end) = trimmed.find("://") {
        let scheme = &lower[..scheme_end];
        if scheme != "http" && scheme != "https" {
            return Err(format!("unsupported media URL scheme: {url}"));
        }
        &trimmed[scheme_end + 3..]
    } else {
        if trimmed
            .find(':')
            .is_some_and(|colon| !trimmed[..colon].contains(['/', '?', '#']))
        {
            return Err(format!("unsupported media URL scheme: {url}"));
        }
        return Ok(None);
    };
    let host_port = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    let host = host_port
        .rsplit_once('@')
        .map(|(_, host)| host)
        .unwrap_or(host_port);
    let host = if let Some(ipv6) = host.strip_prefix('[') {
        ipv6.split_once(']')
            .map(|(address, _)| address)
            .unwrap_or("")
    } else {
        host.split(':').next().unwrap_or(host)
    };
    if host.is_empty() {
        Err(format!("media URL has no host: {url}"))
    } else {
        Ok(Some(host.to_ascii_lowercase()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_relative_and_lx_schemes() {
        validate_media_urls(
            &[
                "./clip.mp4".into(),
                "lx://assets/intro.mp4".into(),
                "data:image/png;base64,abc".into(),
            ],
            &[],
            false,
        )
        .unwrap();
    }

    #[test]
    fn trusted_https_host_is_allowed() {
        validate_media_urls(
            &["https://cdn.example.com/a.mp4".into()],
            &["cdn.example.com".into()],
            false,
        )
        .unwrap();
    }

    #[test]
    fn untrusted_https_host_is_rejected() {
        let err = validate_media_urls(
            &["https://evil.example/a.mp4".into()],
            &["cdn.example.com".into()],
            false,
        )
        .unwrap_err();
        assert!(err.contains("trustedDomains"), "{err}");
        assert!(err.contains("evil.example"), "{err}");
    }

    #[test]
    fn ipv6_and_unsupported_schemes_fail_closed() {
        let err =
            validate_media_urls(&["https://[2001:db8::1]/a.mp4".into()], &[], false).unwrap_err();
        assert!(err.contains("2001:db8::1"), "{err}");
        let err = validate_media_urls(&["javascript:alert(1)".into()], &[], false).unwrap_err();
        assert!(err.contains("unsupported media URL scheme"), "{err}");
        let err = validate_media_urls(&["//evil.example/a.mp4".into()], &[], false).unwrap_err();
        assert!(err.contains("evil.example"), "{err}");
    }

    #[test]
    fn collects_src_poster_watermark_and_quality_urls() {
        let props = serde_json::json!({
            "src": "https://cdn.example.com/a.mp4",
            "poster": "https://cdn.example.com/p.jpg",
            "watermark": { "resource": { "url": "https://cdn.example.com/mark.png" } },
            "qualities": [{ "id": "hd", "label": "HD", "url": "https://cdn.example.com/hd.mp4" }]
        });
        let urls = media_urls_from_props(&props);
        assert_eq!(urls.len(), 4);
    }
}
