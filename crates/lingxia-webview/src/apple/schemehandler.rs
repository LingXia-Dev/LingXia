use crate::webview::{WebTag, find_webview};
use crate::{WebResourceBody, WebResourceResponse};
use dispatch2::DispatchQueue;
use objc2::runtime::{AnyObject, NSObject, Sel};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained};
use objc2_foundation::{NSData, NSMutableDictionary, NSObjectProtocol, NSString};
use objc2_web_kit::WKURLSchemeHandler;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
#[cfg(unix)]
use std::os::fd::FromRawFd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
#[cfg(unix)]
use std::time::Duration;

#[inline]
unsafe fn nsdata_bytes_ptr_unchecked(ns_data: *mut AnyObject) -> *const u8 {
    // Avoid objc2's debug-time method signature verification for `-[NSData bytes]`.
    //
    // On some Apple OS versions, the runtime type encoding for this method can
    // differ from what headers declare, causing a panic in debug builds.
    let sel: Sel = objc2::sel!(bytes);
    let func: unsafe extern "C" fn(*mut AnyObject, Sel) -> *const core::ffi::c_void =
        unsafe { core::mem::transmute(objc2::ffi::objc_msgSend as *const ()) };
    unsafe { func(ns_data, sel) }.cast()
}

fn pipe_task_registry() -> &'static Mutex<HashMap<usize, Arc<AtomicBool>>> {
    static REGISTRY: OnceLock<Mutex<HashMap<usize, Arc<AtomicBool>>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn register_pipe_task(task: *mut AnyObject) -> Arc<AtomicBool> {
    let key = task as usize;
    let flag = Arc::new(AtomicBool::new(false));
    if let Ok(mut guard) = pipe_task_registry().lock() {
        guard.insert(key, flag.clone());
    }
    flag
}

fn cancel_pipe_task(task: *mut AnyObject) {
    let key = task as usize;
    if let Ok(mut guard) = pipe_task_registry().lock()
        && let Some(flag) = guard.remove(&key)
    {
        flag.store(true, Ordering::Release);
    }
}

fn remove_pipe_task(task_key: usize) {
    if let Ok(mut guard) = pipe_task_registry().lock() {
        guard.remove(&task_key);
    }
}

fn run_task_message(context: &str, f: impl FnOnce()) -> bool {
    match objc2::exception::catch(std::panic::AssertUnwindSafe(f)) {
        Ok(()) => true,
        Err(exception) => {
            log::warn!(
                "Ignored ObjC exception while handling WKURLSchemeTask {}: {:?}",
                context,
                exception
            );
            false
        }
    }
}

unsafe fn task_did_receive_response(task: *mut AnyObject, response: *mut AnyObject) -> bool {
    run_task_message("didReceiveResponse", || unsafe {
        let _: () = msg_send![task, didReceiveResponse: response];
    })
}

unsafe fn task_did_receive_data(task: *mut AnyObject, data: &NSData) -> bool {
    run_task_message("didReceiveData", || unsafe {
        let _: () = msg_send![task, didReceiveData: data];
    })
}

unsafe fn task_did_finish(task: *mut AnyObject) -> bool {
    run_task_message("didFinish", || unsafe {
        let _: () = msg_send![task, didFinish];
    })
}
// Define ivars struct first
#[derive(Debug)]
pub(super) struct LingXiaSchemeHandlerIvars {
    webtag: WebTag,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "LingXiaSchemeHandler"]
    #[thread_kind = MainThreadOnly]
    #[ivars = LingXiaSchemeHandlerIvars]
    pub(super) struct LingXiaSchemeHandler;

    unsafe impl NSObjectProtocol for LingXiaSchemeHandler {}

    unsafe impl WKURLSchemeHandler for LingXiaSchemeHandler {
        #[unsafe(method(webView:startURLSchemeTask:))]
        fn start_url_scheme_task(&self, _webview: *mut AnyObject, task: *mut AnyObject) {
            if task.is_null() {
                log::error!("Task is null!");
                return;
            }

            unsafe {
                // Extract request information from the task
                let request: *mut AnyObject = msg_send![task, request];
                if request.is_null() {
                    log::error!("Request is null");
                    self.fail_task_with_error(task, "Request is null");
                    return;
                }

                // Get URL string
                let url_obj: *mut AnyObject = msg_send![request, URL];
                if url_obj.is_null() {
                    log::error!("URL object is null");
                    self.fail_task_with_error(task, "URL object is null");
                    return;
                }

                let url_string: *mut AnyObject = msg_send![url_obj, absoluteString];
                if url_string.is_null() {
                    log::error!("URL string is null");
                    self.fail_task_with_error(task, "URL string is null");
                    return;
                }

                let url_cstring: *const std::ffi::c_char = msg_send![url_string, UTF8String];
                if url_cstring.is_null() {
                    log::error!("URL C string is null");
                    self.fail_task_with_error(task, "URL C string is null");
                    return;
                }

                let url = std::ffi::CStr::from_ptr(url_cstring)
                    .to_string_lossy()
                    .to_string();

                // Get HTTP method
                let method_obj: *mut AnyObject = msg_send![request, HTTPMethod];
                let method = if method_obj.is_null() {
                    log::warn!("HTTP method is null, defaulting to GET");
                    "GET".to_string()
                } else {
                    let method_cstring: *const std::ffi::c_char = msg_send![method_obj, UTF8String];
                    if method_cstring.is_null() {
                        log::warn!("HTTP method C string is null, defaulting to GET");
                        "GET".to_string()
                    } else {
                        std::ffi::CStr::from_ptr(method_cstring)
                            .to_string_lossy()
                            .to_string()
                    }
                };

                // Extract scheme from URL
                let scheme = url.split("://").next().unwrap_or("").to_string();

                log::info!("Processing {}:// request: {} {}", scheme, method, url);

                // Get request body
                let mut body = vec![];
                let body_data: *mut AnyObject = msg_send![request, HTTPBody];
                if !body_data.is_null() {
                    let length: usize = msg_send![body_data, length];
                    if length > 0 {
                        let bytes_ptr: *const u8 = nsdata_bytes_ptr_unchecked(body_data);
                        if !bytes_ptr.is_null() {
                            body = std::slice::from_raw_parts(bytes_ptr, length).to_vec();
                        }
                    }
                }

                // Create http::Request and add headers directly (more efficient)
                let mut builder = http::Request::builder()
                    .method(method.as_str())
                    .uri(url.as_str());

                // Add headers directly to builder without intermediate storage
                let headers_dict: *mut AnyObject = msg_send![request, allHTTPHeaderFields];
                if !headers_dict.is_null() {
                    let keys_array: *mut AnyObject = msg_send![headers_dict, allKeys];
                    let values_array: *mut AnyObject = msg_send![headers_dict, allValues];

                    if !keys_array.is_null() && !values_array.is_null() {
                        let count: usize = msg_send![keys_array, count];

                        for i in 0..count {
                            let key_obj: *mut AnyObject = msg_send![keys_array, objectAtIndex: i];
                            let value_obj: *mut AnyObject =
                                msg_send![values_array, objectAtIndex: i];

                            if !key_obj.is_null() && !value_obj.is_null() {
                                let key_cstring: *const std::ffi::c_char =
                                    msg_send![key_obj, UTF8String];
                                let value_cstring: *const std::ffi::c_char =
                                    msg_send![value_obj, UTF8String];

                                if !key_cstring.is_null() && !value_cstring.is_null() {
                                    let key =
                                        std::ffi::CStr::from_ptr(key_cstring).to_string_lossy();
                                    let value =
                                        std::ffi::CStr::from_ptr(value_cstring).to_string_lossy();

                                    if let (Ok(header_name), Ok(header_value)) = (
                                        key.parse::<http::header::HeaderName>(),
                                        value.parse::<http::header::HeaderValue>(),
                                    ) {
                                        builder = builder.header(header_name, header_value);
                                    }
                                }
                            }
                        }
                    }
                }

                let http_request = match builder.body(body) {
                    Ok(req) => req,
                    Err(e) => {
                        log::error!("Failed to build HTTP request: {}", e);
                        self.fail_task_with_error(task, "Failed to build HTTP request");
                        return;
                    }
                };

                // Dispatch to closure-based scheme handler
                let webtag = &self.ivars().webtag;
                let response = if let Some(webview) = find_webview(webtag) {
                    webview.handle_scheme_request(&scheme, http_request)
                } else {
                    None
                };
                match response {
                    Some(http_response) => {
                        log::debug!(
                            "Got response from scheme handler, status: {}",
                            http_response.parts().status,
                        );
                        self.send_response_to_task(task, http_response, request);
                    }
                    None => {
                        log::warn!("Scheme handler returned None, sending 404");
                        self.send_404_response(task, request);
                    }
                }
            }
        }

        #[unsafe(method(webView:stopURLSchemeTask:))]
        fn stop_url_scheme_task(&self, _webview: *mut AnyObject, task: *mut AnyObject) {
            cancel_pipe_task(task);
            log::debug!("Stopped lx:// request");
        }
    }
);

impl LingXiaSchemeHandler {
    /// Create a new LingXiaSchemeHandler bound to a specific WebView
    pub(super) fn new(webtag: WebTag) -> Option<Retained<Self>> {
        log::debug!(
            "Creating LingXiaSchemeHandler for webtag: {}",
            webtag.as_str()
        );

        let mtm = match MainThreadMarker::new() {
            Some(marker) => marker,
            None => {
                log::error!("Not on main thread when creating scheme handler");
                return None;
            }
        };

        unsafe {
            let instance = Self::alloc(mtm);
            let instance = instance.set_ivars(LingXiaSchemeHandlerIvars { webtag });
            let instance: Retained<Self> = msg_send![super(instance), init];
            log::debug!("Successfully created LingXiaSchemeHandler");
            Some(instance)
        }
    }

    /// Send a successful response to the URL scheme task
    fn send_response_to_task(
        &self,
        task: *mut AnyObject,
        response: WebResourceResponse,
        request: *mut AnyObject,
    ) {
        unsafe {
            let (parts, body) = response.into_parts();
            let status = parts.status;
            let mut headers_map = parts.headers.clone();

            let url_obj: *mut AnyObject = msg_send![request, URL];
            let status_code = status.as_u16() as i64;
            let http_version = NSString::from_str("HTTP/1.1");
            let headers = NSMutableDictionary::new();

            let content_type = headers_map
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("application/octet-stream");

            let content_type_key = NSString::from_str("Content-Type");
            let content_type_with_charset = if content_type == "text/html" {
                "text/html; charset=UTF-8"
            } else {
                content_type
            };
            let content_type_value = NSString::from_str(content_type_with_charset);
            headers.insert(&*content_type_key, &*content_type_value);

            if !headers_map.contains_key("content-length") {
                let content_len = match &body {
                    WebResourceBody::Path(path) => std::fs::metadata(path).ok().map(|m| m.len()),
                    WebResourceBody::Bytes(data) => Some(data.len() as u64),
                    WebResourceBody::Pipe(_) => None,
                };
                if let Some(len) = content_len
                    && let Ok(value) = http::HeaderValue::from_str(&len.to_string())
                {
                    headers_map.insert(http::header::CONTENT_LENGTH, value);
                }
            }

            for (key, value) in headers_map.iter() {
                if let Ok(value_str) = value.to_str() {
                    let key_ns = NSString::from_str(key.as_str());
                    let value_ns = NSString::from_str(value_str);
                    headers.insert(&*key_ns, &*value_ns);
                }
            }

            let response_class: *mut AnyObject = msg_send![objc2::class!(NSHTTPURLResponse), alloc];
            let response_result: *mut AnyObject = msg_send![response_class,
            initWithURL: url_obj,
            statusCode: status_code,
            HTTPVersion: &*http_version,
            headerFields: &*headers];

            if response_result.is_null() {
                log::error!("Failed to create NSHTTPURLResponse using msg_send approach!");
                self.fail_task_with_error(task, "Failed to create HTTP response");
                return;
            }

            if !task_did_receive_response(task, response_result) {
                return;
            }

            match body {
                WebResourceBody::Path(path) => {
                    let mut reader: Box<dyn std::io::Read> = match File::open(&path) {
                        Ok(file) => Box::new(file),
                        Err(e) => {
                            log::error!("Failed to open response file {}: {}", path.display(), e);
                            self.fail_task_with_error(task, "Failed to open response file");
                            return;
                        }
                    };

                    let mut buffer = [0u8; 64 * 1024];
                    loop {
                        match reader.read(&mut buffer) {
                            Ok(0) => {
                                let _ = task_did_finish(task);
                                break;
                            }
                            Ok(read_bytes) => {
                                let chunk = NSData::from_vec(buffer[..read_bytes].to_vec());
                                if !task_did_receive_data(task, &chunk) {
                                    break;
                                }
                            }
                            Err(e) => {
                                log::error!("Failed while streaming response data: {}", e);
                                self.fail_task_with_error(task, "Failed to read response data");
                                return;
                            }
                        }
                    }
                }
                WebResourceBody::Pipe(pipe) => {
                    #[cfg(unix)]
                    {
                        use std::net::Shutdown;

                        let mut reader = {
                            let fd = pipe.into_raw_fd();
                            std::os::unix::net::UnixStream::from_raw_fd(fd)
                        };
                        let _ = reader.set_read_timeout(Some(Duration::from_millis(200)));

                        let retained_task: *mut AnyObject = msg_send![task, retain];
                        let task_key = retained_task as usize;
                        let cancel_flag = register_pipe_task(retained_task);

                        std::thread::spawn(move || {
                            let mut buffer = [0u8; 64 * 1024];
                            loop {
                                if cancel_flag.load(Ordering::Acquire) {
                                    break;
                                }

                                match reader.read(&mut buffer) {
                                    Ok(0) => {
                                        DispatchQueue::main().exec_sync(move || {
                                            let task_ptr = task_key as *mut AnyObject;
                                            let _ = task_did_finish(task_ptr);
                                            let _: () = msg_send![task_ptr, release];
                                        });
                                        remove_pipe_task(task_key);
                                        return;
                                    }
                                    Ok(read_bytes) => {
                                        let chunk = buffer[..read_bytes].to_vec();
                                        DispatchQueue::main().exec_sync(move || {
                                            let task_ptr = task_key as *mut AnyObject;
                                            let data = NSData::from_vec(chunk);
                                            let _ = task_did_receive_data(task_ptr, &data);
                                        });
                                    }
                                    Err(e)
                                        if e.kind() == std::io::ErrorKind::WouldBlock
                                            || e.kind() == std::io::ErrorKind::TimedOut =>
                                    {
                                        continue;
                                    }
                                    Err(e) => {
                                        log::error!(
                                            "Failed while streaming response data asynchronously: {}",
                                            e
                                        );
                                        break;
                                    }
                                }
                            }

                            let _ = reader.shutdown(Shutdown::Both);
                            remove_pipe_task(task_key);
                            DispatchQueue::main().exec_sync(move || {
                                let task_ptr = task_key as *mut AnyObject;
                                let _: () = msg_send![task_ptr, release];
                            });
                        });
                    }
                    #[cfg(not(unix))]
                    {
                        log::error!("Pipe bodies are not supported on this platform");
                        self.fail_task_with_error(task, "Pipe body unsupported");
                        return;
                    }
                }
                WebResourceBody::Bytes(data) => {
                    if !data.is_empty() {
                        let chunk = NSData::from_vec(data);
                        if !task_did_receive_data(task, &chunk) {
                            return;
                        }
                    }
                    let _ = task_did_finish(task);
                }
            }
        }
    }

    /// Send a 404 response to the URL scheme task
    fn send_404_response(&self, task: *mut AnyObject, request: *mut AnyObject) {
        unsafe {
            // Get the original request URL for creating the response
            let url_obj: *mut AnyObject = msg_send![request, URL];

            // Create a 404 HTTP response with proper HTTP version using wry method
            let status_code = 404i64;
            let http_version = NSString::from_str("HTTP/1.1");

            // Create headers dictionary for 404 response like Swift version
            let headers = NSMutableDictionary::new();
            let content_type_key = NSString::from_str("Content-Type");
            let content_type_value = NSString::from_str("text/plain");
            headers.insert(&*content_type_key, &*content_type_value);

            // Create 404 response using same approach as successful response
            let response_class: *mut AnyObject = msg_send![objc2::class!(NSHTTPURLResponse), alloc];
            let response_result: *mut AnyObject = msg_send![response_class,
                initWithURL: url_obj,
                statusCode: status_code,
                HTTPVersion: &*http_version,
                headerFields: &*headers];

            let body_data = NSData::from_vec("Not Found".as_bytes().to_vec());

            if !response_result.is_null() {
                // Send response to WebView - use correct method names from wry
                if task_did_receive_response(task, response_result) {
                    let _ = task_did_receive_data(task, &body_data);
                    let _ = task_did_finish(task);
                }
            } else {
                log::error!("Failed to create 404 response");
                let _ = task_did_finish(task);
            }
        }
    }

    /// Fail the task with an error
    fn fail_task_with_error(&self, task: *mut AnyObject, error_message: &str) {
        unsafe {
            // Create a proper error response with status code
            let error_html = format!(
                "<html><body><h1>Error</h1><p>{}</p></body></html>",
                error_message
            );
            let body_data = NSData::from_vec(error_html.as_bytes().to_vec());

            // Create a 500 error response
            let response_class: *mut AnyObject = msg_send![objc2::class!(NSHTTPURLResponse), alloc];
            let http_version = NSString::from_str("HTTP/1.1");
            let headers = NSMutableDictionary::new();

            let content_type_key = NSString::from_str("Content-Type");
            let content_type_value = NSString::from_str("text/html; charset=UTF-8");
            headers.insert(&*content_type_key, &*content_type_value);

            // Get URL from the task
            let request: *mut AnyObject = msg_send![task, request];
            let url_obj: *mut AnyObject = msg_send![request, URL];

            let response_result: *mut AnyObject = msg_send![response_class,
                initWithURL: url_obj,
                statusCode: 500i64,
                HTTPVersion: &*http_version,
                headerFields: &*headers];

            if !response_result.is_null() {
                if task_did_receive_response(task, response_result) {
                    let _ = task_did_receive_data(task, &body_data);
                    let _ = task_did_finish(task);
                }
            } else {
                // Last resort - just send data and finish
                let _ = task_did_receive_data(task, &body_data);
                let _ = task_did_finish(task);
            }

            log::error!("SchemeHandler error: {}", error_message);
        }
    }
}
