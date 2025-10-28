use crate::webview::{WebTag, get_webview_delegate};
use crate::{WebResourceBody, WebResourceResponse};
use objc2::runtime::{AnyObject, NSObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained};
use objc2_foundation::{NSData, NSMutableDictionary, NSObjectProtocol, NSString};
use objc2_web_kit::WKURLSchemeHandler;
use std::fs::File;
use std::io::Read;
#[cfg(unix)]
use std::os::fd::FromRawFd;
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

                if !url.starts_with("lx://") {
                    log::warn!("Non-lx URL in scheme handler: {}", url);
                    self.fail_task_with_error(task, "Invalid scheme for handler");
                    return;
                }

                log::info!("Processing lx:// request: {} {}", method, url);

                // Get request body
                let mut body = vec![];
                let body_data: *mut AnyObject = msg_send![request, HTTPBody];
                if !body_data.is_null() {
                    let length: usize = msg_send![body_data, length];
                    if length > 0 {
                        let bytes_ptr: *const u8 = msg_send![body_data, bytes];
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

                // Use the bound webtag from this scheme handler
                let webtag = &self.ivars().webtag;
                let response = if let Some(delegate) = get_webview_delegate(webtag) {
                    delegate.handle_request(http_request)
                } else {
                    None
                };
                match response {
                    Some(http_response) => {
                        log::debug!(
                            "Got response from handle_request, status: {}",
                            http_response.parts().status,
                        );
                        self.send_response_to_task(task, http_response, request);
                    }
                    None => {
                        log::warn!("handle_request returned None, sending 404");
                        self.send_404_response(task, request);
                    }
                }
            }
        }

        #[unsafe(method(webView:stopURLSchemeTask:))]
        fn stop_url_scheme_task(&self, _webview: *mut AnyObject, _task: *mut AnyObject) {
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

            if let WebResourceBody::Path(ref path) = body {
                if !headers_map.contains_key("content-length") {
                    if let Ok(metadata) = std::fs::metadata(path) {
                        if let Ok(value) = http::HeaderValue::from_str(&metadata.len().to_string())
                        {
                            headers_map.insert(http::header::CONTENT_LENGTH, value);
                        }
                    }
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

            let _: () = msg_send![task, didReceiveResponse: response_result];

            let mut reader: Box<dyn std::io::Read> = match body {
                WebResourceBody::Path(path) => match File::open(&path) {
                    Ok(file) => Box::new(file),
                    Err(e) => {
                        log::error!("Failed to open response file {}: {}", path.display(), e);
                        self.fail_task_with_error(task, "Failed to open response file");
                        return;
                    }
                },
                WebResourceBody::Pipe(pipe) => {
                    #[cfg(unix)]
                    {
                        let fd = pipe.into_raw_fd();
                        let file = std::fs::File::from_raw_fd(fd);
                        Box::new(file)
                    }
                    #[cfg(not(unix))]
                    {
                        log::error!("Pipe bodies are not supported on this platform");
                        self.fail_task_with_error(task, "Pipe body unsupported");
                        return;
                    }
                }
            };

            let mut buffer = [0u8; 64 * 1024];
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) => {
                        let _: () = msg_send![task, didFinish];
                        break;
                    }
                    Ok(read_bytes) => {
                        let chunk = NSData::from_vec(buffer[..read_bytes].to_vec());
                        let _: () = msg_send![task, didReceiveData: &*chunk];
                    }
                    Err(e) => {
                        log::error!("Failed while streaming response data: {}", e);
                        self.fail_task_with_error(task, "Failed to read response data");
                        return;
                    }
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
                let _: () = msg_send![task, didReceiveResponse: response_result];
                let _: () = msg_send![task, didReceiveData: &*body_data];
                let _: () = msg_send![task, didFinish];
            } else {
                log::error!("Failed to create 404 response");
                let _: () = msg_send![task, didFinish];
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
                let _: () = msg_send![task, didReceiveResponse: response_result];
                let _: () = msg_send![task, didReceiveData: &*body_data];
                let _: () = msg_send![task, didFinish];
            } else {
                // Last resort - just send data and finish
                let _: () = msg_send![task, didReceiveData: &*body_data];
                let _: () = msg_send![task, didFinish];
            }

            log::error!("SchemeHandler error: {}", error_message);
        }
    }
}
