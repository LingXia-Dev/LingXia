//! WebView2 controller and settings setup.

use super::*;

pub(crate) fn create_controller(
    env: &ICoreWebView2Environment,
    hwnd: HWND,
) -> StdResult<ICoreWebView2Controller> {
    let env = env.clone();
    let (tx, rx) = mpsc::channel();

    CreateCoreWebView2ControllerCompletedHandler::wait_for_async_operation(
        Box::new(move |handler| unsafe {
            env.CreateCoreWebView2Controller(hwnd, &handler)
                .map_err(webview2_com::Error::WindowsError)
        }),
        Box::new(move |result, controller| {
            result?;
            tx.send(controller.ok_or_else(|| windows::core::Error::from(E_POINTER)))
                .map_err(|_| windows::core::Error::from(E_POINTER))?;
            Ok(())
        }),
    )
    .map_err(map_webview2_error)?;

    rx.recv()
        .map_err(|_| WebViewError::WebView("Controller callback channel failed".to_string()))?
        .map_err(|err| WebViewError::WebView(format!("Controller creation failed: {err}")))
}

pub(crate) fn configure_controller(controller: &ICoreWebView2Controller) -> StdResult<()> {
    use windows::core::Interface as _;
    unsafe {
        // White default background so a resize or reload (device rotate, lxapp
        // restart, navigation) shows white instead of a black flash before the
        // page repaints. Best-effort: older WebView2 runtimes lack Controller2.
        if let Ok(controller2) = controller.cast::<ICoreWebView2Controller2>() {
            let _ = controller2.SetDefaultBackgroundColor(COREWEBVIEW2_COLOR {
                A: 255,
                R: 255,
                G: 255,
                B: 255,
            });
        }
        controller
            .SetBounds(RECT {
                left: 0,
                top: 0,
                right: 1024,
                bottom: 768,
            })
            .map_err(|err| WebViewError::WebView(format!("SetBounds failed: {err}")))?;
        controller
            .SetIsVisible(false)
            .map_err(|err| WebViewError::WebView(format!("SetIsVisible failed: {err}")))?;
    }
    Ok(())
}

pub(crate) fn configure_settings(
    webview: &ICoreWebView2,
    effective_options: &EffectiveWebViewCreateOptions,
) -> StdResult<()> {
    let relaxed_profile = effective_options.profile == SecurityProfile::BrowserRelaxed;
    unsafe {
        let settings = webview
            .Settings()
            .map_err(|err| WebViewError::WebView(format!("Settings failed: {err}")))?;
        settings
            .SetIsScriptEnabled(true)
            .map_err(|err| WebViewError::WebView(format!("SetIsScriptEnabled failed: {err}")))?;
        settings
            .SetAreDefaultScriptDialogsEnabled(relaxed_profile)
            .map_err(|err| {
                WebViewError::WebView(format!("SetAreDefaultScriptDialogsEnabled failed: {err}"))
            })?;
        // Keep the default context menu available for every profile. lxapp pages
        // (non-relaxed) get it trimmed to the editing items by
        // `configure_context_menu`; browser tabs (relaxed) keep the full menu.
        // (Previously lxapp pages had no menu at all, so users could not even copy.)
        settings
            .SetAreDefaultContextMenusEnabled(true)
            .map_err(|err| {
                WebViewError::WebView(format!("SetAreDefaultContextMenusEnabled failed: {err}"))
            })?;
        settings.SetIsWebMessageEnabled(true).map_err(|err| {
            WebViewError::WebView(format!("SetIsWebMessageEnabled failed: {err}"))
        })?;
        settings
            .SetIsStatusBarEnabled(false)
            .map_err(|err| WebViewError::WebView(format!("SetIsStatusBarEnabled failed: {err}")))?;
        settings
            .SetAreDevToolsEnabled(webview_devtools_enabled())
            .map_err(|err| WebViewError::WebView(format!("SetAreDevToolsEnabled failed: {err}")))?;
    }
    Ok(())
}

/// Standard editing commands kept on the trimmed lxapp context menu, matched by
/// WebView2's locale-independent `ICoreWebView2ContextMenuItem::Name`. Everything
/// else (back/forward/reload, save, print, share, inspect element, …) is removed.
const EDITING_ITEM_NAMES: &[&str] = &[
    "cut",
    "copy",
    "paste",
    "pasteAsPlainText",
    "selectAll",
    "undo",
    "redo",
];

/// Supplies the lxapp page "Refresh" right-click entry. Registered by the host
/// SDK, which knows pull-down-refresh state, i18n, and how to fire a refresh;
/// lingxia-webview sits below that layer, so the context-menu handler calls back
/// through this hook. `label` returns `Some(localized title)` for a page that
/// opted into pull-down refresh (else `None`); `trigger` starts a refresh.
type RefreshMenuLabelFn = Arc<dyn Fn(&str, &str) -> Option<String> + Send + Sync>;
type RefreshMenuTriggerFn = Arc<dyn Fn(&str, &str) + Send + Sync>;
static REFRESH_MENU_PROVIDER: Mutex<Option<(RefreshMenuLabelFn, RefreshMenuTriggerFn)>> =
    Mutex::new(None);

/// Register the pull-down-refresh context-menu provider (host SDK -> webview).
pub fn set_windows_context_menu_refresh_provider(
    label: RefreshMenuLabelFn,
    trigger: RefreshMenuTriggerFn,
) {
    if let Ok(mut slot) = REFRESH_MENU_PROVIDER.lock() {
        *slot = Some((label, trigger));
    }
}

fn refresh_menu_label(appid: &str, path: &str) -> Option<String> {
    let provider = REFRESH_MENU_PROVIDER
        .lock()
        .ok()
        .and_then(|slot| slot.clone())?;
    (provider.0)(appid, path)
}

fn trigger_refresh_menu(appid: &str, path: &str) {
    let provider = REFRESH_MENU_PROVIDER
        .lock()
        .ok()
        .and_then(|slot| slot.clone());
    if let Some((_, trigger)) = provider {
        trigger(appid, path);
    }
}

/// Trim the WebView2 context menu for lxapp pages.
///
/// lxapp pages are not a browser, so WebView2's default right-click menu is
/// inappropriate — the macOS SDK strips it down to Copy plus the standard editing
/// commands. We do the same here by handling `ContextMenuRequested` and removing
/// every non-editing item from the menu before it is shown. On a page that opted
/// into pull-down refresh we then prepend an app-level "Refresh" entry (matching
/// the macOS lxapp menu). Browser tabs (the relaxed profile) keep the full menu.
pub(crate) fn configure_context_menu(
    webview: &ICoreWebView2,
    env: &ICoreWebView2Environment,
    appid: &str,
    path: &str,
    effective_options: &EffectiveWebViewCreateOptions,
) -> StdResult<()> {
    if effective_options.profile == SecurityProfile::BrowserRelaxed {
        return Ok(());
    }

    // The ContextMenuRequested event needs ICoreWebView2_11. On older runtimes
    // lxapp pages simply keep the default menu rather than failing creation.
    let webview11 = match webview.cast::<ICoreWebView2_11>() {
        Ok(webview11) => webview11,
        Err(err) => {
            log::warn!("ContextMenuRequested unavailable; lxapp keeps default menu: {err}");
            return Ok(());
        }
    };

    let env = env.clone();
    let appid = appid.to_string();
    let path = path.to_string();
    let handler = ContextMenuRequestedEventHandler::create(Box::new(move |_sender, args| {
        let Some(args) = args else {
            return Ok(());
        };
        unsafe {
            let items = args.MenuItems()?;
            trim_to_editing_items(&items)?;
            insert_refresh_item(&env, &items, &appid, &path)?;
        }
        Ok(())
    }));

    let mut token = 0;
    unsafe {
        webview11
            .add_ContextMenuRequested(&handler, &mut token)
            .map_err(|err| {
                WebViewError::WebView(format!("add_ContextMenuRequested failed: {err}"))
            })?;
    }
    Ok(())
}

/// On a pull-down-refresh page, prepend an app-level "Refresh" item (plus a
/// separator) that fires `onPullDownRefresh`, mirroring the macOS lxapp menu.
/// We deliberately do NOT keep WebView2's native Reload: a real reload re-fetches
/// the lxapp bundle and drops runtime state.
fn insert_refresh_item(
    env: &ICoreWebView2Environment,
    items: &ICoreWebView2ContextMenuItemCollection,
    appid: &str,
    path: &str,
) -> WinResult<()> {
    let Some(label) = refresh_menu_label(appid, path) else {
        return Ok(());
    };
    // CreateContextMenuItem needs ICoreWebView2Environment9; skip on older runtimes.
    let Ok(env9) = env.cast::<ICoreWebView2Environment9>() else {
        return Ok(());
    };
    let no_icon: Option<&IStream> = None;
    let label_w: Vec<u16> = label.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let item = env9.CreateContextMenuItem(
            PCWSTR(label_w.as_ptr()),
            no_icon,
            COREWEBVIEW2_CONTEXT_MENU_ITEM_KIND_COMMAND,
        )?;
        let appid = appid.to_string();
        let path = path.to_string();
        let selected = CustomItemSelectedEventHandler::create(Box::new(move |_item, _args| {
            trigger_refresh_menu(&appid, &path);
            Ok(())
        }));
        let mut token = 0i64;
        item.add_CustomItemSelected(&selected, &mut token)?;
        items.InsertValueAtIndex(0, &item)?;

        // Separate Refresh from the editing items only when some remain.
        let mut count = 0u32;
        items.Count(&mut count)?;
        if count > 1 {
            let separator = env9.CreateContextMenuItem(
                PCWSTR::null(),
                no_icon,
                COREWEBVIEW2_CONTEXT_MENU_ITEM_KIND_SEPARATOR,
            )?;
            items.InsertValueAtIndex(1, &separator)?;
        }
    }
    Ok(())
}

fn trim_to_editing_items(items: &ICoreWebView2ContextMenuItemCollection) -> WinResult<()> {
    unsafe {
        // Remove disallowed command items back-to-front so indices stay valid.
        let mut count = 0u32;
        items.Count(&mut count)?;
        for index in (0..count).rev() {
            let item = items.GetValueAtIndex(index)?;
            let mut kind = COREWEBVIEW2_CONTEXT_MENU_ITEM_KIND_SEPARATOR;
            item.Kind(&mut kind)?;
            if kind == COREWEBVIEW2_CONTEXT_MENU_ITEM_KIND_SEPARATOR {
                continue;
            }
            let mut name = PWSTR::null();
            item.Name(&mut name)?;
            let name = CoTaskMemPWSTR::from(name).to_string();
            if !EDITING_ITEM_NAMES.contains(&name.as_str()) {
                items.RemoveValueAtIndex(index)?;
            }
        }
        tidy_separators(items)
    }
}

/// Drop leading/trailing separators and collapse consecutive ones left behind by
/// the removals above. Menus are tiny, so the repeated rescan is cheap.
fn tidy_separators(items: &ICoreWebView2ContextMenuItemCollection) -> WinResult<()> {
    unsafe {
        loop {
            let mut count = 0u32;
            items.Count(&mut count)?;

            let mut removed_at: Option<u32> = None;
            // `prev_separator` starts true so a leading separator is dropped.
            let mut prev_separator = true;
            for index in 0..count {
                let item = items.GetValueAtIndex(index)?;
                let mut kind = COREWEBVIEW2_CONTEXT_MENU_ITEM_KIND_SEPARATOR;
                item.Kind(&mut kind)?;
                let is_separator = kind == COREWEBVIEW2_CONTEXT_MENU_ITEM_KIND_SEPARATOR;
                if is_separator && prev_separator {
                    removed_at = Some(index);
                    break;
                }
                prev_separator = is_separator;
            }
            // A trailing separator survives the loop (nothing follows it).
            if removed_at.is_none() && prev_separator && count > 0 {
                removed_at = Some(count - 1);
            }

            match removed_at {
                Some(index) => items.RemoveValueAtIndex(index)?,
                None => break,
            }
        }
        Ok(())
    }
}
