use lingxia_platform::{VideoPlayerCommand, VideoPlayerHandle, VideoPlayerManager};
use lxapp::{LxApp, lx};
use rong::{JSContext, JSFunc, JSResult, JSValue, RongJSError, js_class, js_export, js_method};
use std::sync::Arc;

pub fn init(ctx: &JSContext) -> JSResult<()> {
    ctx.register_class::<JSVideoContext>()?;
    let create_ctx = JSFunc::new(ctx, |ctx: JSContext, component_id: String| {
        JSVideoContext::create(&ctx, component_id)
    })?;
    lx::register_js_api(ctx, "createVideoContext", create_ctx)?;
    Ok(())
}

#[js_export]
pub struct JSVideoContext {
    player_handle: Arc<dyn VideoPlayerHandle>,
}

impl JSVideoContext {
    pub fn create(ctx: &JSContext, component_id: String) -> JSResult<Self> {
        if component_id.trim().is_empty() {
            return Err(RongJSError::Error("componentId required".into()));
        }

        let lxapp = LxApp::from_ctx(ctx)?;
        let runtime: &dyn VideoPlayerManager = lxapp.runtime.as_ref();
        let handle = runtime
            .bind_player(&component_id, 0)
            .map_err(|e| RongJSError::Error(e.to_string()))?;

        Ok(Self {
            player_handle: handle.into(),
        })
    }

    fn dispatch(&self, command: VideoPlayerCommand) -> JSResult<()> {
        self.player_handle
            .dispatch(command)
            .map_err(|e| RongJSError::Error(e.to_string()))
    }
}

#[js_class]
impl JSVideoContext {
    #[js_method(constructor)]
    fn _ctor() -> JSResult<()> {
        Err(RongJSError::TypeError("Use lx.createVideoContext()".into()))
    }

    #[js_method]
    fn play(&self) -> JSResult<()> {
        self.dispatch(VideoPlayerCommand::Play)
    }

    #[js_method]
    fn pause(&self) -> JSResult<()> {
        self.dispatch(VideoPlayerCommand::Pause)
    }

    #[js_method]
    fn stop(&self) -> JSResult<()> {
        self.dispatch(VideoPlayerCommand::Stop)
    }

    #[js_method]
    fn seek(&self, position: f64) -> JSResult<()> {
        self.dispatch(VideoPlayerCommand::Seek { position })
    }

    #[js_method(rename = "requestFullScreen")]
    fn request_full_screen(&self) -> JSResult<()> {
        self.dispatch(VideoPlayerCommand::EnterFullscreen)
    }

    #[js_method(rename = "exitFullScreen")]
    fn exit_full_screen(&self) -> JSResult<()> {
        self.dispatch(VideoPlayerCommand::ExitFullscreen)
    }

    #[js_method(gc_mark)]
    fn gc_mark_with<F>(&self, _mark_fn: F)
    where
        F: FnMut(&JSValue),
    {
    }
}
