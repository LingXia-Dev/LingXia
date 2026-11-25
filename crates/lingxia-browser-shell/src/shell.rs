//! Ordered, mixed sidebar Pins and cached site icons, exposed to trusted control surfaces.
use crate::host::HostResult;
use lingxia_platform::traits::app_runtime::AppRuntime;
use lingxia_shell::{MAX_SHELL_PINS, ShellPin};
use lxapp::{LxApp, LxAppError};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// A full sidebar, as a code a caller can branch on.
///
/// Every other rejection here is a programming error the caller cannot act on,
/// but this one is ordinary: the person pinned one thing too many. A caller
/// that must say so in its own language needs to recognise it without matching
/// this crate's English prose, so it travels as a code with the limit in
/// `data` rather than as a sentence.
pub const SHELL_PIN_LIMIT_CODE: &str = "SHELL_PIN_LIMIT";

/// Map a Pin mutation failure, giving the full-sidebar case its own code.
pub(crate) fn map_pin_error(error: lingxia_shell::ShellError) -> LxAppError {
    match error {
        lingxia_shell::ShellError::LimitReached { max } => LxAppError::RongJSHost {
            code: SHELL_PIN_LIMIT_CODE.to_string(),
            message: format!("the sidebar holds {max} Pins"),
            data: Some(serde_json::json!({ "max": max })),
        },
        lingxia_shell::ShellError::InvalidState(_) => {
            LxAppError::InvalidParameter(error.to_string())
        }
        other => LxAppError::Runtime(other.to_string()),
    }
}

#[derive(serde::Deserialize)]
struct ReorderPinsInput {
    items: Vec<ShellPin>,
}

/// The sidebar's Pins and how many it holds.
///
/// The budget is shared with pinned lxapps, so a caller cannot derive the room
/// it has left from its own rows, and hard-coding the total would copy a
/// constant that lives here.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ShellPins {
    items: Vec<ShellPin>,
    max: usize,
}

fn shell_pins() -> HostResult<ShellPins> {
    Ok(ShellPins {
        items: lingxia_shell::pins().map_err(|error| LxAppError::Runtime(error.to_string()))?,
        max: MAX_SHELL_PINS,
    })
}

#[lingxia::framework_native("shell.pins", audience = "control-app-or-surface")]
fn pins(_app: Arc<LxApp>) -> HostResult<ShellPins> {
    shell_pins()
}

#[lingxia::framework_native("shell.reorderPins", audience = "control-app-or-surface")]
fn reorder_pins(_app: Arc<LxApp>, input: ReorderPinsInput) -> HostResult<ShellPins> {
    lingxia_shell::reorder_pins(input.items).map_err(map_pin_error)?;
    shell_pins()
}

/// One call covers a screenful of rows; more is a caller bug, not a bigger page.
const MAX_SITE_ICON_URLS: usize = 400;
const SITE_ICON_DIR: &str = "site-icons";

#[derive(serde::Deserialize)]
struct SiteIconsInput {
    urls: Vec<String>,
}

/// Each asked-for page URL whose site has a cached icon the View can draw, as an `lx://usercache` URI.
#[derive(serde::Serialize)]
struct SiteIcons {
    icons: HashMap<String, String>,
}

/// Copies a cached PNG or JPEG icon into `dir`, refreshing an older copy; `<image>` decodes no other format.
fn copy_icon(source: &Path, dir: &Path) -> Option<PathBuf> {
    let extension = source.extension()?.to_str()?.to_ascii_lowercase();
    if !matches!(extension.as_str(), "png" | "jpg" | "jpeg") {
        return None;
    }
    let target = dir.join(source.file_name()?);
    let modified = |path: &Path| path.metadata().and_then(|meta| meta.modified()).ok();
    if modified(&target).is_none() || modified(&target) < modified(source) {
        std::fs::create_dir_all(dir).ok()?;
        std::fs::copy(source, &target).ok()?;
    }
    Some(target)
}

fn site_icons_for(
    urls: Vec<String>,
    cached: impl Fn(&str) -> Option<PathBuf>,
    publish: impl Fn(&Path) -> Option<String>,
) -> HostResult<HashMap<String, String>> {
    if urls.len() > MAX_SITE_ICON_URLS {
        return Err(LxAppError::InvalidParameter(format!(
            "shell.siteIcons takes at most {MAX_SITE_ICON_URLS} urls"
        )));
    }
    Ok(urls
        .into_iter()
        .filter_map(|url| {
            let icon = publish(&cached(&url)?)?;
            Some((url, icon))
        })
        .collect())
}

/// Reads the icon cache only: icons arrive there from the tab's WebView, which already goes through the
/// browser's proxy, so this route never opens a connection of its own.
#[lingxia::framework_native("shell.siteIcons", audience = "control-surface-only")]
fn site_icons(app: Arc<LxApp>, input: SiteIconsInput) -> HostResult<SiteIcons> {
    let Some(cache_root) = lxapp::get_platform().map(|runtime| runtime.app_cache_dir()) else {
        return Ok(SiteIcons {
            icons: HashMap::new(),
        });
    };
    let dir = app.user_cache_dir.join(SITE_ICON_DIR);
    let icons = site_icons_for(
        input.urls,
        |url| lingxia_service::favicon::cached(&cache_root, url),
        |source| {
            app.to_uri(&copy_icon(source, &dir)?)
                .map(|uri| uri.into_string())
        },
    )?;
    Ok(SiteIcons { icons })
}

pub(crate) fn register() {
    lxapp::host::register_host_entry(pins_host());
    lxapp::host::register_host_entry(reorder_pins_host());
    lxapp::host::register_host_entry(site_icons_host());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_full_sidebar_is_a_code_and_a_number_not_a_sentence() {
        match map_pin_error(lingxia_shell::ShellError::LimitReached { max: 8 }) {
            LxAppError::RongJSHost { code, data, .. } => {
                assert_eq!(code, SHELL_PIN_LIMIT_CODE);
                assert_eq!(data.and_then(|data| data["max"].as_u64()), Some(8));
            }
            other => panic!("expected a coded host error, got {other:?}"),
        }
        // Everything else stays an ordinary failure the caller cannot act on.
        assert!(matches!(
            map_pin_error(lingxia_shell::ShellError::InvalidState("x".into())),
            LxAppError::InvalidParameter(_)
        ));
    }

    #[test]
    fn site_icons_refuse_more_urls_than_a_screenful() {
        let urls = vec!["https://a.example/".to_string(); MAX_SITE_ICON_URLS + 1];
        let result = site_icons_for(urls, |_| panic!("not looked up"), |_| None);
        assert!(matches!(result, Err(LxAppError::InvalidParameter(_))));
    }

    #[test]
    fn site_icons_map_each_cached_drawable_icon_to_its_url() {
        let cache = tempfile::tempdir().unwrap();
        let out = tempfile::tempdir().unwrap();
        let png = cache.path().join("a.png");
        let ico = cache.path().join("b.ico");
        std::fs::write(&png, b"png").unwrap();
        std::fs::write(&ico, b"ico").unwrap();
        let cached = |url: &str| match url {
            "https://a.example/page" => Some(png.clone()),
            "https://b.example/" => Some(ico.clone()),
            _ => None,
        };
        let urls = [
            "https://a.example/page",
            "https://b.example/",
            "https://c.example/",
        ]
        .map(String::from)
        .to_vec();
        let icons = site_icons_for(urls, cached, |source| {
            copy_icon(source, out.path()).map(|target| target.display().to_string())
        })
        .unwrap();

        // Only the PNG is drawable; the ICO and the uncached site keep their monograms.
        let copied = out.path().join("a.png");
        assert_eq!(icons.len(), 1);
        assert_eq!(
            icons["https://a.example/page"],
            copied.display().to_string()
        );
        assert_eq!(std::fs::read(copied).unwrap(), b"png");
    }
}
