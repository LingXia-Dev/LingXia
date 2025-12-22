package com.lingxia.webview;

import android.util.Log;
import android.webkit.WebResourceError;
import android.webkit.WebResourceRequest;
import android.webkit.WebResourceResponse;
import android.webkit.WebView;
import android.webkit.WebViewClient;
import android.os.ParcelFileDescriptor;
import java.io.ByteArrayInputStream;
import java.io.File;
import java.io.FileInputStream;
import java.io.FileNotFoundException;
import java.io.InputStream;
import java.lang.ref.WeakReference;

/**
 * WebViewClient implementation for LingXia WebView
 */
public class LingXiaWebViewClient extends WebViewClient {
    private static final String TAG = "LingXiaWebViewClient";
    private final WeakReference<LingXiaWebView> webViewRef;

    public LingXiaWebViewClient(LingXiaWebView webView) {
        this.webViewRef = new WeakReference<>(webView);
    }

    @Override
    public void onPageStarted(WebView view, String url, android.graphics.Bitmap favicon) {
        super.onPageStarted(view, url, favicon);
        Log.d(TAG, "Page started loading: " + url);

        LingXiaWebView webView = webViewRef.get();
        if (webView != null) {
            webView.setPageLoaded(false);
            webView.onPageStarted(
                webView.getAppId() != null ? webView.getAppId() : "",
                webView.getCurrentPath() != null ? webView.getCurrentPath() : ""
            );
        }
    }

    @Override
    public void onPageFinished(WebView view, String url) {
        super.onPageFinished(view, url);
        Log.d(TAG, "Page finished loading: " + url);

        LingXiaWebView webView = webViewRef.get();
        if (webView != null) {
            webView.setPageLoaded(true);
            webView.resetViewport();
            webView.onPageFinished(
                webView.getAppId() != null ? webView.getAppId() : "",
                webView.getCurrentPath() != null ? webView.getCurrentPath() : ""
            );
        }
    }

    @Override
    public boolean shouldOverrideUrlLoading(WebView view, WebResourceRequest request) {
        if (request != null && request.getUrl() != null) {
            String url = request.getUrl().toString();
            Log.d(TAG, "Should override URL loading: " + url);

            // Extract scheme from URL
            String scheme = "";
            int schemeEnd = url.indexOf("://");
            if (schemeEnd > 0) {
                scheme = url.substring(0, schemeEnd);
            } else {
                return false; // Invalid URL, don't override
            }

            // Handle lingxia scheme or block non-https schemes
            switch (scheme) {
                case "lx":
                    return true; // Always intercept lingxia scheme
                case "https":
                    return false; // Allow https URLs
                default:
                    return true; // Block all other schemes
            }
        }
        return false;
    }

    @Override
    public void onReceivedError(WebView view, WebResourceRequest request, WebResourceError error) {
        super.onReceivedError(view, request, error);
        Log.e(TAG, "Error loading page: " + error.getDescription() +
              ", code: " + error.getErrorCode() +
              ", failing URL: " + (request != null ? request.getUrl() : "unknown"));
    }

    @Override
    public WebResourceResponse shouldInterceptRequest(WebView view, WebResourceRequest request) {
        String url = request.getUrl().toString();
        String method = request.getMethod();

        // Convert headers to flat array: [key1, value1, key2, value2, ...]
        java.util.List<String> headerList = new java.util.ArrayList<>();
        try {
            for (java.util.Map.Entry<String, String> entry : request.getRequestHeaders().entrySet()) {
                headerList.add(entry.getKey());
                headerList.add(entry.getValue());
            }
        } catch (Exception e) {
            Log.e(TAG, "Error converting headers to array", e);
        }
        String[] headerArray = headerList.toArray(new String[0]);

        LingXiaWebView webView = webViewRef.get();
        if (webView != null) {
            // Call native to handle request
            LingXiaWebView.WebResourceResponseData response = webView.handleRequest(
                webView.getAppId() != null ? webView.getAppId() : "",
                webView.getCurrentPath() != null ? webView.getCurrentPath() : "",
                url,
                method,
                headerArray
            );

            if (response == null) {
                return null;
            }

            try {
                InputStream inputStream = null;
                if (response.pipeFd > 0) {
                    ParcelFileDescriptor pfd = ParcelFileDescriptor.adoptFd(response.pipeFd);
                    inputStream = new ParcelFileDescriptor.AutoCloseInputStream(pfd);
                } else if (response.data != null) {
                    inputStream = new ByteArrayInputStream(response.data);
                } else if (response.filePath != null && !response.filePath.isEmpty()) {
                    File file = new File(response.filePath);
                    inputStream = new FileInputStream(file);
                } else {
                    return null;
                }

                return new WebResourceResponse(
                    response.mimeType,
                    response.encoding,
                    response.statusCode,
                    response.reasonPhrase,
                    response.responseHeaders,
                    inputStream
                );
            } catch (FileNotFoundException e) {
                Log.e(TAG, "Failed to open intercepted body", e);
            }
        }

        return null;
    }
}
