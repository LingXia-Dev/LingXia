use crate::error;
use crate::lxapp::LxApp;
use crate::plugin;
use rong::{JSContext, JSFunc, JSResult, Source, error::HostError};

pub(crate) async fn ensure_plugin_logic_loaded(ctx: &JSContext, plugin_name: &str) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(ctx)?;

    let Some(plugin_cfg) = lxapp.config.plugins.get(plugin_name) else {
        error!("Plugin not configured: {}", plugin_name).with_appid(lxapp.appid.clone());
        return Err(HostError::new(
            rong::error::E_INTERNAL,
            format!("Plugin not configured: {}", plugin_name),
        )
        .into());
    };

    let mut logic_js_path = plugin::get_plugin_logic_js(&lxapp.runtime, plugin_name, plugin_cfg);
    if logic_js_path.is_none() {
        // Plugin might not be installed yet; wait/trigger download so navigation doesn't fail.
        match plugin::download_and_install(lxapp.runtime.clone(), plugin_name, plugin_cfg).await {
            Ok(_) => {
                logic_js_path =
                    plugin::get_plugin_logic_js(&lxapp.runtime, plugin_name, plugin_cfg);
            }
            Err(e) => {
                error!("Failed to download/install plugin {}: {}", plugin_name, e)
                    .with_appid(lxapp.appid.clone());
                return Err(HostError::new(
                    rong::error::E_INTERNAL,
                    format!("Failed to download/install plugin {}: {}", plugin_name, e),
                )
                .into());
            }
        }
    }

    let Some(logic_js_path) = logic_js_path else {
        return Err(HostError::new(
            rong::error::E_INTERNAL,
            format!("Plugin logic.js not found: {}", plugin_name),
        )
        .into());
    };

    let should_load = match super::runtime_ctx::mark_plugin_loaded_if_new(ctx, plugin_name) {
        Ok(v) => v,
        Err(e) => {
            error!("Plugin logic load check failed: {}", e).with_appid(lxapp.appid.clone());
            return Err(HostError::new(
                rong::error::E_INTERNAL,
                format!("Plugin logic load check failed: {}", e),
            )
            .into());
        }
    };
    if !should_load {
        return Ok(());
    }

    let load_result: JSResult<()> = async {
        let js = Source::from_path(ctx, &logic_js_path).await?;
        ctx.eval::<()>(js)?;
        Ok(())
    }
    .await;

    if let Err(e) = &load_result {
        let _ = super::runtime_ctx::unmark_plugin_loaded(ctx, plugin_name);
        error!("Failed to load plugin logic.js: {}", e).with_appid(lxapp.appid.clone());
    }

    load_result
}

pub(crate) async fn ensure_plugin_logic_loaded_for_page_path(
    ctx: &JSContext,
    page_path: &str,
) -> JSResult<()> {
    let Some((plugin_name, _)) = plugin::parse_plugin_page_path(page_path) else {
        return Ok(());
    };
    ensure_plugin_logic_loaded(ctx, &plugin_name).await
}

async fn require_plugin(ctx: JSContext, plugin_name: String) -> JSResult<()> {
    let name = plugin_name.trim();
    if name.is_empty() {
        return Err(HostError::new(
            rong::error::E_INTERNAL,
            "requirePlugin: plugin name is empty",
        )
        .into());
    }
    ensure_plugin_logic_loaded(&ctx, name).await
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    let require_plugin = JSFunc::new(ctx, require_plugin)?;
    ctx.global().set("__LX_REQUIRE_PLUGIN__", require_plugin)?;

    let plugin_js = Source::from_bytes(include_str!("scripts/Plugin.js"));
    ctx.eval::<()>(plugin_js)?;

    Ok(())
}
