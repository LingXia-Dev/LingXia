//! Experimental UI language core. No page-stack or host authority dependencies.
pub mod animation;
mod app;
#[cfg(test)]
mod backdrop_backend_tests;
mod backdrop_region;
mod border;
mod budget;
pub mod color_filter;
mod compatibility;
pub mod composite;
pub mod composite_cpu;
#[cfg(feature = "gpu")]
pub mod composite_gpu;
#[cfg(test)]
mod display_composite_tests;
#[cfg(feature = "gpu")]
pub mod gpu;
#[cfg(feature = "gpu")]
pub mod gpu_color_filter;
#[cfg(feature = "gpu")]
mod gpu_images;
#[cfg(feature = "gpu")]
mod gpu_pass;
#[cfg(feature = "gpu")]
mod gpu_scene;
#[cfg(feature = "gpu")]
mod gpu_surface;
#[cfg(feature = "gpu")]
mod gpu_text;
mod gradient;
pub mod image;
mod image_requests;
#[cfg(test)]
mod image_semantics_tests;
mod input_protocol;
mod input_state;
pub mod list;
mod list_layout;
mod metrics;
mod module;
mod render;
pub mod schema;
mod session;
mod shadow;
pub mod state;
mod text_cache;
pub mod text_input;
mod text_scale;
mod vector;
pub use app::{ViewApp, ViewPageId};
pub use image_requests::{ImageRequest, ImageRequests, ImageSource};
pub use input_state::InputReply;
pub use metrics::FrameMetrics;
pub use plot::{PieLayout, PlotSelection, PlotSnapshot};
pub use render::{DisplayList, Frame, Renderer};
pub use session::ViewSession;

#[cfg(target_os = "android")]
pub mod android;

use rong::{JSEngine, RongJS, Source};

/// Called on a dedicated View thread. No host capabilities are installed.
pub fn evaluate(module: &str, width: f32, height: f32) -> Result<Vec<schema::Record>, String> {
    compatibility::validate(module)?;
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
        return Err("UI_VIEWPORT: invalid dimensions".into());
    }
    let runtime = RongJS::runtime();
    let context = runtime.context();
    compatibility::install(&context)?;
    context
        .eval::<()>(Source::from_bytes(module.as_bytes()))
        .map_err(|e| e.to_string())?;
    let invoke = format!(
        "__lingxiaDescribe(__lingxiaRender, {{data: Object.freeze({{}}), actions: Object.freeze({{}}), env: Object.freeze({{theme:'light', viewport:{{width:{width},height:{height}}}, insets:{{top:0,right:0,bottom:0,left:0}}, keyboard:{{height:0}}, os:'android', formFactor:'phone', sizeClass:'compact', aside:false, displayLanguage:'en', runner:true}})}})"
    );
    decode_context(&context, &invoke)
}

fn decode_context(context: &rong::JSContext, invoke: &str) -> Result<Vec<schema::Record>, String> {
    decode_context_timed(context, invoke).map(|(records, _)| records)
}

fn decode_context_timed(
    context: &rong::JSContext,
    invoke: &str,
) -> Result<(Vec<schema::Record>, metrics::ViewTimings), String> {
    let started = std::time::Instant::now();
    let values: rong::JSArray = context
        .eval(Source::from_bytes(invoke.as_bytes()))
        .map_err(|e| e.to_string())?;
    let eval_us = metrics::micros(started);
    let started = std::time::Instant::now();
    let count = values.len().map_err(|e| e.to_string())?;
    if count as usize > schema::MAX_BUFFER_VALUES {
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
    let transfer_us = metrics::micros(started);
    let started = std::time::Instant::now();
    let records = schema::decode(&buffer)?;
    let decode_us = metrics::micros(started);
    Ok((
        records,
        metrics::ViewTimings {
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

    #[test]
    fn view_has_no_host_capabilities_and_transfers_primitive_buffer() {
        let script = "if (typeof lx !== 'undefined' || typeof fetch !== 'undefined' || typeof setTimeout !== 'undefined') throw Error('host capability leaked'); globalThis.__lingxiaRender = () => {}; globalThis.__lingxiaDescribe = () => [2,-1,'','hello','Hello, world!',0];";
        let records = evaluate(script, 393.0, 760.0).unwrap();
        assert_eq!(records[0].text, "Hello, world!");
        assert!(evaluate(script, f32::NAN, 760.0).is_err());
    }

    #[test]
    fn failed_view_does_not_poison_next_context() {
        assert!(evaluate("throw Error('bad view')", 300.0, 600.0).is_err());
        let script = "globalThis.__lingxiaRender = () => {}; globalThis.__lingxiaDescribe = () => [0,-1,'','','',0,2,0,'','hello','Hello',0];";
        let records = evaluate(script, 300.0, 600.0).unwrap();
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

mod plot;
mod slider;
pub use slider::SliderSnapshot;
