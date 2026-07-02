package com.lingxia.lxapp

import android.app.Activity
import android.content.Context
import android.graphics.Color
import android.graphics.drawable.GradientDrawable
import android.text.InputType
import android.text.TextUtils
import android.util.Log
import android.util.TypedValue
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputMethodManager
import android.widget.EditText
import android.widget.FrameLayout
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import com.lingxia.app.NativeApi
import java.net.URI

internal object LxAppBrowser {
    private const val TAG = "LingXia.Browser"
    private const val ATTACH_RETRY_DELAY_MS = 100L
    private const val ATTACH_MAX_RETRIES = 8
    private const val HIDDEN_NEW_TAB_URL = "lingxia://newtab"

    private val openTabIds = mutableListOf<String>()
    private var activeTabId: String? = null
    private var pendingTabId: String? = null
    private var pendingAttachToken: Long = 0L

    private var overlayContainer: FrameLayout? = null
    private var contentHost: FrameLayout? = null
    private var bottomBar: View? = null
    private var tabSwitcher: View? = null
    private var overflowMenu: View? = null
    private var activeWebView: WebView? = null
    private var activeWebViewTabId: String? = null
    private var currentActivity: Activity? = null

    private var addressIcon: ImageView? = null
    private var addressField: EditText? = null
    private var addressRow: View? = null
    private var backButton: ImageView? = null
    private var forwardButton: ImageView? = null
    private var asideRefreshButton: View? = null
    private var plusButton: View? = null
    private var menuButton: View? = null
    private var tabsBadge: TextView? = null
    // Aside chrome: the active tab was opened as an aside — hide the address
    // row and the new-tab/menu affordances; a row refresh appears instead.
    private var isAsideActive = false
    // Chrome-style history intervention: until the user interacts with a tab
    // (page touch or address navigation), auto-created history (SPA pushState
    // redirects) must not light back/forward.
    private val interactedTabIds = mutableSetOf<String>()

    private val chromeRefreshRunnable = object : Runnable {
        override fun run() {
            refreshChromeFromActiveWebView()
            scheduleChromeRefresh()
        }
    }

    fun show(activity: Activity, tabId: String, initialUrl: String = ""): Boolean {
        val normalizedTabId = normalizeTabId(tabId)
        if (normalizedTabId.isEmpty()) {
            Log.w(TAG, "show failed: empty tabId")
            return false
        }

        registerTab(normalizedTabId)
        val tabChanged = activeTabId != normalizedTabId
        activeTabId = normalizedTabId
        currentActivity = activity
        NativeApi.browserTabActivate(normalizedTabId)

        if (!ensureChrome(activity)) {
            return false
        }
        if (tabChanged) {
            onActiveTabSwitched(activity, normalizedTabId)
        } else {
            refreshAsideChrome(activity)
        }
        closeOverflowMenu()
        closeTabSwitcher()
        startChromeRefreshLoop()
        beginAttachActiveTab(activity, initialUrl.trim())
        return true
    }

    fun dismiss() {
        pendingAttachToken += 1
        pendingTabId = null
        stopChromeRefreshLoop()
        closeOverflowMenu()
        closeTabSwitcher()

        activeWebView?.pause()
        activeWebView?.let { view ->
            (view.parent as? ViewGroup)?.removeView(view)
        }
        activeWebView = null
        activeWebViewTabId = null

        val tabsToClose = openTabIds.toList()
        openTabIds.clear()
        interactedTabIds.clear()
        isAsideActive = false
        activeTabId = null
        tabsToClose.forEach { closeBrowserTab(it) }

        overlayContainer?.let { container ->
            ViewCompat.setOnApplyWindowInsetsListener(container, null)
            (container.parent as? ViewGroup)?.removeView(container)
        }
        overlayContainer = null
        contentHost = null
        bottomBar = null
        currentActivity = null
        addressIcon = null
        addressField = null
        backButton = null
        forwardButton = null
        tabsBadge = null
    }

    fun isShowing(): Boolean = overlayContainer != null

    fun handleBack(): Boolean {
        if (!isShowing()) {
            return false
        }
        if (overflowMenu != null) {
            closeOverflowMenu()
            return true
        }
        if (tabSwitcher != null) {
            closeTabSwitcher()
            return true
        }
        if (activeWebView?.canGoBack() == true) {
            navigateBack()
            return true
        }
        dismiss()
        return true
    }

    private fun ensureChrome(activity: Activity): Boolean {
        val existing = overlayContainer
        if (existing != null) {
            currentActivity = activity
            return true
        }

        val rootView = activity.window.decorView as? ViewGroup ?: return false
        val density = activity.resources.displayMetrics.density

        val container = FrameLayout(activity).apply {
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
            setBackgroundColor(Color.WHITE)
            fitsSystemWindows = false
            clipChildren = false
            clipToPadding = false
        }

        val host = FrameLayout(activity).apply {
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
        }

        val bar = buildBottomBar(activity, density)
        container.addView(host)
        container.addView(bar)
        rootView.addView(container)

        overlayContainer = container
        contentHost = host
        bottomBar = bar
        currentActivity = activity

        ViewCompat.setOnApplyWindowInsetsListener(container) { _, insets ->
            val systemBars = insets.getInsets(WindowInsetsCompat.Type.systemBars())
            val ime = insets.getInsets(WindowInsetsCompat.Type.ime())
            val keyboardVisible = insets.isVisible(WindowInsetsCompat.Type.ime())
            val barHeight = dp(activity, 96)
            val navInset = if (keyboardVisible) 0 else systemBars.bottom
            val liftInset = if (keyboardVisible) ime.bottom else 0
            val totalBarHeight = barHeight + navInset

            (host.layoutParams as? FrameLayout.LayoutParams)?.let { params ->
                params.topMargin = systemBars.top
                params.bottomMargin = totalBarHeight + liftInset
                host.layoutParams = params
            }
            (bar.layoutParams as? FrameLayout.LayoutParams)?.let { params ->
                params.height = totalBarHeight
                params.bottomMargin = liftInset
                bar.layoutParams = params
            }
            bar.setPadding(dp(activity, 12), dp(activity, 6), dp(activity, 12), dp(activity, 6) + navInset)
            insets
        }
        ViewCompat.requestApplyInsets(container)
        return true
    }

    private fun buildBottomBar(activity: Activity, density: Float): View {
        val barHeight = dp(activity, 96)
        val bar = LinearLayout(activity).apply {
            orientation = LinearLayout.VERTICAL
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                barHeight
            ).apply {
                gravity = Gravity.BOTTOM
                bottomMargin = 0
            }
            background = GradientDrawable().apply {
                setColor(Color.parseColor("#FAFFFFFF"))
                cornerRadius = 0f
                setStroke(maxOf(1, (0.5f * density).toInt()), Color.parseColor("#14000000"))
            }
            elevation = 0f
            setPadding(dp(activity, 12), dp(activity, 6), dp(activity, 12), dp(activity, 6))
        }

        val addressRow = LinearLayout(activity).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            layoutParams = LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                0,
                1f
            )
        }
        val addressPill = LinearLayout(activity).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            layoutParams = LinearLayout.LayoutParams(
                0,
                ViewGroup.LayoutParams.MATCH_PARENT,
                1f
            )
            background = GradientDrawable().apply {
                setColor(Color.parseColor("#F0F0F0"))
                cornerRadius = dp(activity, 18).toFloat()
            }
            setPadding(dp(activity, 12), 0, dp(activity, 4), 0)
        }

        val addrIcon = ImageView(activity).apply {
            layoutParams = LinearLayout.LayoutParams(dp(activity, 18), dp(activity, 18)).apply {
                rightMargin = dp(activity, 6)
            }
            scaleType = ImageView.ScaleType.CENTER_INSIDE
            setImageResource(R.drawable.icon_lock)
            setColorFilter(Color.parseColor("#666666"))
            isFocusable = false
            isClickable = false
        }
        val addrField = EditText(activity).apply {
            layoutParams = LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f)
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 14f)
            setTextColor(Color.parseColor("#333333"))
            setHintTextColor(Color.parseColor("#888888"))
            hint = "Search or enter address"
            setSingleLine(true)
            maxLines = 1
            ellipsize = TextUtils.TruncateAt.MIDDLE
            background = null
            inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_URI
            imeOptions = EditorInfo.IME_ACTION_GO
            setPadding(0, 0, 0, 0)
            setSelectAllOnFocus(true)
            setOnEditorActionListener { _, actionId, event ->
                val enterPressed = event?.keyCode == android.view.KeyEvent.KEYCODE_ENTER &&
                    event.action == android.view.KeyEvent.ACTION_UP
                if (actionId == EditorInfo.IME_ACTION_GO || enterPressed) {
                    navigateFromAddressBar(activity)
                    true
                } else {
                    false
                }
            }
            setOnFocusChangeListener { _, hasFocus ->
                if (!hasFocus) {
                    updateAddressBar(activeWebView?.url.orEmpty())
                }
            }
        }
        val refreshBtn = createIconButton(activity, R.drawable.icon_browser_refresh, 32, "#666666") {
            activeWebView?.reload()
            scheduleChromeRefreshSoon()
        }

        addressPill.addView(addrIcon)
        addressPill.addView(addrField)
        addressPill.addView(refreshBtn)
        addressRow.addView(addressPill)
        bar.addView(addressRow)

        val actionRow = LinearLayout(activity).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            layoutParams = LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                0,
                1f
            )
        }
        val backBtn = createIconButton(activity, R.drawable.icon_back, 32, "#666666") {
            navigateBack()
        }
        val fwdBtn = createIconButton(activity, R.drawable.icon_forward, 32, "#666666") {
            navigateForward()
        }
        val plusBtn = createIconButton(activity, R.drawable.icon_plus, 34, "#333333") {
            openNewTab(activity)
        }
        val tabsBtn = createTabsButton(activity) {
            showTabSwitcher(activity)
        }
        val menuBtn = createIconButton(activity, R.drawable.icon_menu, 34, "#333333") { anchor ->
            showOverflowMenu(activity, anchor)
        }
        val closeBtn = createIconButton(activity, R.drawable.icon_close_x, 34, "#333333") {
            dismiss()
        }

        // Aside chrome has no address pill, so refresh moves into the row.
        val asideRefreshBtn = createIconButton(activity, R.drawable.icon_browser_refresh, 32, "#333333") {
            activeWebView?.reload()
            scheduleChromeRefreshSoon()
        }
        asideRefreshBtn.visibility = View.GONE

        actionRow.addView(backBtn)
        actionRow.addView(fwdBtn)
        actionRow.addView(asideRefreshBtn)
        actionRow.addView(View(activity), LinearLayout.LayoutParams(0, 1, 1f))
        actionRow.addView(plusBtn)
        actionRow.addView(tabsBtn)
        actionRow.addView(menuBtn)
        actionRow.addView(closeBtn)
        bar.addView(actionRow)

        addressIcon = addrIcon
        addressField = addrField
        this.addressRow = addressRow
        backButton = backBtn
        forwardButton = fwdBtn
        asideRefreshButton = asideRefreshBtn
        plusButton = plusBtn
        menuButton = menuBtn
        updateNavigationButtons()
        return bar
    }

    private fun beginAttachActiveTab(activity: Activity, initialUrl: String = "") {
        val tabId = activeTabId ?: return
        pendingAttachToken += 1
        val token = pendingAttachToken
        pendingTabId = tabId
        attachActiveTab(activity, tabId, initialUrl, 0, token)
    }

    private fun attachActiveTab(
        activity: Activity,
        tabId: String,
        initialUrl: String,
        attempt: Int,
        token: Long
    ) {
        if (pendingAttachToken != token || pendingTabId != tabId || activeTabId != tabId) {
            return
        }

        val managedWebView = findManagedWebView(tabId)
        if (managedWebView == null) {
            if (attempt >= ATTACH_MAX_RETRIES) {
                pendingTabId = null
                Log.w(TAG, "show failed: managed WebView not found for tabId=$tabId")
                closeTab(tabId)
                return
            }
            activity.window.decorView.postDelayed(
                { attachActiveTab(activity, tabId, initialUrl, attempt + 1, token) },
                ATTACH_RETRY_DELAY_MS
            )
            return
        }

        pendingTabId = null
        attachWebView(managedWebView, tabId, initialUrl)
    }

    private fun attachWebView(managedWebView: WebView, tabId: String, initialUrl: String) {
        val host = contentHost ?: return
        if (activeWebView !== managedWebView) {
            activeWebView?.pause()
            activeWebView?.let { previous ->
                (previous.parent as? ViewGroup)?.removeView(previous)
            }
        }
        (managedWebView.parent as? ViewGroup)?.removeView(managedWebView)
        managedWebView.layoutParams = FrameLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT,
            ViewGroup.LayoutParams.MATCH_PARENT
        )
        managedWebView.visibility = View.VISIBLE
        host.removeAllViews()
        host.addView(managedWebView)
        managedWebView.resume()

        managedWebView.setOnTouchListener { _, event ->
            if (event.action == android.view.MotionEvent.ACTION_DOWN) {
                markActiveTabInteracted()
            }
            false
        }
        activeWebView = managedWebView
        activeWebViewTabId = tabId
        updateAddressBar(initialUrl.ifEmpty { managedWebView.url.orEmpty() })
        refreshChromeFromActiveWebView()
        scheduleChromeRefreshSoon()
    }

    private fun openNewTab(activity: Activity) {
        closeOverflowMenu()
        closeTabSwitcher()
        val appId = NativeApi.getBuiltinBrowserAppId()?.takeIf { it.isNotBlank() }
        if (appId == null) {
            Log.w(TAG, "openNewTab failed: empty browser appId")
            return
        }
        val sessionId = NativeApi.getLxAppSessionId(appId)
        if (sessionId <= 0L) {
            Log.w(TAG, "openNewTab failed: invalid session for appId=$appId")
            return
        }
        val tabId = NativeApi.openBrowserTab(appId, sessionId, HIDDEN_NEW_TAB_URL)
        if (tabId.isNullOrBlank()) {
            Log.w(TAG, "openNewTab failed: native openBrowserTab returned empty tab")
            return
        }
        show(activity, tabId, HIDDEN_NEW_TAB_URL)
    }

    private fun activateTab(activity: Activity, tabId: String) {
        val normalizedTabId = normalizeTabId(tabId)
        if (!openTabIds.contains(normalizedTabId)) {
            return
        }
        activeTabId = normalizedTabId
        NativeApi.browserTabActivate(normalizedTabId)
        onActiveTabSwitched(activity, normalizedTabId)
        closeOverflowMenu()
        closeTabSwitcher()
        beginAttachActiveTab(activity)
    }

    private fun closeTab(tabId: String) {
        val normalizedTabId = normalizeTabId(tabId)
        val index = openTabIds.indexOf(normalizedTabId)
        if (index < 0) {
            closeBrowserTab(normalizedTabId)
            return
        }

        if (activeWebViewTabId == normalizedTabId) {
            activeWebView?.pause()
            activeWebView?.let { view ->
                (view.parent as? ViewGroup)?.removeView(view)
            }
            activeWebView = null
            activeWebViewTabId = null
        }
        openTabIds.removeAt(index)
        closeBrowserTab(normalizedTabId)

        if (openTabIds.isEmpty()) {
            dismiss()
            return
        }

        if (activeTabId == normalizedTabId) {
            val nextIndex = index.coerceAtMost(openTabIds.lastIndex)
            currentActivity?.let { activity ->
                activeTabId = openTabIds[nextIndex]
                NativeApi.browserTabActivate(activeTabId!!)
                beginAttachActiveTab(activity)
            }
        }
        updateTabsBadge()
    }

    private fun navigateFromAddressBar(activity: Activity) {
        val raw = addressField?.text?.toString().orEmpty()
        hideKeyboard(activity, addressField)
        addressField?.clearFocus()
        val targetUrl = normalizeAddressInput(raw)
        if (targetUrl == null) {
            updateAddressBar(activeWebView?.url.orEmpty())
            return
        }
        // An address-bar navigation is a user interaction.
        markActiveTabInteracted()
        navigateActiveTab(activity, targetUrl)
    }

    private fun showTabSwitcher(activity: Activity) {
        val container = overlayContainer ?: return
        closeOverflowMenu()
        closeTabSwitcher()

        val overlay = FrameLayout(activity).apply {
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
            setBackgroundColor(Color.parseColor("#66000000"))
            isClickable = true
            setOnClickListener { closeTabSwitcher() }
        }
        val panel = LinearLayout(activity).apply {
            orientation = LinearLayout.VERTICAL
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT
            ).apply {
                gravity = Gravity.BOTTOM
                leftMargin = dp(activity, 12)
                rightMargin = dp(activity, 12)
                bottomMargin = dp(activity, 12)
            }
            background = GradientDrawable().apply {
                setColor(Color.WHITE)
                cornerRadii = floatArrayOf(
                    dp(activity, 16).toFloat(), dp(activity, 16).toFloat(),
                    dp(activity, 16).toFloat(), dp(activity, 16).toFloat(),
                    0f, 0f,
                    0f, 0f
                )
            }
            elevation = dp(activity, 12).toFloat()
            setPadding(dp(activity, 12), dp(activity, 10), dp(activity, 12), dp(activity, 12))
            setOnClickListener { }
        }

        val header = LinearLayout(activity).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            layoutParams = LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                dp(activity, 44)
            )
        }
        header.addView(TextView(activity).apply {
            text = "Tabs"
            setTextColor(Color.parseColor("#222222"))
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 17f)
            setTypeface(typeface, android.graphics.Typeface.BOLD)
            layoutParams = LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f)
        })
        // New tabs are self mode; hide the affordance while an aside is active.
        if (!isAsideActive) {
            header.addView(createIconButton(activity, R.drawable.icon_plus, 34, "#333333") {
                closeTabSwitcher()
                openNewTab(activity)
            })
        }
        panel.addView(header)

        val list = LinearLayout(activity).apply {
            orientation = LinearLayout.VERTICAL
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT
            )
        }
        openTabIds.forEach { tabId ->
            list.addView(createTabRow(activity, tabId))
        }
        panel.addView(ScrollView(activity).apply {
            layoutParams = LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                minOf(dp(activity, 360), activity.resources.displayMetrics.heightPixels / 2)
            )
            addView(list)
        })

        overlay.addView(panel)
        container.addView(overlay)
        tabSwitcher = overlay
    }

    private fun createTabRow(activity: Activity, tabId: String): View {
        val isActive = tabId == activeTabId
        return LinearLayout(activity).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            layoutParams = LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                dp(activity, 52)
            )
            background = GradientDrawable().apply {
                setColor(if (isActive) Color.parseColor("#F2F4F7") else Color.TRANSPARENT)
                cornerRadius = dp(activity, 8).toFloat()
            }
            setPadding(dp(activity, 10), 0, dp(activity, 4), 0)
            isClickable = true
            setOnClickListener { activateTab(activity, tabId) }

            addView(TextView(activity).apply {
                text = tabTitle(tabId)
                setTextColor(Color.parseColor("#222222"))
                setTextSize(TypedValue.COMPLEX_UNIT_SP, 15f)
                setSingleLine(true)
                ellipsize = TextUtils.TruncateAt.END
                layoutParams = LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f)
            })
            addView(createIconButton(activity, R.drawable.icon_close_x, 32, "#666666") {
                closeTab(tabId)
                if (isShowing()) {
                    showTabSwitcher(activity)
                }
            })
        }
    }

    private fun closeTabSwitcher() {
        tabSwitcher?.let { view ->
            (view.parent as? ViewGroup)?.removeView(view)
        }
        tabSwitcher = null
    }

    private fun closeOverflowMenu() {
        overflowMenu?.let { view ->
            (view.parent as? ViewGroup)?.removeView(view)
        }
        overflowMenu = null
    }

    private fun showOverflowMenu(activity: Activity, anchor: View) {
        val container = overlayContainer ?: return
        if (overflowMenu != null) {
            closeOverflowMenu()
            return
        }
        closeTabSwitcher()

        val overlay = FrameLayout(activity).apply {
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
            setBackgroundColor(Color.TRANSPARENT)
            elevation = dp(activity, 32).toFloat()
            translationZ = dp(activity, 32).toFloat()
            isClickable = true
            setOnClickListener { closeOverflowMenu() }
        }

        val panelWidth = dp(activity, 188)
        val containerLocation = IntArray(2)
        val anchorLocation = IntArray(2)
        container.getLocationInWindow(containerLocation)
        anchor.getLocationInWindow(anchorLocation)
        val containerWidth = container.width.takeIf { it > 0 } ?: activity.resources.displayMetrics.widthPixels
        val containerHeight = container.height.takeIf { it > 0 } ?: activity.resources.displayMetrics.heightPixels
        val anchorRight = anchorLocation[0] - containerLocation[0] + anchor.width
        val rightMargin = (containerWidth - anchorRight).coerceAtLeast(dp(activity, 12))
        val barTop = bottomBar?.let { bar ->
            val location = IntArray(2)
            bar.getLocationInWindow(location)
            location[1] - containerLocation[1]
        }?.takeIf { it > 0 }
        val bottomMargin = if (barTop != null) {
            (containerHeight - barTop + dp(activity, 8)).coerceAtLeast(dp(activity, 12))
        } else {
            (bottomBar?.height ?: dp(activity, 96)) + getNavigationBarHeight(activity) + dp(activity, 10)
        }

        val panel = LinearLayout(activity).apply {
            orientation = LinearLayout.VERTICAL
            layoutParams = FrameLayout.LayoutParams(
                panelWidth,
                ViewGroup.LayoutParams.WRAP_CONTENT
            ).apply {
                gravity = Gravity.BOTTOM or Gravity.END
                this.rightMargin = rightMargin
                this.bottomMargin = bottomMargin
            }
            background = GradientDrawable().apply {
                setColor(Color.WHITE)
                cornerRadius = dp(activity, 12).toFloat()
                setStroke(dp(activity, 1), Color.parseColor("#14000000"))
            }
            elevation = dp(activity, 14).toFloat()
            setPadding(dp(activity, 6), dp(activity, 6), dp(activity, 6), dp(activity, 6))
            setOnClickListener { }
        }

        panel.addView(
            createOverflowMenuRow(activity, R.drawable.icon_download, "Downloads") {
                closeOverflowMenu()
                navigateActiveTab(activity, "lingxia://downloads")
            }
        )
        panel.addView(
            createOverflowMenuRow(activity, R.drawable.icon_settings, "Settings") {
                closeOverflowMenu()
                navigateActiveTab(activity, "lingxia://settings")
            }
        )

        overlay.addView(panel)
        container.addView(overlay)
        overlay.bringToFront()
        overflowMenu = overlay
    }

    private fun createOverflowMenuRow(
        activity: Activity,
        resId: Int,
        title: String,
        onClick: () -> Unit
    ): View {
        return LinearLayout(activity).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            layoutParams = LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                dp(activity, 48)
            )
            background = GradientDrawable().apply {
                setColor(Color.TRANSPARENT)
                cornerRadius = dp(activity, 8).toFloat()
            }
            setPadding(dp(activity, 12), 0, dp(activity, 12), 0)
            isClickable = true
            isFocusable = true
            val outValue = TypedValue()
            activity.theme.resolveAttribute(
                android.R.attr.selectableItemBackground, outValue, true
            )
            setBackgroundResource(outValue.resourceId)
            setOnClickListener { onClick() }

            addView(ImageView(activity).apply {
                layoutParams = LinearLayout.LayoutParams(dp(activity, 22), dp(activity, 22)).apply {
                    rightMargin = dp(activity, 12)
                }
                scaleType = ImageView.ScaleType.CENTER_INSIDE
                setImageResource(resId)
                setColorFilter(Color.parseColor("#333333"))
            })
            addView(TextView(activity).apply {
                text = title
                setTextColor(Color.parseColor("#222222"))
                setTextSize(TypedValue.COMPLEX_UNIT_SP, 15f)
                includeFontPadding = false
                layoutParams = LinearLayout.LayoutParams(
                    0,
                    ViewGroup.LayoutParams.WRAP_CONTENT,
                    1f
                )
            })
        }
    }

    private fun updateAddressBar(url: String) {
        val cleanUrl = url.trim()
        val hidden = cleanUrl.isEmpty() || cleanUrl.equals(HIDDEN_NEW_TAB_URL, ignoreCase = true)
        val field = addressField
        if (field != null && !field.hasFocus()) {
            field.setText(if (hidden) "" else displayUrl(cleanUrl))
        }

        val icon = addressIcon ?: return
        if (hidden) {
            icon.visibility = View.GONE
            return
        }
        icon.visibility = View.VISIBLE
        val scheme = runCatching { URI(cleanUrl).scheme?.lowercase() }.getOrNull()
        if (scheme == "https" || scheme == "lingxia") {
            icon.setImageResource(R.drawable.icon_lock)
            icon.setColorFilter(Color.parseColor("#666666"))
        } else {
            icon.setImageResource(R.drawable.icon_warning)
            icon.setColorFilter(Color.parseColor("#C44A21"))
        }
    }

    private fun updateNavigationButtons() {
        val view = activeWebView
        // Pre-interaction history is auto-created (redirects/pushState) and
        // must not light the affordances.
        val interacted = activeTabId?.let(interactedTabIds::contains) == true
        setButtonEnabled(backButton, view?.canGoBack() == true && interacted)
        setButtonEnabled(forwardButton, view?.canGoForward() == true && interacted)
        updateTabsBadge()
    }

    private fun updateTabsBadge() {
        val count = openTabIds.size.coerceAtLeast(1)
        tabsBadge?.text = if (count > 99) "99+" else count.toString()
    }

    private fun refreshChromeFromActiveWebView() {
        // During attach retry the displayed webview still belongs to the
        // previous tab; address and back/forward are per-tab, keep them reset.
        if (activeWebViewTabId != activeTabId) {
            updateTabsBadge()
            return
        }
        updateAddressBar(activeWebView?.url.orEmpty())
        updateNavigationButtons()
    }

    // Blank the per-tab chrome immediately on a tab switch and re-derive the
    // aside styling for the new tab.
    private fun onActiveTabSwitched(activity: Activity, tabId: String) {
        addressField?.setText("")
        setButtonEnabled(backButton, false)
        setButtonEnabled(forwardButton, false)
        isAsideActive = NativeApi.browserTabIsAside(tabId)
        refreshAsideChrome(activity)
    }

    // Aside chrome: no address row, no new-tab/menu, refresh in the row.
    private fun refreshAsideChrome(activity: Activity) {
        val aside = isAsideActive
        addressRow?.visibility = if (aside) View.GONE else View.VISIBLE
        plusButton?.visibility = if (aside) View.GONE else View.VISIBLE
        menuButton?.visibility = if (aside) View.GONE else View.VISIBLE
        asideRefreshButton?.visibility = if (aside) View.VISIBLE else View.GONE
        bottomBar?.layoutParams?.let { params ->
            params.height = dp(activity, if (aside) 56 else 96)
            bottomBar?.layoutParams = params
        }
    }

    private fun markActiveTabInteracted() {
        val tabId = activeTabId ?: return
        if (!interactedTabIds.add(tabId)) {
            return
        }
        updateNavigationButtons()
    }

    private fun startChromeRefreshLoop() {
        val container = overlayContainer ?: return
        container.removeCallbacks(chromeRefreshRunnable)
        container.postDelayed(chromeRefreshRunnable, 400L)
    }

    private fun scheduleChromeRefresh() {
        overlayContainer?.postDelayed(chromeRefreshRunnable, 400L)
    }

    private fun scheduleChromeRefreshSoon() {
        val container = overlayContainer ?: return
        container.removeCallbacks(chromeRefreshRunnable)
        container.postDelayed(chromeRefreshRunnable, 120L)
    }

    private fun stopChromeRefreshLoop() {
        overlayContainer?.removeCallbacks(chromeRefreshRunnable)
    }

    private fun navigateActiveTab(activity: Activity, targetUrl: String): Boolean {
        val tabId = activeTabId ?: return false
        updateAddressBar(targetUrl)
        if (!NativeApi.browserTabNavigate(tabId, targetUrl)) {
            Log.w(TAG, "navigate failed: tabId=$tabId url=$targetUrl")
            scheduleChromeRefreshSoon()
            return false
        }
        beginAttachActiveTab(activity, targetUrl)
        scheduleChromeRefreshSoon()
        return true
    }

    private fun navigateBack() {
        val view = activeWebView ?: return
        if (view.canGoBack()) {
            view.goBack()
            scheduleChromeRefreshSoon()
        } else {
            updateNavigationButtons()
        }
    }

    private fun navigateForward() {
        val view = activeWebView ?: return
        if (view.canGoForward()) {
            view.goForward()
            scheduleChromeRefreshSoon()
        } else {
            updateNavigationButtons()
        }
    }

    private fun registerTab(tabId: String) {
        if (!openTabIds.contains(tabId)) {
            openTabIds.add(tabId)
        }
    }

    private fun findManagedWebView(tabId: String): WebView? {
        val appId = NativeApi.getBuiltinBrowserAppId()?.takeIf { it.isNotBlank() } ?: return null
        val path = NativeApi.browserTabPathForId(tabId)?.takeIf { it.isNotBlank() } ?: return null
        val sessionId = NativeApi.getLxAppSessionId(appId)
        if (sessionId <= 0L) {
            return null
        }
        return NativeApi.findWebView(appId, path, sessionId)
    }

    private fun closeBrowserTab(tabId: String) {
        if (tabId.isBlank()) return
        NativeApi.browserTabClose(tabId)
    }

    private fun tabTitle(tabId: String): String {
        val view = findManagedWebView(tabId)
        val title = view?.title?.trim()
        if (!title.isNullOrEmpty()) {
            return title
        }
        val url = view?.url?.trim().orEmpty()
        if (url.isEmpty() || url.equals(HIDDEN_NEW_TAB_URL, ignoreCase = true)) {
            return "New Tab"
        }
        return runCatching {
            URI(url).host?.removePrefix("www.")?.takeIf { it.isNotBlank() }
        }.getOrNull() ?: url
    }

    private fun displayUrl(url: String): String {
        return runCatching {
            val uri = URI(url)
            val host = uri.host?.removePrefix("www.")
            if (!host.isNullOrBlank() && uri.scheme in setOf("http", "https")) {
                val path = uri.rawPath?.takeIf { it.isNotBlank() && it != "/" }.orEmpty()
                val query = uri.rawQuery?.let { "?$it" }.orEmpty()
                "$host$path$query"
            } else {
                url
            }
        }.getOrDefault(url)
    }

    private fun normalizeAddressInput(raw: String): String? {
        val input = raw.trim()
        if (input.isEmpty()) {
            return HIDDEN_NEW_TAB_URL
        }
        val explicitScheme = runCatching { URI(input).scheme?.lowercase() }.getOrNull()
        if (explicitScheme == "http" || explicitScheme == "https" || explicitScheme == "lingxia") {
            return input
        }
        val looksLikeHost = input.contains(".") && !input.contains(" ")
        return if (looksLikeHost) "https://$input" else null
    }

    private fun createTabsButton(activity: Activity, onClick: (View) -> Unit): View {
        val frame = FrameLayout(activity).apply {
            layoutParams = LinearLayout.LayoutParams(dp(activity, 38), dp(activity, 38)).apply {
                leftMargin = dp(activity, 2)
                rightMargin = dp(activity, 2)
            }
            val outValue = TypedValue()
            activity.theme.resolveAttribute(
                android.R.attr.selectableItemBackgroundBorderless, outValue, true
            )
            setBackgroundResource(outValue.resourceId)
            isClickable = true
            isFocusable = true
            setOnClickListener { onClick(it) }
        }
        frame.addView(ImageView(activity).apply {
            layoutParams = FrameLayout.LayoutParams(dp(activity, 24), dp(activity, 24), Gravity.CENTER)
            scaleType = ImageView.ScaleType.CENTER_INSIDE
            setImageResource(R.drawable.icon_tabs)
            setColorFilter(Color.parseColor("#333333"))
        })
        val badge = TextView(activity).apply {
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.WRAP_CONTENT,
                ViewGroup.LayoutParams.WRAP_CONTENT,
                Gravity.CENTER
            )
            background = null
            gravity = Gravity.CENTER
            setPadding(0, 0, 0, 0)
            setTextColor(Color.parseColor("#333333"))
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 10f)
            setTypeface(typeface, android.graphics.Typeface.BOLD)
            includeFontPadding = false
            translationX = dp(activity, 2).toFloat()
            translationY = -dp(activity, 2).toFloat()
        }
        frame.addView(badge)
        tabsBadge = badge
        updateTabsBadge()
        return frame
    }

    private fun createIconButton(
        activity: Activity,
        resId: Int,
        sizeDp: Int = 34,
        tint: String = "#333333",
        onClick: (View) -> Unit
    ): ImageView {
        return ImageView(activity).apply {
            layoutParams = LinearLayout.LayoutParams(dp(activity, sizeDp), dp(activity, sizeDp)).apply {
                leftMargin = dp(activity, 2)
                rightMargin = dp(activity, 2)
            }
            scaleType = ImageView.ScaleType.CENTER_INSIDE
            setPadding(dp(activity, 6), dp(activity, 6), dp(activity, 6), dp(activity, 6))
            setImageResource(resId)
            setColorFilter(Color.parseColor(tint))
            val outValue = TypedValue()
            activity.theme.resolveAttribute(
                android.R.attr.selectableItemBackgroundBorderless, outValue, true
            )
            setBackgroundResource(outValue.resourceId)
            isClickable = true
            isFocusable = true
            setOnClickListener { onClick(it) }
        }
    }

    private fun setButtonEnabled(button: ImageView?, enabled: Boolean) {
        button?.isEnabled = enabled
        button?.alpha = if (enabled) 1f else 0.3f
    }

    private fun normalizeTabId(tabId: String): String = tabId.trim()

    private fun hideKeyboard(activity: Activity, target: View?) {
        val inputMethod = activity.getSystemService(Context.INPUT_METHOD_SERVICE) as? InputMethodManager
        inputMethod?.hideSoftInputFromWindow(target?.windowToken, 0)
    }

    private fun dp(activity: Activity, value: Int): Int {
        return (value * activity.resources.displayMetrics.density + 0.5f).toInt()
    }

    private fun getNavigationBarHeight(activity: Activity): Int {
        val resources = activity.resources
        val resourceId = resources.getIdentifier("navigation_bar_height", "dimen", "android")
        return if (resourceId > 0) resources.getDimensionPixelSize(resourceId) else 0
    }
}
