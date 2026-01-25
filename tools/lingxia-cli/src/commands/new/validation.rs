use anyhow::{anyhow, Result};

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
            "Project name can only contain alphanumeric characters, underscores, and hyphens"
        ));
    }

    Ok(())
}

/// Validate package ID format
pub fn validate_package_id(package_id: &str) -> Result<()> {
    if package_id.is_empty() {
        return Err(anyhow!("Package ID cannot be empty"));
    }

    let parts: Vec<&str> = package_id.split('.').collect();
    if parts.len() < 2 {
        return Err(anyhow!(
            "Package ID must have at least 2 parts (e.g., com.example)"
        ));
    }

    for part in parts {
        if part.is_empty() {
            return Err(anyhow!("Package ID parts cannot be empty"));
        }
        if !part.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return Err(anyhow!(
                "Package ID can only contain alphanumeric characters and underscores"
            ));
        }
    }

    Ok(())
}
