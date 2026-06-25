use anyhow::{Context, Result, anyhow};
use include_dir::{Dir, DirEntry, include_dir};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use tempfile::TempDir;

static EMBEDDED_TEMPLATES: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/templates");
static EXTRACTED_TEMPLATES: OnceLock<ExtractedTemplates> = OnceLock::new();

struct ExtractedTemplates {
    _temp_dir: TempDir,
    path: PathBuf,
}

pub(super) fn locate_templates_dir() -> Result<PathBuf> {
    if let Some(extracted) = EXTRACTED_TEMPLATES.get() {
        return Ok(extracted.path.clone());
    }

    let extracted = materialize_embedded_templates()?;
    match EXTRACTED_TEMPLATES.set(extracted) {
        Ok(()) => Ok(EXTRACTED_TEMPLATES
            .get()
            .expect("embedded templates just initialized")
            .path
            .clone()),
        Err(_) => Ok(EXTRACTED_TEMPLATES
            .get()
            .expect("embedded templates initialized by another thread")
            .path
            .clone()),
    }
}

fn materialize_embedded_templates() -> Result<ExtractedTemplates> {
    let temp_dir = tempfile::Builder::new()
        .prefix("lingxia-templates-")
        .tempdir()
        .context("Failed to create temporary embedded templates directory")?;
    write_embedded_dir(&EMBEDDED_TEMPLATES, temp_dir.path())?;
    let path = temp_dir.path().to_path_buf();
    Ok(ExtractedTemplates {
        _temp_dir: temp_dir,
        path,
    })
}

fn write_embedded_dir(dir: &Dir<'_>, output_dir: &Path) -> Result<()> {
    fs::create_dir_all(output_dir)?;

    for entry in dir.entries() {
        match entry {
            DirEntry::Dir(child_dir) => {
                let child_name = child_dir.path().file_name().ok_or_else(|| {
                    anyhow!(
                        "Invalid embedded template directory {}",
                        child_dir.path().display()
                    )
                })?;
                write_embedded_dir(child_dir, &output_dir.join(child_name))?;
            }
            DirEntry::File(file) => write_embedded_file(file, output_dir)?,
        }
    }

    Ok(())
}

fn write_embedded_file(file: &include_dir::File<'_>, output_dir: &Path) -> Result<()> {
    let file_name = file.path().file_name().ok_or_else(|| {
        anyhow!(
            "Invalid embedded template file path {}",
            file.path().display()
        )
    })?;
    let target_path = output_dir.join(file_name);
    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&target_path, file.contents()).with_context(|| {
        format!(
            "Failed to write embedded template {}",
            target_path.display()
        )
    })?;
    set_template_permissions(&target_path)?;
    Ok(())
}

fn set_template_permissions(_path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let file_name = _path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        if matches!(file_name, "gradlew") {
            let mut perms = fs::metadata(_path)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(_path, perms)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn materializes_embedded_templates() {
        let path = locate_templates_dir().unwrap();
        assert!(path.join("AppIcon.png").exists());
        assert!(path.join("lxapp-create").exists());
        assert!(path.join("android").exists());
    }

    #[test]
    fn locate_templates_dir_is_reusable() {
        let path1 = locate_templates_dir().unwrap();
        let path2 = locate_templates_dir().unwrap();
        assert_eq!(path1, path2);
    }
}
