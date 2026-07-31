package com.lingxia.webview;

import android.view.View;
import android.webkit.ValueCallback;

/** Common host contract for Android system WebView and alternative renderers. */
public interface LingXiaWebViewHost {
    View getHostView();
    String getAppId();
    String getCurrentPath();
    long getSessionId();
    String getUrl();
    String getTitle();
    boolean canGoBack();
    boolean canGoForward();
    boolean usesStrictSecurityProfile();
    boolean retainsSurfaceWhenHidden();
    void reload();
    void goBack();
    void goForward();
    void evaluateJavascript(String script, ValueCallback<String> callback);
    void dispatchClickAt(float x, float y);
    void scrollByPixels(int dx, int dy);
    void pause();
    void resume();
    void destroy();
}
