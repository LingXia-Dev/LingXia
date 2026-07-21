package com.lingxia.webview;

import android.os.Build;
import android.util.Log;
import android.webkit.WebResourceError;
import android.webkit.WebResourceRequest;
import android.webkit.WebResourceResponse;
import android.webkit.WebView;
import android.webkit.WebViewClient;
import androidx.annotation.RequiresApi;
import android.os.ParcelFileDescriptor;
import java.io.BufferedInputStream;
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
                webView.getCurrentPath() != null ? webView.getCurrentPath() : "",
                webView.getSessionId(),
                url != null ? url : ""
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
            webView.pushWebViewState();
            webView.onPageFinished(
                webView.getAppId() != null ? webView.getAppId() : "",
                webView.getCurrentPath() != null ? webView.getCurrentPath() : "",
                webView.getSessionId(),
                url != null ? url : ""
            );
        }
    }

    @Override
    public void onPageCommitVisible(WebView view, String url) {
        super.onPageCommitVisible(view, url);
        // Commit evidence: the displayed document was replaced.
        LingXiaWebView webView = webViewRef.get();
        if (webView != null) {
            webView.onPageCommitted(
                webView.getAppId() != null ? webView.getAppId() : "",
                webView.getCurrentPath() != null ? webView.getCurrentPath() : "",
                webView.getSessionId()
            );
        }
    }

    @Override
    public void doUpdateVisitedHistory(WebView view, String url, boolean isReload) {
        super.doUpdateVisitedHistory(view, url, isReload);
        // Fires for committed navigations, redirects, and History API updates —
        // the Android signal for "URL / back-forward state changed".
        LingXiaWebView webView = webViewRef.get();
        if (webView != null) {
            webView.pushWebViewState();
        }
    }

    @Override
    public boolean shouldOverrideUrlLoading(WebView view, WebResourceRequest request) {
        if (request == null || request.getUrl() == null) return false;
        String url = request.getUrl().toString();
        Log.d(TAG, "Should override URL loading: " + url);

        LingXiaWebView webView = webViewRef.get();
        if (webView != null) {
            return webView.handleNavigationPolicy(
                webView.getAppId() != null ? webView.getAppId() : "",
                webView.getCurrentPath() != null ? webView.getCurrentPath() : "",
                webView.getSessionId(),
                url,
                Build.VERSION.SDK_INT >= Build.VERSION_CODES.N && request.hasGesture(),
                request.isForMainFrame()
            );
        }
        return false;
    }

    private void reportMainFrameLoadError(String failingUrl, int errorCode, String description) {
        LingXiaWebView webView = webViewRef.get();
        if (webView == null) {
            return;
        }
        webView.setPageLoaded(false);
        webView.onLoadError(
            webView.getAppId() != null ? webView.getAppId() : "",
            webView.getCurrentPath() != null ? webView.getCurrentPath() : "",
            webView.getSessionId(),
            failingUrl,
            errorCode,
            description
        );
    }

    @Override
    @RequiresApi(api = Build.VERSION_CODES.M)
    public void onReceivedError(WebView view, WebResourceRequest request, WebResourceError error) {
        super.onReceivedError(view, request, error);
        String failingUrl = request != null ? request.getUrl().toString() : "";
        CharSequence desc = error.getDescription();
        String description = desc != null ? desc.toString() : "unknown error";
        int errorCode = error.getErrorCode();
        Log.e(TAG, "Error loading page: " + description +
              ", code: " + errorCode +
              ", failing URL: " + failingUrl);

        // Only report main-frame errors to Rust; sub-resource errors are not actionable.
        if (request != null && request.isForMainFrame()) {
            reportMainFrameLoadError(failingUrl, errorCode, description);
        }
    }

    @Override
    @SuppressWarnings("deprecation")
    public void onReceivedError(WebView view, int errorCode, String description, String failingUrl) {
        super.onReceivedError(view, errorCode, description, failingUrl);
        String safeUrl = failingUrl != null ? failingUrl : "";
        String safeDescription = description != null ? description : "unknown error";
        Log.e(TAG, "Error loading page (legacy callback): " + safeDescription +
              ", code: " + errorCode +
              ", failing URL: " + safeUrl);
        reportMainFrameLoadError(safeUrl, errorCode, safeDescription);
    }

    @Override
    public WebResourceResponse shouldInterceptRequest(WebView view, WebResourceRequest request) {
        if (request == null || request.getUrl() == null) {
            return null;
        }
        String url = request.getUrl().toString();
        LingXiaWebView webView = webViewRef.get();
        if (webView != null && webView.shouldSkipRustIntercept(url)) {
            return null;
        }
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

        if (webView != null) {
            // Call native to handle request
            LingXiaWebView.WebResourceResponseData response = webView.handleRequest(
                webView.getAppId() != null ? webView.getAppId() : "",
                webView.getCurrentPath() != null ? webView.getCurrentPath() : "",
                webView.getSessionId(),
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
                    // Buffered stream helps large cached assets (images) load more smoothly.
                    inputStream = new BufferedInputStream(new FileInputStream(file));
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
