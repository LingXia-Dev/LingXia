import Foundation

// Rust FFI Function Declarations (from Rust ffi.rs)
// Keep these in sync with your Rust ffi.rs file.

// Assuming these are declared in Rust and part of the compiled library:
@_silgen_name("rust_miniapp_init")
private func rust_miniapp_init(_ data_dir: UnsafePointer<CChar>?, _ cache_dir: UnsafePointer<CChar>?, _ app_delegate: UnsafeMutableRawPointer?) -> UnsafeMutablePointer<CChar>?

@_silgen_name("rust_webview_attached")
private func rust_webview_attached(_ appid: UnsafePointer<CChar>?, _ path: UnsafePointer<CChar>?) -> Int32

@_silgen_name("rust_handle_post_message")
private func rust_handle_post_message(_ appid: UnsafePointer<CChar>?, _ path: UnsafePointer<CChar>?, _ message: UnsafePointer<CChar>?, _ message_len: Int) -> Int32

@_silgen_name("rust_page_started")
private func rust_page_started(_ appid: UnsafePointer<CChar>?, _ path: UnsafePointer<CChar>?) -> Int32

@_silgen_name("rust_page_finished")
private func rust_page_finished(_ appid: UnsafePointer<CChar>?, _ path: UnsafePointer<CChar>?) -> Int32

@_silgen_name("rust_page_show")
private func rust_page_show(_ appid: UnsafePointer<CChar>?, _ path: UnsafePointer<CChar>?)

@_silgen_name("rust_should_override_url_loading")
private func rust_should_override_url_loading(_ appid: UnsafePointer<CChar>?, _ url: UnsafePointer<CChar>?) -> Int32

@_silgen_name("rust_find_webview")
private func rust_find_webview(_ appid: UnsafePointer<CChar>?, _ path: UnsafePointer<CChar>?) -> UnsafeMutableRawPointer?

@_silgen_name("rust_handle_request")
private func rust_handle_request(_ appid: UnsafePointer<CChar>?, _ url: UnsafePointer<CChar>?, _ method: UnsafePointer<CChar>?, _ headers: UnsafePointer<CChar>?, _ body: UnsafePointer<UInt8>?, _ body_len: Int) -> UnsafeMutablePointer<CChar>?

@_silgen_name("rust_miniapp_closed")
private func rust_miniapp_closed(_ appid: UnsafePointer<CChar>?) -> Int32

@_silgen_name("rust_console_message")
private func rust_console_message(_ appid: UnsafePointer<CChar>?, _ path: UnsafePointer<CChar>?, _ level: Int32, _ message: UnsafePointer<CChar>?) -> Int32

@_silgen_name("rust_get_page_config")
private func rust_get_page_config(_ appid: UnsafePointer<CChar>?, _ path: UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>?

@_silgen_name("rust_back_pressed")
private func rust_back_pressed(_ appid: UnsafePointer<CChar>?) -> Int32

@_silgen_name("rust_miniapp_opened")
private func rust_miniapp_opened(_ appid: UnsafePointer<CChar>?, _ path: UnsafePointer<CChar>?) -> Int32

@_silgen_name("rust_get_tab_bar_config")
private func rust_get_tab_bar_config(_ appid: UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>?

@_silgen_name("rust_scroll_changed")
private func rust_scroll_changed(_ appid: UnsafePointer<CChar>?, _ path: UnsafePointer<CChar>?, _ scroll_x: Int32, _ scroll_y: Int32, _ max_scroll_x: Int32, _ max_scroll_y: Int32) -> Int32

@_silgen_name("rust_free_string")
private func rust_free_string(_ ptr: UnsafeMutablePointer<CChar>?)

public class Rust {

    public static func miniappInit(dataDir: String, cacheDir: String, appDelegate: UnsafeMutableRawPointer?) -> String? {
        let resultPtr = rust_miniapp_init(dataDir, cacheDir, appDelegate)
        if let result = resultPtr {
            let resultString = String(cString: result)
            rust_free_string(result) // Crucial: free the string Rust allocated
            return resultString
        }
        return nil
    }

    public static func webviewAttached(appid: String, path: String) -> Int32 {
        return rust_webview_attached(appid, path)
    }

    public static func handlePostMessage(appid: String, path: String, message: String) -> Int32 {
        let messageData = message.data(using: .utf8)
        let messageLen = messageData?.count ?? 0
        // Use withUnsafeBytes to get a pointer to the underlying bytes
        return messageData?.withUnsafeBytes { (bytes: UnsafeRawBufferPointer) -> Int32 in
            // Ensure the memory is bound to CChar (Int8) if that's what Rust expects
            let charPtr = bytes.baseAddress?.assumingMemoryBound(to: CChar.self)
            return rust_handle_post_message(appid, path, charPtr, messageLen)
        } ?? rust_handle_post_message(appid, path, nil, 0) // Fallback if messageData is nil
    }

    public static func handlePostMessage(appid: String, path: String, message: Data) -> Int32 {
        let messageLen = message.count
        return message.withUnsafeBytes { (bytes: UnsafeRawBufferPointer) -> Int32 in
            let charPtr = bytes.baseAddress?.assumingMemoryBound(to: CChar.self)
            return rust_handle_post_message(appid, path, charPtr, messageLen)
        }
    }

    public static func pageStarted(appid: String, path: String) -> Int32 {
        return rust_page_started(appid, path)
    }

    public static func pageFinished(appid: String, path: String) -> Int32 {
        return rust_page_finished(appid, path)
    }

    public static func pageShow(appid: String, path: String) {
        rust_page_show(appid, path)
    }

    public static func shouldOverrideUrlLoading(appid: String, url: String) -> Bool {
        // Rust returns Int32, convert to Bool
        return rust_should_override_url_loading(appid, url) != 0
    }

    public static func findWebview(appid: String, path: String) -> UnsafeMutableRawPointer? {
        return rust_find_webview(appid, path)
    }

    public static func handleRequest(appid: String, url: String, method: String, headers: [String: String], body: Data?) -> String? {
        let headersData = try? JSONSerialization.data(withJSONObject: headers, options: [])
        // Ensure headersString is valid JSON, even if serialization fails or is empty
        let headersString = headersData != nil ? String(data: headersData!, encoding: .utf8) ?? "{}" : "{}"

        var resultPtr: UnsafeMutablePointer<CChar>?
        if let body = body, !body.isEmpty {
            let bodyLen = body.count
            body.withUnsafeBytes { (bytes: UnsafeRawBufferPointer) -> Void in
                // Assuming Rust expects UInt8 for body bytes
                let bytePtr = bytes.baseAddress?.assumingMemoryBound(to: UInt8.self)
                resultPtr = rust_handle_request(appid, url, method, headersString, bytePtr, bodyLen)
            }
        } else {
            resultPtr = rust_handle_request(appid, url, method, headersString, nil, 0)
        }

        if let result = resultPtr {
            let resultString = String(cString: result)
            rust_free_string(result)
            return resultString
        }
        return nil
    }

    // Convenience overload for URLRequest
    public static func handleRequest(request: URLRequest, appid: String) -> String? {
        guard let url = request.url?.absoluteString, let method = request.httpMethod else {
            return nil
        }
        let headers = request.allHTTPHeaderFields ?? [:]
        let body = request.httpBody

        return handleRequest(appid: appid, url: url, method: method, headers: headers, body: body)
    }

    public static func miniappClosed(appid: String) -> Int32 {
        return rust_miniapp_closed(appid)
    }

    public static func consoleMessage(appid: String, path: String, level: Int32, message: String) -> Int32 {
        return rust_console_message(appid, path, level, message)
    }

    public static func getPageConfig(appid: String, path: String) -> String? {
        let resultPtr = rust_get_page_config(appid, path)
        if let result = resultPtr {
            let resultString = String(cString: result)
            rust_free_string(result)
            return resultString
        }
        return nil
    }

    public static func backPressed(appid: String) -> Bool {
        return rust_back_pressed(appid) != 0
    }

    public static func miniappOpened(appid: String, path: String) -> Int32 {
        return rust_miniapp_opened(appid, path)
    }

    public static func getTabBarConfig(appid: String) -> String? {
        let resultPtr = rust_get_tab_bar_config(appid)
        if let result = resultPtr {
            let resultString = String(cString: result)
            rust_free_string(result)
            return resultString
        }
        return nil
    }

    public static func scrollChanged(appid: String, path: String, scrollX: Int32, scrollY: Int32, maxScrollX: Int32, maxScrollY: Int32) -> Int32 {
        return rust_scroll_changed(appid, path, scrollX, scrollY, maxScrollX, maxScrollY)
    }

    // Public wrapper for freeing strings, if needed by Swift callers for other Rust-allocated strings.
    public static func freeString(ptr: UnsafeMutablePointer<CChar>?) {
        rust_free_string(ptr)
    }
}
