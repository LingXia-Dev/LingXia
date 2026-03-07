package com.lingxia.lxapp

import android.app.Activity
import android.graphics.Color
import android.graphics.drawable.GradientDrawable
import android.util.Log
import android.util.TypedValue
import android.view.KeyEvent
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.view.inputmethod.EditorInfo
import android.text.InputType
import android.webkit.WebChromeClient
import android.webkit.WebResourceRequest
import android.webkit.WebSettings
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.EditText
import android.widget.FrameLayout
import android.widget.ImageView
import android.widget.LinearLayout

object LxAppBrowserOverlay {
    private const val TAG = "LingXia.BrowserOverlay"

    private var overlayContainer: FrameLayout? = null
    private var webView: WebView? = null
    private var addressField: EditText? = null
    private var backButton: ImageView? = null
    private var forwardButton: ImageView? = null

    fun show(activity: Activity, url: String) {
        Log.d(TAG, "show URL: $url")

        // Dismiss existing overlay
        dismiss()

        val density = activity.resources.displayMetrics.density
        val rootView = activity.window.decorView as ViewGroup

        // Full-screen container
        val container = FrameLayout(activity).apply {
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
            setBackgroundColor(Color.WHITE)
            fitsSystemWindows = false
        }

        val statusBarHeight = LxAppActivity.getStatusBarHeight(activity)

        // === Top: Address Bar Area ===
        val topBarHeight = statusBarHeight + (60 * density).toInt() // status bar + 8dp + 44dp pill + 8dp
        val topBar = FrameLayout(activity).apply {
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                topBarHeight,
                Gravity.TOP
            )
            setBackgroundColor(Color.WHITE)
        }

        // Address pill (rounded rectangle)
        val pillHeight = (44 * density).toInt()
        val pillMarginH = (12 * density).toInt()
        val pillTop = statusBarHeight + (8 * density).toInt()

        val addressPill = LinearLayout(activity).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                pillHeight
            ).apply {
                leftMargin = pillMarginH
                rightMargin = pillMarginH
                topMargin = pillTop
            }
            val bg = GradientDrawable().apply {
                setColor(Color.parseColor("#F0F0F0"))
                cornerRadius = pillHeight / 2f
            }
            background = bg
            setPadding((16 * density).toInt(), 0, (6 * density).toInt(), 0)
        }

        // Address label
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
            setText(url)
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
        addressPill.addView(addrField)

        // Refresh button inside pill
        val refreshBtnSize = (36 * density).toInt()
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
            setOnClickListener { webView?.reload() }
        }
        addressPill.addView(refreshBtn)

        topBar.addView(addressPill)
        container.addView(topBar)

        // === Bottom: Navigation Toolbar ===
        val navBarHeight = getNavigationBarHeight(activity)
        val toolbarHeight = (44 * density).toInt()
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

        // Button row
        val buttonRow = LinearLayout(activity).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
            setPadding((8 * density).toInt(), 0, (8 * density).toInt(), 0)
        }

        // Back button
        val backBtn = createNavButton(activity, R.drawable.icon_back) {
            webView?.goBack()
        }
        backBtn.alpha = 0.3f
        backBtn.isEnabled = false
        buttonRow.addView(backBtn)

        // Forward button
        val fwdBtn = createNavButton(activity, R.drawable.icon_forward) {
            webView?.goForward()
        }
        fwdBtn.alpha = 0.3f
        fwdBtn.isEnabled = false
        buttonRow.addView(fwdBtn)

        // Spacer
        val spacer = View(activity).apply {
            layoutParams = LinearLayout.LayoutParams(0, 0, 1f)
        }
        buttonRow.addView(spacer)

        // Close button
        val closeBtn = createNavButton(activity, R.drawable.icon_close_x) {
            dismiss()
        }
        buttonRow.addView(closeBtn)

        bottomBar.addView(buttonRow)
        container.addView(bottomBar)

        // === Middle: WebView ===
        val wv = WebView(activity).apply {
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            ).apply {
                topMargin = topBarHeight
            }
            settings.apply {
                javaScriptEnabled = true
                domStorageEnabled = true
                useWideViewPort = true
                loadWithOverviewMode = true
                setSupportZoom(true)
                builtInZoomControls = true
                displayZoomControls = false
                mixedContentMode = WebSettings.MIXED_CONTENT_COMPATIBILITY_MODE
            }
            webViewClient = object : WebViewClient() {
                override fun onPageFinished(view: WebView?, pageUrl: String?) {
                    updateAddressBar(pageUrl)
                    updateNavigationButtons()
                }

                override fun shouldOverrideUrlLoading(view: WebView?, request: WebResourceRequest?): Boolean {
                    return false
                }

                override fun doUpdateVisitedHistory(view: WebView?, pageUrl: String?, isReload: Boolean) {
                    updateAddressBar(pageUrl)
                    updateNavigationButtons()
                }
            }
            webChromeClient = WebChromeClient()
        }
        container.addView(wv)

        rootView.addView(container)

        wv.loadUrl(url)

        overlayContainer = container
        webView = wv
        addressField = addrField
        backButton = backBtn
        forwardButton = fwdBtn

        Log.d(TAG, "Browser overlay shown")
    }

    fun dismiss() {
        webView?.apply {
            stopLoading()
            destroy()
        }
        webView = null

        overlayContainer?.let { container ->
            (container.parent as? ViewGroup)?.removeView(container)
        }
        overlayContainer = null
        addressField = null
        backButton = null
        forwardButton = null

        Log.d(TAG, "Browser overlay dismissed")
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
        webView?.loadUrl(result.url)
    }

    private fun createNavButton(
        activity: Activity,
        iconResId: Int,
        onClick: () -> Unit
    ): ImageView {
        val density = activity.resources.displayMetrics.density
        val size = (44 * density).toInt()

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
