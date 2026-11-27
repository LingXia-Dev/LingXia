use super::set_app_svc_for_ctx;
use crate::lifecycle::AppServiceEvent;
use rong::{
    JSContext, JSFunc, JSObject, JSResult, JSValue, Source, error::HostError, function::Optional,
    js_class, js_method,
};
use std::collections::HashMap;

#[js_class(clone)]
pub(crate) struct LxAppSvc {
    event_handlers: HashMap<AppServiceEvent, JSFunc>,
    this: JSObject,
}

#[js_class]
impl LxAppSvc {
    #[js_method(constructor)]
    fn _new(ctx: JSContext, obj: JSObject, handler_names_json: Optional<String>) -> JSResult<Self> {
        let mut app_svc = LxAppSvc {
            event_handlers: HashMap::new(),
            this: obj.clone(),
        };

        app_svc.assign_funcs(&obj, handler_names_json.0.as_deref())?;
        if let Ok(app) = crate::lxapp::LxApp::from_ctx(&ctx) {
            app.launch_gate
                .set_links_through_on_open(obj.get::<_, JSFunc>("onOpen").is_ok());
        }

        // Bind this AppSvc instance to the LxApp runtime context associated with ctx.
        set_app_svc_for_ctx(&ctx, app_svc.clone())?;

        Ok(app_svc)
    }

    #[js_method(gc_mark)]
    fn gc_mark_with<F>(&self, mut mark_fn: F)
    where
        F: FnMut(&JSValue),
    {
        for func in self.event_handlers.values() {
            mark_fn(func.as_js_value());
        }
        mark_fn(self.this.as_js_value());
    }
}

impl LxAppSvc {
    fn assign_funcs(&mut self, obj: &JSObject, handler_names_json: Option<&str>) -> JSResult<()> {
        let handler_names: Vec<String> = handler_names_json
            .map(serde_json::from_str)
            .transpose()
            .map_err(|e| HostError::new(rong::error::E_INTERNAL, e.to_string()))?
            .unwrap_or_default();

        for key_string in handler_names {
            if let Some(evt) = AppServiceEvent::from_name(&key_string)
                && let Ok(func) = obj.get::<_, JSFunc>(key_string.as_str())
            {
                self.event_handlers.insert(evt, func);
            }
        }

        // Metadata is a fast path, but still reflect over the realized object so
        // spread-derived and aliased lifecycle handlers remain compatible.
        for key_value in obj.keys()? {
            let Ok(key_string) = key_value.to_rust::<String>() else {
                continue;
            };
            if key_string.starts_with('_') {
                continue;
            }
            if let Some(evt) = AppServiceEvent::from_name(&key_string)
                && let Ok(func) = obj.get::<_, JSFunc>(key_string.as_str())
            {
                self.event_handlers.insert(evt, func);
            }
        }
        Ok(())
    }

    pub async fn call_event(
        &self,
        ctx: &JSContext,
        event: AppServiceEvent,
        args: Option<String>,
    ) -> JSResult<()> {
        if let Some(func) = self.event_handlers.get(&event) {
            // The caller runs this outside the worker pump. Await the handler so
            // its task retains the JSContext through any returned Promise. Call
            // directly because Rong's runtime-wide invoke queue can outlive this
            // app context when an LxApp is restarted.
            let args_obj = args
                .as_ref()
                .and_then(|json| JSObject::from_json_string(ctx, json).ok());
            return match args_obj {
                Some(obj) => {
                    func.call_async::<_, ()>(Some(self.this.clone()), (obj,))
                        .await
                }
                None => func.call_async::<_, ()>(Some(self.this.clone()), ()).await,
            };
        }
        // App lifecycle handlers are optional, matching PageInstance handlers.
        Ok(())
    }

    /// Runs `onLaunch` and returns the first page it settled on, as JSON (`null` for none).
    pub async fn call_launch(&self, ctx: &JSContext, args: Option<String>) -> JSResult<String> {
        let Some(func) = self.event_handlers.get(&AppServiceEvent::OnLaunch) else {
            return Ok("null".to_string());
        };
        let args_obj = args
            .as_ref()
            .and_then(|json| JSObject::from_json_string(ctx, json).ok());
        let value: JSValue = match args_obj {
            Some(obj) => {
                func.call_async::<_, JSValue>(Some(self.this.clone()), (obj,))
                    .await?
            }
            None => {
                func.call_async::<_, JSValue>(Some(self.this.clone()), ())
                    .await?
            }
        };
        Ok(super::js_value_to_json_string(value).unwrap_or_else(|_| "null".into()))
    }
}

// Register the global App function
pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    ctx.register_class::<LxAppSvc>()?;

    let app_js = Source::from_bytes(include_str!("scripts/App.js"));
    ctx.eval::<()>(app_js)?;
    #[cfg(feature = "rejection-tracking")]
    track_unhandled_rejections(ctx)?;

    Ok(())
}

/// Report each promise rejection nothing handled to `App.onError`, as module-level: the engine's
/// event names no page or action. An engine that cannot track them (system JavaScriptCore) is left as is.
#[cfg(feature = "rejection-tracking")]
macro_rules! rejection_tracking {
    ($name:ident, $($engine:ident)::+) => {
        fn $name(ctx: &$($engine)::+::JSContext) -> rong::JSResult<()> {
            use $($engine)::+::{JSFunc, JSObject, JSValue, RejectionEvent};
            if !ctx.supports_rejection_tracking().unhandled {
                return Ok(());
            }
            ctx.set_rejection_tracker(rong::RejectionEvents::UNHANDLED, |ctx, event| {
                let RejectionEvent::Unhandled { reason, .. } = event else {
                    return Ok(());
                };
                let report: JSFunc = ctx.global().get("__LX_REPORT_FAILURE__")?;
                let source = JSObject::new(ctx);
                source.set("kind", "app")?;
                source.set("hook", JSValue::null(ctx))?;
                let failure = JSObject::new(ctx);
                failure.set("error", reason)?;
                failure.set("source", source)?;
                report.call::<_, ()>(None, (failure, JSValue::null(ctx)))
            })
        }
    };
}
#[cfg(feature = "rejection-tracking")]
rejection_tracking!(track_unhandled_rejections, rong);

#[cfg(all(test, feature = "rejection-tracking"))]
mod rejection_tests {
    use rong::{JSEngine, Source};

    // QuickJS reports rejections on every host; the primary engine on macOS does not.
    rejection_tracking!(track_on_quickjs, rong::quickjs);

    #[test]
    fn a_rejection_nothing_handles_reaches_app_on_error_as_module_level() {
        let runtime = <rong::quickjs::RongJS as JSEngine>::runtime();
        let ctx = runtime.context();
        ctx.eval::<()>(Source::from_bytes(
            "globalThis.seen = []; globalThis.__LX_REPORT_FAILURE__ = (raw, page) => \
             seen.push([raw.error.message, raw.source.kind, raw.source.hook, page]);",
        ))
        .unwrap();
        track_on_quickjs(&ctx).unwrap();
        ctx.eval::<()>(Source::from_bytes(
            "Promise.reject(new Error('lost')); \
             Promise.reject(new Error('caught')).catch(() => {}); undefined",
        ))
        .unwrap();
        runtime.run_pending_jobs();
        runtime.run_pending_jobs();
        let seen: String = ctx
            .eval(Source::from_bytes("JSON.stringify(seen)"))
            .unwrap();
        assert_eq!(seen, r#"[["lost","app",null,null]]"#);
    }
}

#[cfg(all(test, feature = "js-appservice"))]
mod launch_destination_tests {
    use rong::{JSEngine, Source};

    /// Runs `script` after App.js with stubbed `LxAppSvc`/`lx.nav.go`; returns `log` as JSON.
    fn run(script: &str) -> String {
        let runtime = <rong::RongJS as JSEngine>::runtime();
        let ctx = runtime.context();
        ctx.eval::<()>(Source::from_bytes(
            "globalThis.log = []; globalThis.LxAppSvc = class {}; \
             globalThis.lx = { nav: { go: (t) => { log.push(['go', t]); return { opened: Promise.resolve() }; } } };",
        ))
        .unwrap();
        ctx.eval::<()>(Source::from_bytes(include_str!("scripts/App.js")))
            .unwrap();
        ctx.eval::<()>(Source::from_bytes(script)).unwrap();
        for _ in 0..20 {
            runtime.run_pending_jobs();
        }
        ctx.eval(Source::from_bytes("JSON.stringify(log)")).unwrap()
    }

    const LINK: &str = "{ scene: 8003, url: 'https://x.app/reset?c=1', path: '', query: { c: '1' }, launchDeadline: Date.now() + 60000 }";

    fn launch(config: &str, options: &str) -> String {
        run(&format!(
            "const app = __registerApp({config}); \
             app.onLaunch(JSON.stringify({options})).then((d) => {{ log.push(['decided', d]); app.onShow('{{}}'); }});"
        ))
    }

    #[test]
    fn on_launch_target_is_the_first_page_and_skips_on_open() {
        let log = launch(
            "{ onLaunch: ({ link }) => (log.push(['link', link.url]), { page: 'onboarding' }), onOpen: () => log.push(['onOpen']) }",
            LINK,
        );
        assert_eq!(
            log,
            r#"[["link","https://x.app/reset?c=1"],["decided",{"page":"onboarding"}]]"#
        );
    }

    #[test]
    fn a_cold_link_reaches_on_open_before_the_first_page() {
        let log = launch(
            "{ onOpen: (link) => ({ page: 'reset', params: link.query }) }",
            LINK,
        );
        assert_eq!(log, r#"[["decided",{"page":"reset","params":{"c":"1"}}]]"#);
        let log = launch("{ onOpen: () => {} }", LINK);
        assert_eq!(log, r#"[["decided","start"]]"#);
    }

    #[test]
    fn without_a_link_or_target_the_requested_page_stays() {
        let log = launch(
            "{ onLaunch() {}, onOpen: () => log.push(['onOpen']) }",
            "{ scene: 1001, launchDeadline: Date.now() + 60000 }",
        );
        assert_eq!(log, r#"[["decided",null]]"#);
    }

    #[test]
    fn late_or_foreign_targets_open_after_the_first_page() {
        let log = launch(
            "{ onLaunch: () => ({ page: 'late' }) }",
            "{ scene: 1001, launchDeadline: Date.now() - 1 }",
        );
        assert_eq!(log, r#"[["decided",null],["go",{"page":"late"}]]"#);
        let log = launch("{ onOpen: () => ({ url: 'https://x.app' }) }", LINK);
        assert_eq!(
            log,
            r#"[["decided","start"],["go",{"url":"https://x.app"}]]"#
        );
    }

    #[test]
    fn a_throwing_on_launch_opens_the_start_page_and_still_routes_the_link() {
        let log = launch(
            "{ onLaunch() { throw new Error('boom'); }, onOpen: () => ({ page: 'reset' }), onError: (f) => log.push(['onError', f.source.hook]) }",
            LINK,
        );
        assert!(log.starts_with(r#"[["decided","start"]"#), "{log}");
        assert!(log.contains(r#"["go",{"page":"reset"}]"#), "{log}");
        assert!(log.contains(r#"["onError","onLaunch"]"#), "{log}");
    }
}
