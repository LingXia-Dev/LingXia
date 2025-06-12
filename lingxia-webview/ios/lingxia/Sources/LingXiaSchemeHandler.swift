import WebKit
import Foundation
import os.log
import CLingXiaFFI

/// Custom URL scheme handler for lx:// requests
/// This handler intercepts lx:// scheme requests and forwards them to the Rust layer
/// for processing, enabling the same asset serving logic as Android
class SchemeHandler: NSObject, WKURLSchemeHandler {
    private static let log = OSLog(subsystem: "LingXia", category: "SchemeHandler")
    private let appId: String

    init(appId: String) {
        self.appId = appId
        super.init()
        os_log("SchemeHandler initialized for appId: %@", log: Self.log, type: .info, appId)
    }

    func webView(_ webView: WKWebView, start urlSchemeTask: WKURLSchemeTask) {
        let request = urlSchemeTask.request

        guard let url = request.url?.absoluteString else {
            os_log("URLRequest missing URL", log: Self.log, type: .error)
            urlSchemeTask.didFailWithError(NSError(
                domain: "SchemeHandler",
                code: -1,
                userInfo: [NSLocalizedDescriptionKey: "Missing URL"]
            ))
            return
        }

        let method = request.httpMethod ?? "GET"

        // Convert headers to RustVec efficiently
        var headerKeys: [RustString] = []
        var headerValues: [RustString] = []
        if let requestHeaders = request.allHTTPHeaderFields {
            for (key, value) in requestHeaders {
                headerKeys.append(RustString(key))
                headerValues.append(RustString(value))
            }
            os_log("Added %d headers to request", log: Self.log, type: .debug, headerKeys.count)
        }
        
        // Get request body
        let body = request.httpBody ?? Data()
        
        // Create high-efficiency HttpRequest struct
        let headerKeysVec = RustVec<RustString>()
        for key in headerKeys {
            headerKeysVec.push(value: key)
        }
        
        let headerValuesVec = RustVec<RustString>()
        for value in headerValues {
            headerValuesVec.push(value: value)
        }
        
        let bodyVec = RustVec<UInt8>()
        for byte in Array(body) {
            bodyVec.push(value: byte)
        }
        
        let httpRequest = HttpRequest(
            url: RustString(url),
            method: RustString(method),
            header_keys: headerKeysVec,
            header_values: headerValuesVec,
            body: bodyVec
        )

        // Call Rust FFI with efficient struct
        if let httpResponse = handleRequest(appId, httpRequest) {
            // Convert headers back to dictionary from RustVec
            var responseHeaders: [String: String] = [:]
            let keys = Array(httpResponse.header_keys)
            let values = Array(httpResponse.header_values)
            for (key, value) in zip(keys, values) {
                responseHeaders[key.as_str().toString()] = value.as_str().toString()
            }
            
            // Create URLResponse directly
            let urlResponse = HTTPURLResponse(
                url: request.url!,
                statusCode: Int(httpResponse.status_code),
                httpVersion: "HTTP/1.1",
                headerFields: responseHeaders
            )!
            
            // Create body data directly from RustVec<u8>
            let bodyData = Data(Array(httpResponse.body))

            // Send response to WebView
            urlSchemeTask.didReceive(urlResponse)
            urlSchemeTask.didReceive(bodyData)
            urlSchemeTask.didFinish()
        } else {
            // Return 404 if Rust didn't handle the request
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 404,
                httpVersion: "HTTP/1.1",
                headerFields: ["Content-Type": "text/plain"]
            )!

            urlSchemeTask.didReceive(response)
            urlSchemeTask.didReceive("Not Found".data(using: .utf8) ?? Data())
            urlSchemeTask.didFinish()

            os_log("lx:// request not handled by Rust: %@", log: Self.log, type: .default, url)
        }
    }

    func webView(_ webView: WKWebView, stop urlSchemeTask: WKURLSchemeTask) {
        // Cleanup if needed
        os_log("Stopped lx:// request: %@", log: Self.log, type: .debug, urlSchemeTask.request.url?.absoluteString ?? "")
    }
}


