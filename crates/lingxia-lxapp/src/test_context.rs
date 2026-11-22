//! Binds a throwaway dev lxapp to a JS context, so a Logic test can boot the full `lx` surface.

use crate::LxApp;
use crate::lxapp::register_dev_bundle_source;
use crate::{Channel, appservice};
use lingxia_platform::Platform;
use rong::JSContext;
use std::sync::Arc;

/// A minimal GPU lxapp under a fresh temp directory, bound to `ctx` as its runtime context, ready for the Logic extension.
pub fn bind_test_lxapp(ctx: &JSContext, appid: &str) -> Arc<LxApp> {
    let root = std::env::temp_dir().join(format!("lingxia-test-lxapp-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(root.join("pages/home")).unwrap();
    std::fs::write(
        root.join("lxapp.json"),
        format!(
            r#"{{"appId":"{appid}","version":"1.0.0","logic":true,"renderer":"gpu","pages":[{{"name":"home","path":"pages/home/index.js"}}]}}"#
        ),
    )
    .unwrap();
    let (data, cache) = (root.join("data"), root.join("cache"));
    std::fs::create_dir_all(&data).unwrap();
    std::fs::create_dir_all(&cache).unwrap();
    let runtime = Arc::new(
        Platform::new(
            data.display().to_string(),
            cache.display().to_string(),
            "en-US".to_string(),
        )
        .expect("test platform"),
    );
    register_dev_bundle_source(appid, &root);
    let workers = appservice::LxAppWorkers::init(1);
    let app = Arc::new(
        LxApp::new(appid.to_string(), runtime, workers, Channel::Draft).expect("test lxapp"),
    );
    app.bind_arc();
    appservice::prepare_test_context(ctx, &app).expect("test context");
    app
}
