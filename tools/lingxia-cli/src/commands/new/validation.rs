use anyhow::{Result, anyhow};

/// Validate project name
pub fn validate_project_name(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(anyhow!("Project name cannot be empty"));
    }

    if !name
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
    {
        return Err(anyhow!(
            "Project name must be one word (no spaces) and can only contain alphanumeric characters, underscores, and hyphens"
        ));
    }

    Ok(())
}

/// Validate product name (single line; spaces allowed).
pub fn validate_product_name(name: &str) -> Result<()> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("Product name cannot be empty"));
    }
    if trimmed.contains('\n') || trimmed.contains('\r') {
        return Err(anyhow!("Product name must be a single line"));
    }
    Ok(())
}

/// Validate an lxapp `appId`. Allows dotted, namespaced ids
/// (e.g. `lingxia.lxapp.demo`) as well as a bare single segment. Each
/// dot-separated segment must be non-empty and contain only alphanumeric
/// characters, underscores, or hyphens.
pub fn validate_lxapp_id(app_id: &str) -> Result<()> {
    let app_id = app_id.trim();
    if app_id.is_empty() {
        return Err(anyhow!("LxApp ID cannot be empty"));
    }
    for segment in app_id.split('.') {
        if segment.is_empty() {
            return Err(anyhow!(
                "LxApp ID segments cannot be empty (no leading, trailing, or doubled dots)"
            ));
        }
        if !segment
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        {
            return Err(anyhow!(
                "LxApp ID can only contain alphanumeric characters, underscores, hyphens, and dots"
            ));
        }
    }
    Ok(())
}

/// Validate package ID format
pub fn validate_package_id(package_id: &str) -> Result<()> {
    crate::host_identity::validate_package_id(package_id)
}

/// Convert a project name to a SwiftPM-safe target name.
///
/// Keeps ASCII alphanumerics and underscores, replaces other characters with
/// underscores, and prefixes with `_` if it starts with a digit.
pub fn swift_target_name_from_project_name(project_name: &str) -> String {
    let mut out = String::with_capacity(project_name.len());
    let mut last_was_underscore = false;

    for ch in project_name.chars() {
        let mapped = if ch.is_ascii_alphanumeric() || ch == '_' {
            ch
        } else {
            '_'
        };

        if mapped == '_' {
            if !last_was_underscore {
                out.push('_');
                last_was_underscore = true;
            }
        } else {
            out.push(mapped);
            last_was_underscore = false;
        }
    }

    if out.is_empty() {
        out.push_str("App");
    }

    if out.as_bytes().first().is_some_and(u8::is_ascii_digit) {
        out.insert(0, '_');
    }

    out
}

/// Turn a project name into one segment of a package id.
///
/// A project name may carry `-` (`h-app`); a package id may not, because
/// Android's is a Java package name. Pasting the name in unchanged produced a
/// default id the CLI then refused — a new project that could not build until
/// its id was edited by hand.
pub fn package_segment_from_project_name(project_name: &str) -> String {
    let mut out = String::with_capacity(project_name.len());
    let mut last_was_underscore = false;
    for ch in project_name.chars() {
        let mapped = if ch.is_alphanumeric() { ch } else { '_' };
        if mapped == '_' {
            if !last_was_underscore && !out.is_empty() {
                out.push('_');
                last_was_underscore = true;
            }
        } else {
            out.extend(mapped.to_lowercase());
            last_was_underscore = false;
        }
    }
    while out.ends_with('_') {
        out.pop();
    }
    if out.is_empty() {
        out.push_str("app");
    }
    // A Java package segment cannot start with a digit.
    if out.as_bytes().first().is_some_and(u8::is_ascii_digit) {
        out.insert(0, '_');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dashed_project_name_still_derives_a_legal_default_id() {
        assert_eq!(package_segment_from_project_name("h-app"), "h_app");
        assert_eq!(package_segment_from_project_name("My App"), "my_app");
        assert_eq!(package_segment_from_project_name("2fast"), "_2fast");
        assert_eq!(package_segment_from_project_name("---"), "app");
        assert_eq!(package_segment_from_project_name("a--b"), "a_b");
        // Whatever it derives has to pass the id rule it feeds.
        for name in ["h-app", "My App", "2fast", "---", "a--b"] {
            let id = format!("app.lingxia.{}", package_segment_from_project_name(name));
            assert!(validate_package_id(&id).is_ok(), "{id}");
        }
    }

    #[test]
    fn validate_lxapp_id_accepts_dotted_and_bare() {
        assert!(validate_lxapp_id("lingxia.lxapp.demo").is_ok());
        assert!(validate_lxapp_id("demo").is_ok());
        assert!(validate_lxapp_id("home-lxapp").is_ok());
        assert!(validate_lxapp_id("a.b_c.d-e").is_ok());
    }

    #[test]
    fn validate_lxapp_id_rejects_bad_segments() {
        assert!(validate_lxapp_id("").is_err());
        assert!(validate_lxapp_id("lingxia..demo").is_err());
        assert!(validate_lxapp_id(".demo").is_err());
        assert!(validate_lxapp_id("demo.").is_err());
        assert!(validate_lxapp_id("has space").is_err());
        assert!(validate_lxapp_id("bad/slash").is_err());
    }
}
