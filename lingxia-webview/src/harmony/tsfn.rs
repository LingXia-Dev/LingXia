use miniapp::MiniAppError;
use napi_ohos::Status;
use napi_ohos::bindgen_prelude::Function;
use napi_ohos::threadsafe_function::{ThreadsafeCallContext, ThreadsafeFunctionCallMode};
use std::sync::OnceLock;

// Global ThreadSafe Function storage
//
// Node-API ThreadSafe Function limitation: napi_call_threadsafe_function() can only
// pass a single void* data pointer. To pass multiple parameters, we pack them into
// a single string with colon separator: "function_name:arg1:arg2:..."
type CallbackTsfn = napi_ohos::threadsafe_function::ThreadsafeFunction<
    String,
    napi_ohos::JsUnknown,
    napi_ohos::JsString,
    false,
    false,
    200,
>;
static CALLBACK_TSFN: OnceLock<CallbackTsfn> = OnceLock::new();

/// Initialize the ThreadSafe Function for TSFN calls
pub fn init(callback_function: Function) -> Result<(), String> {
    // Create ThreadSafe Function for callback - pass colon-separated string with function name and args
    let tsfn = callback_function
        .build_threadsafe_function::<String>()
        .callee_handled::<false>()
        .max_queue_size::<200>()
        .build_callback(|ctx: ThreadsafeCallContext<String>| {
            let data = ctx.value;
            log::info!("ThreadSafe callback called with data: {}", data);

            // Return the data string to JavaScript
            let js_string = ctx.env.create_string(&data)?;
            Ok(js_string)
        });

    let tsfn = match tsfn {
        Ok(tsfn) => {
            log::info!("ThreadSafe Function created successfully");
            tsfn
        }
        Err(e) => {
            log::error!("Failed to create threadsafe function: {:?}", e);
            return Err(format!("Failed to create threadsafe function: {:?}", e));
        }
    };

    // Store the ThreadSafe Function globally
    if CALLBACK_TSFN.set(tsfn).is_err() {
        log::error!("ThreadSafe Function already set");
        return Err("ThreadSafe Function already set".to_string());
    }

    Ok(())
}

/// Helper function for TSFN calls
pub fn call_arkts(name: &str, args: &[&str]) -> Result<(), MiniAppError> {
    let tsfn = CALLBACK_TSFN
        .get()
        .ok_or_else(|| MiniAppError::WebView("No callback".to_string()))?;
    let data = format!("{}|{}", name, args.join("|"));
    match tsfn.call(data, ThreadsafeFunctionCallMode::Blocking) {
        Status::Ok => Ok(()),
        _ => Err(MiniAppError::WebView("TSFN call failed".to_string())),
    }
}

/// Helper function for TSFN calls with callback
pub fn call_arkts_with_callback<F>(
    name: &str,
    args: &[&str],
    callback: F,
) -> Result<(), MiniAppError>
where
    F: FnOnce() + Send + 'static,
{
    let tsfn = CALLBACK_TSFN
        .get()
        .ok_or_else(|| MiniAppError::WebView("No callback".to_string()))?;
    let data = format!("{}|{}", name, args.join("|"));

    // Call ArkTS with return value and wait for completion
    match tsfn.call_with_return_value(
        data,
        ThreadsafeFunctionCallMode::Blocking,
        |_env, _result| {
            callback();
            Ok(())
        },
    ) {
        Status::Ok => Ok(()),
        _ => Err(MiniAppError::WebView(
            "TSFN call_with_return_value failed".to_string(),
        )),
    }
}
