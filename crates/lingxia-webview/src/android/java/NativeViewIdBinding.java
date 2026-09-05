package com.lingxia.webview;

/**
 * Rust assigns one identity to each concrete WebView. The Java callback
 * surface may read that value but cannot replace it with a successor's ID.
 */
final class NativeViewIdBinding {
    private volatile long value;

    void assign(long nativeViewId) {
        if (nativeViewId <= 0) {
            throw new IllegalArgumentException("nativeViewId must be positive");
        }
        if (value != 0 && value != nativeViewId) {
            throw new IllegalStateException("nativeViewId is immutable for this WebView");
        }
        value = nativeViewId;
    }

    long current() {
        return value;
    }
}
