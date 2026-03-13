package com.lingxia.lxapp

import android.app.Activity
import android.graphics.Color
import android.graphics.drawable.GradientDrawable
import android.text.InputType
import android.util.Log
import android.util.TypedValue
import android.view.Gravity
import android.view.KeyEvent
import android.view.View
import android.view.ViewGroup
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputMethodManager
import android.widget.EditText
import android.widget.FrameLayout
import android.widget.ImageView
import android.widget.LinearLayout
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat

object LxAppBrowserOverlay {
    private const val TAG = "LingXia.BrowserOverlay"
    private const val ATTACH_RETRY_DELAY_MS = 100L
    private const val ATTACH_MAX_RETRIES = 8

    private var overlayContainer: FrameLayout? = null
    private var webView: com.lingxia.lxapp.WebView? = null
    private var currentTabId: String? = null
    private var pendingTabId: String? = null
    private var pendingAttachToken: Long = 0L
    private var addressField: EditText? = null
    private var backButton: ImageView? = null
    private var forwardButton: ImageView? = null

    fun show(activity: Activity, tabId: String, initialUrl: String = ""): Boolean {
        val normalizedTabId = tabId.trim().lowercase()
        val normalizedInitialUrl = initialUrl.trim()
        if (normalizedTabId.isEmpty()) {
            Log.w(TAG, "show failed: empty tabId")
            return false
        }
        if (overlayContainer != null && currentTabId == normalizedTabId) {
            if (normalizedInitialUrl.isNotEmpty()) {
                updateAddressBar(normalizedInitialUrl)
            }
            return true
        }
        if (pendingTabId == normalizedTabId) {
            return true
        }

        pendingAttachToken += 1
        val token = pendingAttachToken
        pendingTabId = normalizedTabId
        tryShowOverlay(activity, normalizedTabId, normalizedInitialUrl, 0, token)
        return true
    }

    private fun tryShowOverlay(
        activity: Activity,
        tabId: String,
        initialUrl: String,
        attempt: Int,
        token: Long
    ) {
        if (pendingAttachToken != token || pendingTabId != tabId) {
            return
        }

        val managedWebView = findManagedWebView(tabId)
        if (managedWebView == null) {
            if (attempt >= ATTACH_MAX_RETRIES) {
                pendingTabId = null
                Log.w(TAG, "show failed: managed WebView not found for tabId=$tabId")
                closeBrowserTab(tabId)
                return
            }
            activity.window.decorView.postDelayed(
                { tryShowOverlay(activity, tabId, initialUrl, attempt + 1, token) },
                ATTACH_RETRY_DELAY_MS
            )
            return
        }

        pendingTabId = null
        dismiss()

        val density = activity.resources.displayMetrics.density
        val rootView = activity.window.decorView as ViewGroup

        val container = FrameLayout(activity).apply {
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
            setBackgroundColor(Color.WHITE)
            fitsSystemWindows = false
        }

        val navBarHeight = getNavigationBarHeight(activity)
        val toolbarHeight = (52 * density).toInt()
        val toolbarSideMargin = (12 * density).toInt()
        val toolbarBottomMargin = navBarHeight + (8 * density).toInt()

        val bottomBar = FrameLayout(activity).apply {
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                toolbarHeight
            ).apply {
                gravity = Gravity.BOTTOM
                leftMargin = toolbarSideMargin
                rightMargin = toolbarSideMargin
                bottomMargin = toolbarBottomMargin
            }
            background = GradientDrawable().apply {
                setColor(Color.parseColor("#F2FFFFFF"))
                cornerRadius = 16f * density
                setStroke(maxOf(1, (0.5f * density).toInt()), Color.parseColor("#1A000000"))
            }
            elevation = 10f * density
            clipToPadding = false
            clipChildren = false
        }

        val buttonRow = LinearLayout(activity).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
            setPadding((6 * density).toInt(), 0, (6 * density).toInt(), 0)
        }

        val backBtn = createNavButton(activity, R.drawable.icon_back, sizeDp = 32) {
            webView?.goBack()
            updateNavigationButtons()
        }
        backBtn.alpha = 0.3f
        backBtn.isEnabled = false
        buttonRow.addView(backBtn)

        val fwdBtn = createNavButton(activity, R.drawable.icon_forward, sizeDp = 32) {
            webView?.goForward()
            updateNavigationButtons()
        }
        fwdBtn.alpha = 0.3f
        fwdBtn.isEnabled = false
        buttonRow.addView(fwdBtn)

        val pillHeight = (38 * density).toInt()
        val pillMarginH = (4 * density).toInt()
        val addressPill = LinearLayout(activity).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            layoutParams = LinearLayout.LayoutParams(
                0,
                pillHeight,
                1f
            ).apply {
                leftMargin = pillMarginH
                rightMargin = pillMarginH
            }
            val bg = GradientDrawable().apply {
                setColor(Color.parseColor("#F0F0F0"))
                cornerRadius = pillHeight / 2f
            }
            background = bg
            setPadding((12 * density).toInt(), 0, (4 * density).toInt(), 0)
        }

        val addrField = EditText(activity).apply {
            layoutParams = LinearLayout.LayoutParams(
                0,
                ViewGroup.LayoutParams.WRAP_CONTENT,
                1f
            )
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 14f)
            setTextColor(Color.parseColor("#333333"))
            setSingleLine(true)
            imeOptions = EditorInfo.IME_ACTION_GO
            inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_URI
            background = null
            setPadding(0, 0, 0, 0)
            maxLines = 1
            setOnEditorActionListener { _, actionId, event ->
                val isSubmit = actionId == EditorInfo.IME_ACTION_GO ||
                    actionId == EditorInfo.IME_ACTION_DONE ||
                    (event?.keyCode == KeyEvent.KEYCODE_ENTER && event.action == KeyEvent.ACTION_DOWN)
                if (isSubmit) {
                    submitAddress()
                    true
                } else {
                    false
                }
            }
        }
        if (initialUrl.isNotEmpty()) {
            addrField.setText(initialUrl)
        }
        addressPill.addView(addrField)

        val refreshBtnSize = (32 * density).toInt()
        val refreshBtn = ImageView(activity).apply {
            layoutParams = LinearLayout.LayoutParams(refreshBtnSize, refreshBtnSize)
            scaleType = ImageView.ScaleType.CENTER_INSIDE
            setImageResource(R.drawable.icon_browser_refresh)
            setColorFilter(Color.parseColor("#666666"))
            isClickable = true
            isFocusable = true
            val outValue = TypedValue()
            activity.theme.resolveAttribute(
                android.R.attr.selectableItemBackgroundBorderless, outValue, true
            )
            setBackgroundResource(outValue.resourceId)
            setOnClickListener {
                webView?.reload()
                updateNavigationButtons()
            }
        }
        addressPill.addView(refreshBtn)
        buttonRow.addView(addressPill)

        val closeBtn = createNavButton(activity, R.drawable.icon_close_x) {
            dismiss()
        }
        buttonRow.addView(closeBtn)

        bottomBar.addView(buttonRow)

        val webViewParams = FrameLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT,
            ViewGroup.LayoutParams.MATCH_PARENT
        ).apply {
            topMargin = 0
        }
        if (managedWebView.parent != null) {
            (managedWebView.parent as? ViewGroup)?.removeView(managedWebView)
        }
        managedWebView.layoutParams = webViewParams
        managedWebView.visibility = View.VISIBLE

        container.addView(managedWebView)
        container.addView(bottomBar)
        rootView.addView(container)

        managedWebView.resume()
        updateAddressBar(managedWebView.url ?: initialUrl)

        overlayContainer = container
        webView = managedWebView
        currentTabId = tabId
        addressField = addrField
        backButton = backBtn
        forwardButton = fwdBtn
        updateNavigationButtons()

        ViewCompat.setOnApplyWindowInsetsListener(container) { _, insets ->
            val statusBarTop = insets.getInsets(WindowInsetsCompat.Type.statusBars()).top
            val wvParams = managedWebView.layoutParams as FrameLayout.LayoutParams
            if (wvParams.topMargin != statusBarTop) {
                wvParams.topMargin = statusBarTop
                managedWebView.layoutParams = wvParams
            }

            val imeBottom = insets.getInsets(WindowInsetsCompat.Type.ime()).bottom
            val navBottom = insets.getInsets(WindowInsetsCompat.Type.navigationBars()).bottom
            val keyboardHeight = (imeBottom - navBottom).coerceAtLeast(0)
            val params = bottomBar.layoutParams as FrameLayout.LayoutParams
            val newMargin = navBarHeight + keyboardHeight + (8 * density).toInt()
            if (params.bottomMargin != newMargin) {
                params.bottomMargin = newMargin
                bottomBar.layoutParams = params
            }
            insets
        }
        ViewCompat.requestApplyInsets(container)

        Log.d(TAG, "Browser overlay shown tabId=$tabId")
    }

    fun dismiss() {
        val pendingTab = pendingTabId
        cancelPendingAttachRetry()

        overlayContainer?.let { container ->
            ViewCompat.setOnApplyWindowInsetsListener(container, null)
        }

        val tabId = currentTabId

        webView?.apply {
            stopLoading()
            (parent as? ViewGroup)?.removeView(this)
            pause()
        }
        webView = null

        if (!pendingTab.isNullOrBlank() && pendingTab != tabId) {
            closeBrowserTab(pendingTab)
        }
        if (!tabId.isNullOrBlank()) {
            closeBrowserTab(tabId)
        }

        overlayContainer?.let { container ->
            (container.parent as? ViewGroup)?.removeView(container)
        }
        overlayContainer = null
        currentTabId = null
        addressField = null
        backButton = null
        forwardButton = null

        Log.d(TAG, "Browser overlay dismissed")
    }

    private fun cancelPendingAttachRetry() {
        pendingAttachToken += 1
        pendingTabId = null
    }

    private fun closeBrowserTab(tabId: String) {
        val closed = NativeApi.browserTabClose(tabId)
        if (!closed) {
            Log.w(TAG, "browserTabClose failed for tabId=$tabId")
        }
    }

    fun isShowing(): Boolean = overlayContainer != null

    private fun updateAddressBar(url: String?) {
        if (url == null) return
        val field = addressField ?: return
        if (field.hasFocus()) return
        field.setText(url)
    }

    private fun updateNavigationButtons() {
        val canGoBack = webView?.canGoBack() ?: false
        backButton?.apply {
            alpha = if (canGoBack) 1.0f else 0.3f
            isEnabled = canGoBack
        }

        val canGoForward = webView?.canGoForward() ?: false
        forwardButton?.apply {
            alpha = if (canGoForward) 1.0f else 0.3f
            isEnabled = canGoForward
        }
    }

    private fun submitAddress() {
        val field = addressField ?: return
        val result = handleBrowserAddressSubmission(field.text?.toString(), webView?.url) ?: return
        field.setText(result.displayText)
        field.clearFocus()
        val imm = field.context.getSystemService(android.content.Context.INPUT_METHOD_SERVICE) as? InputMethodManager
        imm?.hideSoftInputFromWindow(field.windowToken, 0)
        webView?.loadUrl(result.url)
        updateAddressBar(result.url)
    }

    private fun findManagedWebView(tabId: String): com.lingxia.lxapp.WebView? {
        val browserAppId = NativeApi.getBuiltinBrowserAppId()?.trim().orEmpty()
        if (browserAppId.isEmpty()) {
            Log.w(TAG, "findManagedWebView failed: empty browser appId")
            return null
        }

        val sessionId = NativeApi.getLxAppSessionId(browserAppId)
        if (sessionId <= 0L) {
            Log.w(TAG, "findManagedWebView failed: invalid browser session for appId=$browserAppId")
            return null
        }

        val path = NativeApi.browserTabPathForId(tabId)?.trim().orEmpty()
        if (path.isEmpty()) {
            Log.w(TAG, "findManagedWebView failed: invalid tab path for tabId=$tabId")
            return null
        }

        return NativeApi.findWebView(browserAppId, path, sessionId)
    }

    private fun createNavButton(
        activity: Activity,
        iconResId: Int,
        sizeDp: Int = 44,
        onClick: () -> Unit
    ): ImageView {
        val density = activity.resources.displayMetrics.density
        val size = (sizeDp * density).toInt()

        return ImageView(activity).apply {
            layoutParams = LinearLayout.LayoutParams(size, size)
            scaleType = ImageView.ScaleType.CENTER
            setImageResource(iconResId)
            setColorFilter(Color.parseColor("#333333"))
            isClickable = true
            isFocusable = true
            val outValue = TypedValue()
            activity.theme.resolveAttribute(
                android.R.attr.selectableItemBackgroundBorderless,
                outValue,
                true
            )
            setBackgroundResource(outValue.resourceId)
            setOnClickListener { onClick() }
        }
    }

    private fun getNavigationBarHeight(activity: Activity): Int {
        val resourceId = activity.resources.getIdentifier("navigation_bar_height", "dimen", "android")
        return if (resourceId > 0) {
            activity.resources.getDimensionPixelSize(resourceId)
        } else {
            0
        }
    }
}
