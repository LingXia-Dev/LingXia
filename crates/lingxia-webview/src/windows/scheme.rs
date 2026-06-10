//! Custom-scheme/web-resource responses and in-memory HTML pages.

use super::*;

pub(crate) fn prepare_navigation_html(html: &str, base_url: &str, navigation_url: &str) -> Vec<u8> {
    if navigation_url == base_url {
        return html.as_bytes().to_vec();
    }

    inject_base_url(html, base_url).into_bytes()
}

pub(crate) fn store_memory_page(
    memory_pages: &Arc<Mutex<HashMap<String, Vec<u8>>>>,
    url: &str,
    html: Vec<u8>,
) {
    if let Ok(mut pages) = memory_pages.lock() {
        pages.insert(normalize_memory_page_url(url), html);
    }
}

pub(crate) fn clear_memory_pages(memory_pages: &Arc<Mutex<HashMap<String, Vec<u8>>>>) {
    if let Ok(mut pages) = memory_pages.lock() {
        pages.clear();
    }
}

pub(crate) fn find_memory_page(
    memory_pages: &Arc<Mutex<HashMap<String, Vec<u8>>>>,
    url: &str,
) -> Option<Vec<u8>> {
    memory_pages
        .lock()
        .ok()
        .and_then(|pages| pages.get(&normalize_memory_page_url(url)).cloned())
}

pub(crate) fn normalize_memory_page_url(url: &str) -> String {
    url.trim().trim_end_matches('/').to_string()
}

pub(crate) fn request_scheme(url: &str) -> &str {
    url.split_once(':')
        .map(|(scheme, _)| scheme)
        .unwrap_or_default()
}

pub(crate) fn build_memory_html_response(
    env: &ICoreWebView2Environment,
    html: Vec<u8>,
) -> WinResult<ICoreWebView2WebResourceResponse> {
    let response = http::Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "text/html; charset=utf-8")
        .header("access-control-allow-origin", "null")
        .body(html)
        .map_err(http_error_to_win)?;
    let (parts, body) = response.into_parts();
    build_webview2_response(env, (parts, body).into())
}

pub(crate) fn inject_base_url(html: &str, base_url: &str) -> String {
    let base_tag = format!(r#"<base href="{}">"#, html_escape(base_url));
    let lower = html.to_lowercase();

    if let Some(pos) = lower.find("</head>") {
        let (before, after) = html.split_at(pos);
        return format!("{before}{base_tag}{after}");
    }

    if let Some(pos) = lower.find("<body")
        && let Some(end) = html[pos..].find('>')
    {
        let insert = pos + end + 1;
        let (before, after) = html.split_at(insert);
        return format!("{before}{base_tag}{after}");
    }

    format!("{base_tag}{html}")
}

pub(crate) fn html_escape(value: &str) -> String {
    value.replace('&', "&amp;").replace('"', "&quot;")
}

pub(crate) fn build_webview2_response(
    env: &ICoreWebView2Environment,
    response: WebResourceResponse,
) -> WinResult<ICoreWebView2WebResourceResponse> {
    let (parts, body) = response.into_parts();
    let bytes = materialize_body(body);
    let stream = body_to_stream(&bytes)?;
    let reason = CoTaskMemPWSTR::from(canonical_reason(parts.status).as_str());
    let headers = CoTaskMemPWSTR::from(format_headers(&parts.headers).as_str());

    unsafe {
        env.CreateWebResourceResponse(
            Some(&stream),
            parts.status.as_u16() as i32,
            *reason.as_ref().as_pcwstr(),
            *headers.as_ref().as_pcwstr(),
        )
    }
}

pub(crate) fn materialize_body(body: WebResourceBody) -> Vec<u8> {
    match body {
        WebResourceBody::Bytes(bytes) => bytes,
        WebResourceBody::Path(path) => std::fs::read(path).unwrap_or_default(),
        WebResourceBody::Pipe(reader) => {
            let mut data = Vec::new();
            let mut file = pipe_reader_to_file(reader);
            let _ = file.as_mut().map(|file| file.read_to_end(&mut data));
            data
        }
    }
}

pub(crate) fn pipe_reader_to_file(reader: crate::SystemPipeReader) -> Option<std::fs::File> {
    #[cfg(unix)]
    {
        Some(reader.into_file())
    }
    #[cfg(windows)]
    {
        Some(reader.into_file())
    }
    #[cfg(all(not(unix), not(windows)))]
    {
        let _ = reader;
        None
    }
}

pub(crate) fn body_to_stream(bytes: &[u8]) -> WinResult<IStream> {
    unsafe { SHCreateMemStream(Some(bytes)).ok_or_else(windows::core::Error::from_thread) }
}

pub(crate) fn format_headers(headers: &http::HeaderMap) -> String {
    let mut result = String::new();
    for (name, value) in headers {
        if let Ok(value) = value.to_str() {
            result.push_str(name.as_str());
            result.push_str(": ");
            result.push_str(value);
            result.push_str("\r\n");
        }
    }
    result
}

pub(crate) fn populate_request_headers(
    request: &ICoreWebView2WebResourceRequest,
    headers: &mut http::HeaderMap,
) -> WinResult<()> {
    let native_headers = unsafe { request.Headers()? };
    let iterator = unsafe { native_headers.GetIterator()? };
    let mut has_current = BOOL::default();
    unsafe {
        iterator.HasCurrentHeader(&mut has_current)?;
    }

    while has_current.as_bool() {
        let mut name = PWSTR::null();
        let mut value = PWSTR::null();
        unsafe {
            iterator.GetCurrentHeader(&mut name, &mut value)?;
        }

        let name = CoTaskMemPWSTR::from(name).to_string();
        let value = CoTaskMemPWSTR::from(value).to_string();
        if let (Ok(header_name), Ok(header_value)) = (
            name.parse::<http::header::HeaderName>(),
            value.parse::<http::header::HeaderValue>(),
        ) {
            headers.append(header_name, header_value);
        }

        let mut has_next = BOOL::default();
        unsafe {
            iterator.MoveNext(&mut has_next)?;
        }
        has_current = has_next;
    }

    Ok(())
}

pub(crate) fn canonical_reason(status: StatusCode) -> String {
    status.canonical_reason().unwrap_or("OK").to_string()
}

pub(crate) fn not_found_response() -> WebResourceResponse {
    let response = http::Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header("content-type", "text/plain; charset=utf-8")
        .body(b"Not Found".to_vec())
        .expect("failed to build fallback response");
    response.into_parts().into()
}

pub(crate) fn http_error_to_win(err: http::Error) -> windows::core::Error {
    windows::core::Error::new(E_POINTER, format!("{err}"))
}
