//! JavaScript engine adapters for View isolates.
pub(crate) mod budget;
pub(crate) mod thread_clock;
pub(crate) mod compatibility;
mod quickjs;

// QuickJS on every target, beside whatever engine the app's Logic runtime uses.
pub(crate) use rong::quickjs::{JSArray, JSContext, JSRuntime, RongJS};

/// A module feature an engine adapter implements for compiled UI bundles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Feature {
    /// Candidate render/commit/discard of View output.
    CandidateTransactions,
    /// Microtasks drain inside the frame budget and a runaway job can be interrupted.
    BoundedJobs,
}

impl Feature {
    pub const fn name(self) -> &'static str {
        match self {
            Self::CandidateTransactions => "candidate-transactions",
            Self::BoundedJobs => "bounded-jobs",
        }
    }
}

/// The per-engine half of a View isolate. Capabilities come from the impl, never from target lists.
pub(crate) trait Engine {
    const FEATURES: &'static [Feature];
    /// Whether the budget watchdog can interrupt running script.
    const PREEMPTIBLE: bool;
    fn evaluate_module(context: &JSContext, source: &str) -> Result<(), String>;
    fn drain_jobs(context: &JSContext) -> Result<(), String>;
    /// Drain under interruption before dropping a failed context or running a sibling page.
    fn discard_jobs(context: &JSContext);
}

pub(crate) type Active = quickjs::QuickJs;

/// Features the linked engine provides.
pub fn features() -> &'static [Feature] {
    <Active as Engine>::FEATURES
}

pub(crate) fn supports(feature: Feature) -> bool {
    features().contains(&feature)
}

use rong::{JSEngine, Source};

/// Called on a dedicated View thread. No host capabilities are installed.
/// `environment` is the platform's View `env`; the viewport is added to it.
pub fn evaluate(
    module: &str,
    environment: &crate::platform::FrameEnvironment,
) -> Result<Vec<crate::protocol::Record>, String> {
    let (width, height) = (f32::from(environment.width), f32::from(environment.height));
    crate::engine::compatibility::validate(module)?;
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
        return Err("UI_VIEWPORT: invalid dimensions".into());
    }
    let runtime = RongJS::runtime();
    let context = runtime.context();
    crate::engine::compatibility::install(&context)?;
    context
        .eval::<()>(Source::from_bytes(module.as_bytes()))
        .map_err(|e| js_error(&context, e))?;
    let mut env = environment.value.clone();
    if !env.is_object() {
        return Err("UI_ENVIRONMENT: expected an object".into());
    }
    env["viewport"] = serde_json::json!({"width": width, "height": height});
    let invoke = format!(
        "__lingxiaDescribe(__lingxiaRender, {{data: Object.freeze({{}}), actions: Object.freeze({{}}), env: Object.freeze({env})}})"
    );
    decode_context(&context, &invoke)
}

pub(crate) fn decode_context(
    context: &JSContext,
    invoke: &str,
) -> Result<Vec<crate::protocol::Record>, String> {
    decode_context_timed(context, invoke).map(|(records, _)| records)
}

/// A JS failure as text. Thrown values keep their own message, which carries
/// the `UI_*` code and the JSX trace; rong's display of them drops both.
pub(crate) fn js_error(context: &JSContext, error: rong::RongJSError) -> String {
    if error.is_thrown() {
        if let Some(host) = error.into_host_in(context).into_host_error() {
            return host.message;
        }
        return "JavaScript threw a value".into();
    }
    error.to_string()
}

pub(crate) fn decode_context_timed(
    context: &JSContext,
    invoke: &str,
) -> Result<
    (
        Vec<crate::protocol::Record>,
        crate::session::metrics::ViewTimings,
    ),
    String,
> {
    let started = std::time::Instant::now();
    let values: JSArray = context
        .eval(Source::from_bytes(invoke.as_bytes()))
        .map_err(|e| js_error(context, e))?;
    let eval_us = crate::session::metrics::micros(started);
    let started = std::time::Instant::now();
    let count = values.len().map_err(|e| e.to_string())?;
    if count as usize > crate::protocol::limits::WIRE_VALUES {
        return Err("UI_LIMIT: buffer too large".into());
    }
    let buffer = (0..count)
        .map(|index| {
            let value = values.get_value(index).map_err(|e| e.to_string())?;
            if value.is_number() {
                let n: f64 = value.to_rust().map_err(|e| e.to_string())?;
                if n.is_finite() && n.fract() == 0.0 && n.abs() < 9_007_199_254_740_992.0 {
                    return Ok(serde_json::Value::from(n as i64));
                }
                serde_json::Number::from_f64(n)
                    .map(serde_json::Value::Number)
                    .ok_or("UI_BUFFER: non-finite number".into())
            } else if value.is_string() {
                value
                    .to_rust::<String>()
                    .map(serde_json::Value::String)
                    .map_err(|e| e.to_string())
            } else {
                Err("UI_BUFFER: expected primitive".into())
            }
        })
        .collect::<Result<Vec<_>, String>>()?;
    drop(values);
    let transfer_us = crate::session::metrics::micros(started);
    let started = std::time::Instant::now();
    let records = crate::protocol::decode(&buffer)?;
    let decode_us = crate::session::metrics::micros(started);
    Ok((
        records,
        crate::session::metrics::ViewTimings {
            eval_us,
            transfer_us,
            decode_us,
            values: count as usize,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Renderer;

    #[test]
    fn transfer_rejects_oversized_arrays_before_reading_any_element() {
        let runtime = RongJS::runtime();
        let context = runtime.context();
        let error=decode_context(&context,"(()=>{const values=[];values.length=100001;Object.defineProperty(values,0,{get(){throw Error('element read before bound')}});return values})()").unwrap_err();
        assert!(error.contains("UI_LIMIT: buffer too large"), "{error}");
        let valid = decode_context(&context, "[2,-1,'','label','Still usable',0]").unwrap();
        assert_eq!(valid[0].text, "Still usable");
    }

    #[test]
    fn transfer_preserves_primitive_validation_and_getter_errors() {
        let runtime = RongJS::runtime();
        let context = runtime.context();
        for value in [
            "null",
            "true",
            "{}",
            "()=>0",
            "1n",
            "NaN",
            "Infinity",
            "undefined",
        ] {
            let expression = format!("[2,-1,'','label',{value},0]");
            assert!(decode_context(&context, &expression).is_err(), "{value}");
        }
        let error=decode_context(&context,"(()=>{const values=[2,-1,'','label','text',0];Object.defineProperty(values,4,{get(){globalThis.getterReads=1;throw Error('getter failure')}});return values})()").unwrap_err();
        assert!(!error.is_empty());
        let reads: u32 = context
            .eval(Source::from_bytes("globalThis.getterReads"))
            .unwrap();
        assert_eq!(reads, 1);
        assert!(decode_context(&context, "[2,-1,'','label','Recovered',0]").is_ok());
    }

    fn environment(width: u16, height: u16) -> crate::platform::FrameEnvironment {
        crate::platform::FrameEnvironment {
            width,
            height,
            scale: 1.0,
            value: serde_json::json!({"theme": "light", "os": "test", "pointer": "coarse"}),
        }
    }

    #[test]
    fn view_has_no_host_capabilities_and_transfers_primitive_buffer() {
        let script = "if (typeof lx !== 'undefined' || typeof fetch !== 'undefined' || typeof setTimeout !== 'undefined') throw Error('host capability leaked'); globalThis.__lingxiaRender = () => {}; globalThis.__lingxiaDescribe = () => [2,-1,'','hello','Hello, world!',0];";
        let records = evaluate(script, &environment(393, 760)).unwrap();
        assert_eq!(records[0].text, "Hello, world!");
        assert!(evaluate(script, &environment(0, 760)).is_err());
    }

    #[test]
    fn failed_view_does_not_poison_next_context() {
        assert!(evaluate("throw Error('bad view')", &environment(300, 600)).is_err());
        let script = "globalThis.__lingxiaRender = () => {}; globalThis.__lingxiaDescribe = () => [0,-1,'','','',0,2,0,'','hello','Hello',0];";
        let records = evaluate(script, &environment(300, 600)).unwrap();
        let frame = Renderer::default().render(&records, 300, 600, 1.0).unwrap();
        assert_eq!(frame.nodes.len(), 2);
        assert!(frame.nodes[1].bounds[2] > 0.0);
        assert!(frame.nodes[1].bounds[3] > 0.0);
        let pixels = frame.pixels.data();
        assert!(
            pixels
                .iter()
                .any(|pixel| *pixel != pixels[pixels.len() - 1])
        );
    }
}
