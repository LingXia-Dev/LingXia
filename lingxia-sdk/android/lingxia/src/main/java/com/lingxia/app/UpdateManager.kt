package com.lingxia.app

import com.lingxia.lxapp.LxApp
import com.lingxia.lxapp.R

import android.app.Activity
import android.app.AlertDialog
import android.app.Dialog
import android.app.PendingIntent
import android.app.UiModeManager
import android.content.BroadcastReceiver
import android.content.ClipData
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.content.pm.PackageInstaller
import android.content.pm.PackageManager
import android.content.res.Configuration
import android.graphics.Color
import android.graphics.drawable.GradientDrawable
import android.graphics.drawable.RippleDrawable
import android.graphics.drawable.StateListDrawable
import android.net.Uri
import android.os.Build
import android.os.Environment
import android.os.Looper
import android.provider.Settings
import android.util.Log
import android.util.TypedValue
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.widget.Button
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import android.widget.Toast
import java.io.File
import java.io.FileInputStream
import java.lang.ref.WeakReference
import java.util.concurrent.CountDownLatch
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicReference
import org.json.JSONObject

/**
 * Manages application updates with progress tracking
 */
internal object UpdateManager {
    private const val TAG = "LingXia.UpdateManager"
    private const val APK_MIME_TYPE = "application/vnd.android.package-archive"
    private const val FILE_PROVIDER_SUFFIX = ".fileprovider"
    private const val UPDATE_CACHE_DIR = "lingxia-updates"
    private const val INSTALL_RESULT_ACTION = "com.lingxia.lxapp.INSTALL_RESULT"
    private const val SESSION_WRITE_NAME = "base.apk"
    private const val NOTIFICATION_CHANNEL_ID = "lingxia_update"
    // Stable per-process ID; reusing replaces any previous confirm notification.
    private const val NOTIFICATION_ID_INSTALL_CONFIRM = 0x4C58_5550 // "LXUP"
    private const val UPDATE_PREFS = "lingxia_update"
    private const val KEY_SESSION_INSTALL_RESTRICTED = "session_install_restricted"

    private var activityRef: WeakReference<Activity>? = null
    // AtomicReference so check-and-clear in tryInstallPendingUpdate doesn't
    // race with the LxUpdateInstaller worker thread writing a new pending path.
    private val pendingInstallPath = AtomicReference<String?>(null)
    // A downloaded-and-staged update awaiting the "ready to install" prompt,
    // remembered when the download finishes with no foreground activity. Shown
    // when an activity returns. Distinct from pendingInstallPath, which is the
    // post-permission-grant re-install that proceeds without re-prompting.
    private data class ReadyUpdate(val apkPath: String, val info: ReadyInfo)
    private val pendingReadyInstall = AtomicReference<ReadyUpdate?>(null)
    private val stagedUpdate = AtomicReference<ReadyUpdate?>(null)
    private val sessionUpdates = ConcurrentHashMap<Int, ReadyUpdate>()
    private var readyDialog: WeakReference<Dialog>? = null
    // Store prompt requested before an activity existed (cold-start check).
    private val pendingStoreUpdateInfo = AtomicReference<String?>(null)
    @Volatile private var installReceiver: BroadcastReceiver? = null

    /** Version + release notes shown in the "ready to install" prompt. */
    data class ReadyInfo(
        val version: String,
        val releaseNotes: List<String>
    ) {
        companion object {
            val EMPTY = ReadyInfo("", emptyList())

            fun parse(json: String?): ReadyInfo {
                if (json.isNullOrEmpty()) return EMPTY
                return try {
                    val obj = JSONObject(json)
                    val notesArray = obj.optJSONArray("releaseNotes")
                    val notes = if (notesArray != null) {
                        (0 until notesArray.length()).map { notesArray.getString(it) }
                            .filter { it.isNotBlank() }
                    } else {
                        emptyList()
                    }
                    ReadyInfo(
                        obj.optString("version", ""),
                        notes
                    )
                } catch (e: Exception) {
                    Log.w(TAG, "Failed to parse update info JSON", e)
                    EMPTY
                }
            }
        }
    }

    /**
     * Initialize UpdateManager with the current activity.
     */
    @JvmStatic
    fun init(activity: Activity?) {
        activityRef = if (activity == null) null else WeakReference(activity)
        if (activity != null) {
            tryShowPendingReadyInstall()
            tryShowPendingStoreUpdate()
            tryInstallPendingUpdate()
        }
    }

    private fun resolveActivity(): Activity? {
        return activityRef?.get() ?: LxApp.getCurrentActivity()
    }

    @JvmStatic
    fun tryInstallPendingUpdate() {
        // Don't pre-claim: peek, decide, then clear only if the slot still
        // holds exactly the value we read. A concurrent installUpdate() that
        // writes a fresher path while we're working stays put and is picked
        // up by the next onResume / init — never lost.
        val apkPath = pendingInstallPath.get() ?: return
        val activity = resolveActivity() ?: return
        // Break the settings-redirect loop: if the user returned from the
        // "Unknown sources" screen without granting permission, don't re-open
        // settings on every onResume — drop the pending install (only if it's
        // still our path) and surface a toast so they can re-trigger it
        // intentionally.
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O &&
            !activity.packageManager.canRequestPackageInstalls()
        ) {
            Log.w(TAG, "Discarding pending install: install permission still not granted")
            pendingInstallPath.compareAndSet(apkPath, null)
            showInstallErrorToast(
                activity,
                activity.getString(R.string.lx_update_install_permission_required)
            )
            return
        }
        // Only clear the slot if it's still the value we observed — otherwise
        // a newer pending path took its place and we should leave that alone.
        if (!pendingInstallPath.compareAndSet(apkPath, null)) {
            return
        }
        startInstall(activity, apkPath)
    }

    private fun dp(context: Context, value: Int): Int {
        return TypedValue.applyDimension(
            TypedValue.COMPLEX_UNIT_DIP,
            value.toFloat(),
            context.resources.displayMetrics
        ).toInt()
    }

    /**
     * Detect a TV / set-top-box UI context.
     *
     * Combines two signals so we cover both Android TV (Leanback) and the
     * non-Leanback Chinese smart-TV / STB boxes the muke app targets.
     */
    private fun isTvUi(context: Context): Boolean {
        val uiMode = context.getSystemService(Context.UI_MODE_SERVICE) as? UiModeManager
        if (uiMode?.currentModeType == Configuration.UI_MODE_TYPE_TELEVISION) return true
        return context.packageManager.hasSystemFeature(PackageManager.FEATURE_LEANBACK)
    }

    /**
     * Resolved sizes for the update / progress dialogs. TV mode bumps the
     * dialog width, paddings, font sizes, button heights and notes list
     * height so the dialog stays legible from a ~10ft viewing distance.
     */
    private data class DialogMetrics(
        val isTv: Boolean,
        val containerWidthDp: Int,
        val outerPaddingDp: Int,
        val cornerDp: Int,
        val gapDp: Int,
        val titleSp: Float,
        val bodySp: Float,
        val buttonHeightDp: Int,
        val buttonSp: Float,
        val closeSizeDp: Int,
    )

    private fun metricsFor(context: Context): DialogMetrics {
        return if (isTvUi(context)) {
            DialogMetrics(
                isTv = true,
                containerWidthDp = 560,
                outerPaddingDp = 40,
                cornerDp = 20,
                gapDp = 24,
                titleSp = 28f,
                bodySp = 20f,
                buttonHeightDp = 64,
                buttonSp = 22f,
                closeSizeDp = 48,
            )
        } else {
            DialogMetrics(
                isTv = false,
                containerWidthDp = 320,
                outerPaddingDp = 24,
                cornerDp = 16,
                gapDp = 16,
                titleSp = 20f,
                bodySp = 14f,
                buttonHeightDp = 52,
                buttonSp = 16f,
                closeSizeDp = 36,
            )
        }
    }

    private fun roundedFill(context: Context, color: Int, cornerDp: Int): GradientDrawable {
        return GradientDrawable().apply {
            setColor(color)
            cornerRadius = dp(context, cornerDp).toFloat()
        }
    }

    /**
     * Build the confirm-button background. On TV we add a clearly visible
     * focused state (lighter fill + outer stroke) so D-pad focus is obvious;
     * on phone we keep the flat blue fill that matches the existing look.
     */
    private fun confirmButtonBackground(
        context: Context,
        metrics: DialogMetrics
    ): android.graphics.drawable.Drawable {
        val baseColor = Color.parseColor("#3B82F6")
        val pressedColor = Color.parseColor("#1D4ED8")
        val focusedColor = Color.parseColor("#2563EB")
        val focusStroke = Color.parseColor("#BFDBFE")
        val corner = metrics.cornerDp - 4

        val state = StateListDrawable().apply {
            addState(intArrayOf(android.R.attr.state_pressed), roundedFill(context, pressedColor, corner))
            if (metrics.isTv) {
                addState(
                    intArrayOf(android.R.attr.state_focused),
                    GradientDrawable().apply {
                        setColor(focusedColor)
                        cornerRadius = dp(context, corner).toFloat()
                        setStroke(dp(context, 3), focusStroke)
                    }
                )
            }
            addState(intArrayOf(), roundedFill(context, baseColor, corner))
        }
        // Touch ripple still useful on hybrid devices (TV box with mouse, etc.)
        return RippleDrawable(
            android.content.res.ColorStateList.valueOf(Color.parseColor("#1E40AF")),
            state,
            null
        )
    }

    /**
     * Background for the small close (×) button. On TV we surface a focused
     * circular fill so the user can tell which control is selected.
     */
    private fun closeButtonBackground(
        context: Context,
        metrics: DialogMetrics
    ): android.graphics.drawable.Drawable {
        val focusedFill = Color.parseColor("#E5E7EB")
        val pressedFill = Color.parseColor("#D1D5DB")
        return StateListDrawable().apply {
            addState(intArrayOf(android.R.attr.state_pressed),
                GradientDrawable().apply {
                    setColor(pressedFill)
                    shape = GradientDrawable.OVAL
                })
            if (metrics.isTv) {
                addState(intArrayOf(android.R.attr.state_focused),
                    GradientDrawable().apply {
                        setColor(focusedFill)
                        shape = GradientDrawable.OVAL
                    })
            }
            addState(intArrayOf(), android.graphics.drawable.ColorDrawable(Color.TRANSPARENT))
        }
    }

    /**
     * Post-download "ready to install" prompt. Lightweight by design: the
     * package is already downloaded, so the only decision left is *when* to
     * install. Confirm fires the system installer; the update can be
     * dismissed and re-offered on the next update check.
     */
    /** Scrollable bulleted release-notes block, capped so the dialog stays compact. */
    private fun buildReleaseNotesView(
        activity: Activity,
        metrics: DialogMetrics,
        notes: List<String>
    ): View {
        val maxHeightDp = if (metrics.isTv) 200 else 120
        val scroll = ScrollView(activity).apply {
            if (metrics.isTv) {
                isFocusable = true
                isFocusableInTouchMode = true
            }
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                dp(activity, maxHeightDp)
            ).apply { bottomMargin = dp(activity, metrics.gapDp) }
        }
        val notesContainer = LinearLayout(activity).apply {
            orientation = LinearLayout.VERTICAL
            val p = dp(activity, if (metrics.isTv) 20 else 12)
            setPadding(p, p, p, p)
            background = GradientDrawable().apply {
                setColor(Color.parseColor("#F3F4F6"))
                cornerRadius = dp(activity, if (metrics.isTv) 12 else 8).toFloat()
            }
        }
        notes.forEach { note ->
            val noteView = TextView(activity).apply {
                text = "• $note"
                setTextColor(Color.parseColor("#4B5563"))
                textSize = metrics.bodySp
                layoutParams = LinearLayout.LayoutParams(
                    LinearLayout.LayoutParams.MATCH_PARENT,
                    LinearLayout.LayoutParams.WRAP_CONTENT
                ).apply { bottomMargin = dp(activity, if (metrics.isTv) 8 else 4) }
            }
            notesContainer.addView(noteView)
        }
        scroll.addView(notesContainer)
        return scroll
    }

    private fun createReadyToInstallDialog(
        activity: Activity,
        apkPath: String,
        info: ReadyInfo
    ): Dialog {
        val metrics = metricsFor(activity)
        val dialog = Dialog(activity)
        dialog.window?.setBackgroundDrawableResource(android.R.color.transparent)
        dialog.setCancelable(false)

        val container = LinearLayout(activity).apply {
            orientation = LinearLayout.VERTICAL
            val p = dp(activity, metrics.outerPaddingDp)
            setPadding(p, p, p, p)
            background = GradientDrawable().apply {
                colors = intArrayOf(
                    Color.parseColor("#F8F9FA"),
                    Color.parseColor("#FFFFFF")
                )
                gradientType = GradientDrawable.LINEAR_GRADIENT
                orientation = GradientDrawable.Orientation.TOP_BOTTOM
                cornerRadius = dp(activity, metrics.cornerDp).toFloat()
            }
            layoutParams = ViewGroup.LayoutParams(
                dp(activity, metrics.containerWidthDp),
                ViewGroup.LayoutParams.WRAP_CONTENT
            )
        }

        val header = LinearLayout(activity).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            ).apply { bottomMargin = dp(activity, metrics.gapDp) }
        }

        val titleView = TextView(activity).apply {
            text = activity.getString(R.string.lx_update_ready_title)
            setTextColor(Color.parseColor("#1F2937"))
            textSize = metrics.titleSp
            setTypeface(null, android.graphics.Typeface.BOLD)
            layoutParams = LinearLayout.LayoutParams(
                0,
                LinearLayout.LayoutParams.WRAP_CONTENT,
                1f
            )
        }
        header.addView(titleView)

        val closeButton = ImageView(activity).apply {
            id = View.generateViewId()
            layoutParams = LinearLayout.LayoutParams(
                dp(activity, metrics.closeSizeDp),
                dp(activity, metrics.closeSizeDp)
            )
            setImageResource(R.drawable.icon_close_x)
            setColorFilter(Color.parseColor("#9CA3AF"))
            scaleType = ImageView.ScaleType.CENTER_INSIDE
            val pad = dp(activity, if (metrics.isTv) 10 else 4)
            setPadding(pad, pad, pad, pad)
            background = closeButtonBackground(activity, metrics)
            contentDescription = activity.getString(R.string.lx_common_close)
            isClickable = true
            isFocusable = true
            isFocusableInTouchMode = true
            setOnClickListener { dialog.dismiss() }
        }
        header.addView(closeButton)
        container.addView(header)

        if (info.version.isNotBlank()) {
            val versionView = TextView(activity).apply {
                text = "v${info.version}"
                setTextColor(Color.parseColor("#9CA3AF"))
                textSize = metrics.bodySp
                layoutParams = LinearLayout.LayoutParams(
                    LinearLayout.LayoutParams.MATCH_PARENT,
                    LinearLayout.LayoutParams.WRAP_CONTENT
                ).apply { bottomMargin = dp(activity, if (metrics.isTv) 12 else 8) }
            }
            container.addView(versionView)
        }

        if (info.releaseNotes.isNotEmpty()) {
            container.addView(buildReleaseNotesView(activity, metrics, info.releaseNotes))
        } else {
            val messageView = TextView(activity).apply {
                text = activity.getString(R.string.lx_update_ready_message)
                setTextColor(Color.parseColor("#6B7280"))
                textSize = metrics.bodySp
                layoutParams = LinearLayout.LayoutParams(
                    LinearLayout.LayoutParams.MATCH_PARENT,
                    LinearLayout.LayoutParams.WRAP_CONTENT
                ).apply { bottomMargin = dp(activity, metrics.gapDp) }
            }
            container.addView(messageView)
        }

        val confirmButton = Button(activity).apply {
            id = View.generateViewId()
            text = activity.getString(R.string.lx_update_install_now)
            setTextColor(Color.WHITE)
            textSize = metrics.buttonSp
            setTypeface(null, android.graphics.Typeface.BOLD)
            isAllCaps = false
            isFocusable = true
            isFocusableInTouchMode = true
            background = confirmButtonBackground(activity, metrics)
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                dp(activity, metrics.buttonHeightDp)
            )
            setOnClickListener {
                dialog.dismiss()
                proceedInstall(activity, apkPath)
            }
        }

        closeButton.nextFocusDownId = confirmButton.id
        closeButton.nextFocusForwardId = confirmButton.id
        confirmButton.nextFocusUpId = closeButton.id

        container.addView(confirmButton)
        dialog.setContentView(container)
        dialog.setOnShowListener { confirmButton.requestFocus() }
        return dialog
    }

    private val STORE_INSTALLERS = setOf(
        "com.android.vending",
        "com.google.android.feedback",
        "com.huawei.appmarket",
        "com.xiaomi.market",
        "com.xiaomi.mipicks",
        "com.sec.android.app.samsungapps",
        "com.oppo.market",
        "com.heytap.market",
        "com.vivo.appstore",
        "com.bbk.appstore",
        "com.tencent.android.qqdownloader",
        "com.amazon.venezia",
        "com.hihonor.appmarket",
    )

    private val PLAY_INSTALLERS = setOf("com.android.vending", "com.google.android.feedback")

    /** One listing attempt: `targetPackage` pins the intent to that store. */
    private data class StoreListingIntent(val url: String, val targetPackage: String?)

    /**
     * True when this APK was installed by a store. Sideload / `adb install`
     * (null installer, or the system package installer) is not a store.
     */
    @JvmStatic
    fun installedFromStore(): Boolean {
        val context = Lingxia.applicationContext() ?: return false
        val installer = installerPackage(context) ?: return false
        return STORE_INSTALLERS.contains(installer)
    }

    private fun installerPackage(context: Context): String? {
        val pm = context.packageManager
        return try {
            if (Build.VERSION.SDK_INT >= 30) {
                val info = pm.getInstallSourceInfo(context.packageName)
                val installing = info.installingPackageName
                if (!installing.isNullOrEmpty()) {
                    return installing
                }
                // Only trust initiating when it is a known store — otherwise a
                // sideload session can look like a store install.
                val initiating = info.initiatingPackageName
                initiating?.takeIf { STORE_INSTALLERS.contains(it) }
            } else {
                @Suppress("DEPRECATION")
                pm.getInstallerPackageName(context.packageName)
            }
        } catch (_: PackageManager.NameNotFoundException) {
            null
        }
    }

    @JvmStatic
    fun openUpdateStore(infoJson: String?): Boolean {
        val context = Lingxia.applicationContext() ?: return false
        val pkg = context.packageName
        val installer = installerPackage(context)
        for (candidate in storeListingCandidates(pkg, installer, storeUrlFromInfo(infoJson))) {
            if (tryOpenView(context, candidate)) return true
        }
        Log.w(TAG, "Failed to open store listing installer=$installer pkg=$pkg")
        return false
    }

    /**
     * Open the listing in the store that installed this APK. Every store
     * handles `market://`, so pinning that to the installing package lands on
     * its own listing with no chooser and no Play bounce; the OEM's private
     * scheme and the HTTPS pages are only fallbacks for stores that refuse it.
     */
    private fun storeListingCandidates(
        pkg: String,
        installer: String?,
        baked: String?,
    ): List<StoreListingIntent> {
        val out = ArrayList<StoreListingIntent>()
        fun add(url: String, targetPackage: String?) {
            if (url.isBlank()) return
            val candidate = StoreListingIntent(url, targetPackage)
            if (candidate !in out) out.add(candidate)
        }
        if (installer == null || installer !in STORE_INSTALLERS) {
            add("market://details?id=$pkg", null)
            add("https://play.google.com/store/apps/details?id=$pkg", null)
            baked?.let { add(it, null) }
            return out
        }
        add("market://details?id=$pkg", installer)
        for (scheme in oemStoreSchemes(pkg, installer)) {
            add(scheme, installer)
            add(scheme, null)
        }
        for (url in storeHttpsListings(pkg, installer)) {
            add(url, null)
        }
        baked?.let { add(it, null) }
        return out
    }

    private fun oemStoreSchemes(pkg: String, installer: String): List<String> = when (installer) {
        "com.huawei.appmarket", "com.hihonor.appmarket" -> listOf("appmarket://details?id=$pkg")
        "com.xiaomi.market", "com.xiaomi.mipicks" -> listOf("mimarket://details?id=$pkg")
        "com.oppo.market", "com.heytap.market" -> listOf("oppomarket://details?packagename=$pkg")
        "com.vivo.appstore", "com.bbk.appstore" -> listOf("vivomarket://details?id=$pkg")
        "com.sec.android.app.samsungapps" -> listOf("samsungapps://ProductDetail/$pkg")
        "com.amazon.venezia" -> listOf("amzn://apps/android?p=$pkg")
        "com.tencent.android.qqdownloader" -> listOf("tmast://appdetails?pname=$pkg")
        else -> emptyList()
    }

    private fun storeHttpsListings(pkg: String, installer: String): List<String> = when (installer) {
        in PLAY_INSTALLERS -> listOf("https://play.google.com/store/apps/details?id=$pkg")
        "com.amazon.venezia" -> listOf("https://www.amazon.com/gp/mas/dl/android?p=$pkg")
        else -> emptyList()
    }

    private fun tryOpenView(context: Context, candidate: StoreListingIntent): Boolean {
        return try {
            val intent = Intent(Intent.ACTION_VIEW, Uri.parse(candidate.url)).apply {
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                candidate.targetPackage?.let { setPackage(it) }
            }
            context.startActivity(intent)
            true
        } catch (_: Exception) {
            false
        }
    }

    /** @return true once the prompt is shown or deferred to the next activity. */
    @JvmStatic
    fun presentStoreUpdate(infoJson: String?): Boolean {
        val activity = resolveActivity()
        if (activity == null || activity.isFinishing || activity.isDestroyed) {
            Log.w(TAG, "No current activity; deferring store update prompt")
            pendingStoreUpdateInfo.set(infoJson ?: "")
            return true
        }
        showStoreUpdatePrompt(activity, infoJson)
        return true
    }

    @JvmStatic
    fun tryShowPendingStoreUpdate() {
        val infoJson = pendingStoreUpdateInfo.getAndSet(null) ?: return
        val activity = resolveActivity()
        if (activity == null) {
            pendingStoreUpdateInfo.set(infoJson)
            return
        }
        showStoreUpdatePrompt(activity, infoJson)
    }

    private fun showStoreUpdatePrompt(activity: Activity, infoJson: String?) {
        val info = ReadyInfo.parse(infoJson)
        activity.runOnUiThread {
            if (activity.isFinishing || activity.isDestroyed) {
                pendingStoreUpdateInfo.set(infoJson ?: "")
                return@runOnUiThread
            }
            val message = if (info.releaseNotes.isNotEmpty()) {
                info.releaseNotes.joinToString("\n")
            } else {
                activity.getString(R.string.lx_update_store_message)
            }
            AlertDialog.Builder(activity)
                .setTitle(activity.getString(R.string.lx_update_store_title))
                .setMessage(message)
                .setPositiveButton(activity.getString(R.string.lx_update_open_store)) { _, _ ->
                    openUpdateStore(infoJson)
                }
                .setNegativeButton(activity.getString(R.string.lx_common_close), null)
                .setCancelable(true)
                .show()
        }
    }

    private fun storeUrlFromInfo(infoJson: String?): String? {
        if (infoJson.isNullOrEmpty()) return null
        return try {
            val url = JSONObject(infoJson).optString("storeUrl", "").trim()
            url.takeIf { it.isNotEmpty() }
        } catch (_: Exception) {
            null
        }
    }

    /**
     * Hand off a downloaded-and-verified update for installation.
     *
     * The package was downloaded silently in the background. Rather than pop the
     * system installer unprompted, present a lightweight "ready to install"
     * prompt; the system installer fires only when the user confirms. When no
     * activity is in the foreground the update is remembered and the prompt is
     * shown on return.
     *
     * @param apkPath Absolute file path to the downloaded APK file
     * @param infoJson `{version, releaseNotes}` shown in the prompt
     * @return true once the request has been accepted (prompt shown or deferred)
     */
    @JvmStatic
    fun installUpdate(apkPath: String, infoJson: String?): Boolean {
        val info = ReadyInfo.parse(infoJson)
        val update = ReadyUpdate(apkPath, info)
        stagedUpdate.set(update)
        pendingReadyInstall.set(null)
        val activity = resolveActivity()
        if (activity == null) {
            Log.w(TAG, "No current activity; deferring ready-to-install prompt")
            pendingReadyInstall.set(update)
            return true
        }
        showReadyToInstallPrompt(activity, apkPath, info)
        return true
    }

    @JvmStatic
    fun tryShowPendingReadyInstall() {
        val update = pendingReadyInstall.get() ?: return
        val activity = resolveActivity()
        if (activity == null || !pendingReadyInstall.compareAndSet(update, null)) return
        showReadyToInstallPrompt(activity, update.apkPath, update.info)
    }

    private fun showReadyToInstallPrompt(
        activity: Activity,
        apkPath: String,
        info: ReadyInfo
    ) {
        activity.runOnUiThread {
            if (stagedUpdate.get() != ReadyUpdate(apkPath, info)) return@runOnUiThread
            if (activity.isFinishing || activity.isDestroyed) {
                pendingReadyInstall.set(ReadyUpdate(apkPath, info))
                return@runOnUiThread
            }
            readyDialog?.get()?.dismiss()
            val dialog = createReadyToInstallDialog(activity, apkPath, info)
            readyDialog = WeakReference(dialog)
            dialog.setOnDismissListener {
                if (readyDialog?.get() === dialog) readyDialog = null
            }
            dialog.show()
        }
    }

    private fun offerInstallRetry(update: ReadyUpdate) {
        // Restore a user-controlled prompt, never automatically install again.
        android.os.Handler(android.os.Looper.getMainLooper()).post {
            if (stagedUpdate.get() != update || !File(update.apkPath).isFile) return@post
            pendingReadyInstall.set(update)
            tryShowPendingReadyInstall()
        }
    }

    /**
     * Run the actual install: validate, request the unknown-sources permission
     * if needed, then launch the system installer. Always invoked from the
     * "ready to install" prompt confirm handler (UI thread).
     */
    private fun proceedInstall(activity: Activity, apkPath: String) {
        if (!validateInstallRequest(activity, apkPath)) {
            return
        }
        if (needsInstallPermission(activity)) {
            requestInstallPermission(activity, apkPath)
            return
        }
        startInstall(activity, apkPath)
    }

    private fun validateInstallRequest(activity: Activity, apkPath: String): Boolean {
        val apkFile = File(apkPath)
        if (!apkFile.exists() || apkFile.length() == 0L) {
            LxLog.e(TAG, "APK file missing or empty: $apkPath (len=${apkFile.length()})")
            showInstallErrorToast(activity, activity.getString(R.string.lx_update_install_apk_empty))
            return false
        }

        val pkgInfo = activity.packageManager.getPackageArchiveInfo(apkPath, 0)
        if (pkgInfo == null) {
            LxLog.e(TAG, "Invalid APK (cannot parse package): $apkPath")
            showInstallErrorToast(activity, activity.getString(R.string.lx_update_install_apk_invalid))
            return false
        }
        if (pkgInfo.packageName != activity.packageName) {
            LxLog.e(
                TAG,
                "APK package mismatch: ${pkgInfo.packageName} vs ${activity.packageName}"
            )
            showInstallErrorToast(activity, activity.getString(R.string.lx_update_install_apk_mismatch))
            return false
        }

        // Intentionally do NOT fail here when unknown-sources settings can't
        // be resolved: stripped MIUI TV builds (e.g. Mi TV Stick) ship without
        // any "Install unknown apps" UI at all, but their packageinstaller
        // activity often still accepts ACTION_VIEW installs. We rely on the
        // fall-through in requestInstallPermission() to attempt the install
        // anyway and surface whatever the system installer says.
        return true
    }

    private fun needsInstallPermission(activity: Activity): Boolean {
        return Build.VERSION.SDK_INT >= Build.VERSION_CODES.O &&
            !activity.packageManager.canRequestPackageInstalls()
    }

    // UI-thread callers must not parse or copy an APK inline; run validation
    // and the install on a worker. Only a failed launch is re-offered — a
    // rejected APK fails identically every time. Non-UI callers use
    // launchInstaller() directly so Rust can observe "request failed" vs
    // "request launched".
    private fun startInstall(activity: Activity, apkPath: String) {
        val update = stagedUpdate.get()?.takeIf { it.apkPath == apkPath }
        Thread({
            if (!validateInstallRequest(activity, apkPath)) return@Thread
            if (!launchInstaller(activity, apkPath) && update != null) {
                offerInstallRetry(update)
            }
        }, "LxUpdateInstaller").start()
    }

    /** Launches an installer for an already-validated APK. */
    private fun launchInstaller(activity: Activity, apkPath: String): Boolean {
        try {
            val apkFile = File(apkPath)
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                if (!activity.packageManager.canRequestPackageInstalls()) {
                    return requestInstallPermission(activity, apkPath)
                }
            }

            if (sessionInstallRestricted(activity)) {
                Log.i(TAG, "Session installs are restricted here; using ACTION_VIEW installer")
                return launchInstallerLegacy(activity, apkFile)
            }

            // Primary path: PackageInstaller Session API.
            // Reasons: surfaces failure status, works on Android TV / restricted devices
            // where PackageInstallerActivity may be absent, and on API 31+ enables silent
            // self-update when signatures match.
            if (installViaSession(activity, apkFile)) {
                return true
            }

            // Fallback: legacy ACTION_VIEW intent (some OEM ROMs reject session installs).
            Log.w(TAG, "Session install failed to commit; falling back to ACTION_VIEW")
            return launchInstallerLegacy(activity, apkFile)
        } catch (e: Exception) {
            LxLog.e(TAG, "Failed to install update from: $apkPath", e)
            showInstallErrorToast(
                activity,
                activity.getString(R.string.lx_update_install_failure)
            )
            return false
        }
    }

    private fun requestInstallPermission(activity: Activity, apkPath: String): Boolean {
        Log.w(TAG, "Install permission not granted; opening settings")
        if (openUnknownSourcesSettings(activity)) {
            pendingInstallPath.set(apkPath)
            return true
        }
        // No Settings UI exists to grant unknown-sources (e.g. stripped MIUI
        // TV / Mi TV Stick where the action filters were stripped from the
        // system Settings app). Falling through to the legacy ACTION_VIEW
        // path hands the APK off to com.android.packageinstaller, which on
        // such builds often accepts the install directly or surfaces its
        // own actionable error — better than dead-ending the user here.
        Log.w(TAG, "No settings UI available; trying legacy installer directly")
        return launchInstallerLegacy(activity, File(apkPath))
    }

    private fun openUnknownSourcesSettings(activity: Activity): Boolean {
        val resolved = resolveUnknownSourcesSettingsIntent(activity)
        if (resolved == null) {
            LxLog.e(TAG, "No settings activity available to grant install permission")
            return false
        }
        return runOnUiThreadForResult(activity) {
            activity.startActivity(resolved)
            true
        }
    }

    private fun resolveUnknownSourcesSettingsIntent(activity: Activity): Intent? {
        return listOf(
            Intent(
                Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES,
                Uri.parse("package:${activity.packageName}")
            ).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
            // Fallback: some OEM ROMs / TVs don't honor the per-app variant.
            Intent(Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES)
                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
            Intent(Settings.ACTION_SECURITY_SETTINGS)
                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        ).firstOrNull {
            it.resolveActivity(activity.packageManager) != null
        }
    }

    private fun installViaSession(activity: Activity, apkFile: File): Boolean {
        var createdSessionId: Int? = null
        return try {
            ensureInstallReceiver(activity.applicationContext)
            val packageInstaller = activity.packageManager.packageInstaller
            val params = PackageInstaller.SessionParams(
                PackageInstaller.SessionParams.MODE_FULL_INSTALL
            ).apply {
                setAppPackageName(activity.packageName)
                setSize(apkFile.length())
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
                    // Best-effort: honored only for self-updates with matching signatures.
                    setRequireUserAction(PackageInstaller.SessionParams.USER_ACTION_NOT_REQUIRED)
                }
            }

            val sessionId = packageInstaller.createSession(params)
            createdSessionId = sessionId
            stagedUpdate.get()?.takeIf { it.apkPath == apkFile.path }?.let {
                sessionUpdates[sessionId] = it
            }
            packageInstaller.openSession(sessionId).use { session ->
                FileInputStream(apkFile).use { input ->
                    session.openWrite(SESSION_WRITE_NAME, 0, apkFile.length()).use { out ->
                        input.copyTo(out)
                        session.fsync(out)
                    }
                }
                val statusIntent = Intent(INSTALL_RESULT_ACTION)
                    .setPackage(activity.packageName)
                val pendingFlags = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
                    PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_MUTABLE
                } else {
                    PendingIntent.FLAG_UPDATE_CURRENT
                }
                val pending = PendingIntent.getBroadcast(
                    activity.applicationContext,
                    sessionId,
                    statusIntent,
                    pendingFlags
                )
                session.commit(pending.intentSender)
            }
            Log.i(TAG, "PackageInstaller session committed: id=$sessionId, apk=${apkFile.path}")
            true
        } catch (e: SecurityException) {
            createdSessionId?.let { abandonFailedSession(activity, it) }
            LxLog.e(TAG, "Session install denied by system", e)
            false
        } catch (e: Exception) {
            createdSessionId?.let { abandonFailedSession(activity, it) }
            LxLog.e(TAG, "Session install failed", e)
            false
        }
    }

    // Remembered across launches so a slow TV stick doesn't copy the whole APK
    // into a session the package manager will reject again.
    private fun sessionInstallRestricted(context: Context): Boolean = try {
        context.applicationContext
            .getSharedPreferences(UPDATE_PREFS, Context.MODE_PRIVATE)
            .getBoolean(KEY_SESSION_INSTALL_RESTRICTED, false)
    } catch (e: Exception) {
        Log.w(TAG, "Could not read install policy prefs: ${e.message}")
        false
    }

    private fun rememberSessionInstallRestricted(context: Context) {
        try {
            context.applicationContext
                .getSharedPreferences(UPDATE_PREFS, Context.MODE_PRIVATE)
                .edit()
                .putBoolean(KEY_SESSION_INSTALL_RESTRICTED, true)
                .apply()
        } catch (e: Exception) {
            Log.w(TAG, "Could not persist install policy prefs: ${e.message}")
        }
    }

    private fun abandonFailedSession(activity: Activity, sessionId: Int) {
        sessionUpdates.remove(sessionId)
        try {
            activity.packageManager.packageInstaller.abandonSession(sessionId)
        } catch (e: Exception) {
            Log.w(TAG, "Could not abandon failed install session $sessionId", e)
        }
    }

    private fun launchInstallerLegacy(activity: Activity, apkFile: File): Boolean {
        val apkUri = resolveInstallUri(activity, apkFile)

        val intent = Intent(Intent.ACTION_VIEW).apply {
            setDataAndType(apkUri, APK_MIME_TYPE)
            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            if (usesFileProviderUri()) {
                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
                clipData = ClipData.newUri(
                    activity.contentResolver,
                    "LingXia update APK",
                    apkUri
                )
            }
        }
        if (usesFileProviderUri()) {
            grantInstallerReadPermissions(activity, intent, apkUri)
        }

        if (intent.resolveActivity(activity.packageManager) == null) {
            LxLog.e(TAG, "No installer activity available on this device (likely TV/restricted)")
            showInstallErrorToast(
                activity,
                activity.getString(R.string.lx_update_install_no_ui)
            )
            return false
        }

        // Worker thread → hop to main for Activity.startActivity and report
        // whether the request was actually launched.
        val launched = runOnUiThreadForResult(activity) {
            activity.startActivity(intent)
            true
        }
        if (!launched) {
            LxLog.e(TAG, "Failed to launch ACTION_VIEW installer")
            showInstallErrorToast(
                activity,
                activity.getString(R.string.lx_update_install_failure)
            )
        }
        return launched
    }

    private fun runOnUiThreadForResult(
        activity: Activity,
        action: () -> Boolean
    ): Boolean {
        fun runAction(): Boolean = try {
            action()
        } catch (e: Exception) {
            Log.w(TAG, "UI action failed: ${e.message}")
            false
        }

        if (Looper.myLooper() == Looper.getMainLooper()) {
            return runAction()
        }

        val latch = CountDownLatch(1)
        val result = AtomicReference(false)
        activity.runOnUiThread {
            result.set(runAction())
            latch.countDown()
        }
        return if (latch.await(3, TimeUnit.SECONDS)) {
            result.get()
        } else {
            Log.w(TAG, "Timed out waiting for UI action")
            false
        }
    }

    private fun ensureInstallReceiver(context: Context) {
        if (installReceiver != null) return
        val receiver = object : BroadcastReceiver() {
            override fun onReceive(ctx: Context, intent: Intent) {
                if (intent.action != INSTALL_RESULT_ACTION) return
                handleInstallStatus(ctx, intent)
            }
        }
        val filter = IntentFilter(INSTALL_RESULT_ACTION)
        val appCtx = context.applicationContext
        try {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                appCtx.registerReceiver(receiver, filter, Context.RECEIVER_NOT_EXPORTED)
            } else {
                @Suppress("UnspecifiedRegisterReceiverFlag")
                appCtx.registerReceiver(receiver, filter)
            }
            installReceiver = receiver
        } catch (e: Exception) {
            LxLog.e(TAG, "Failed to register install result receiver", e)
        }
    }

    private fun handleInstallStatus(ctx: Context, intent: Intent) {
        val status = intent.getIntExtra(PackageInstaller.EXTRA_STATUS, -1)
        val message = intent.getStringExtra(PackageInstaller.EXTRA_STATUS_MESSAGE) ?: ""
        val sessionId = intent.getIntExtra(PackageInstaller.EXTRA_SESSION_ID, -1)
        when (status) {
            PackageInstaller.STATUS_PENDING_USER_ACTION -> {
                @Suppress("DEPRECATION")
                val confirm = intent.getParcelableExtra<Intent>(Intent.EXTRA_INTENT)
                if (confirm == null) {
                    LxLog.e(TAG, "STATUS_PENDING_USER_ACTION but EXTRA_INTENT is null")
                    showInstallErrorToast(
                        ctx,
                        ctx.getString(R.string.lx_update_install_failure)
                    )
                    return
                }
                confirm.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)

                // Prefer a direct foreground launch from an Activity: that
                // path is exempt from the Android 10+ background-activity
                // restrictions that can silently swallow startActivity from
                // a BroadcastReceiver. Fall back to a notification when
                // there is no resumed activity available.
                val activity = resolveActivity()
                val launchedFromActivity = activity != null &&
                    !activity.isFinishing &&
                    !activity.isDestroyed &&
                    tryStartActivity(activity, confirm)
                if (!launchedFromActivity) {
                    Log.w(TAG, "No foreground activity for install confirm; posting notification")
                    postInstallConfirmNotification(ctx, confirm)
                }
            }
            PackageInstaller.STATUS_SUCCESS -> {
                sessionUpdates.remove(sessionId)?.let { stagedUpdate.compareAndSet(it, null) }
                Log.i(TAG, "Install succeeded")
            }
            else -> {
                LxLog.e(TAG, "Install failed: status=$status msg=\"$message\"")
                val update = sessionUpdates.remove(sessionId)
                if (isSessionInstallRestricted(message)) {
                    rememberSessionInstallRestricted(ctx)
                }
                val action = installFailureAction(status, message, update != null)
                if (update == null || action == InstallFailureAction.NONE) {
                    showInstallFailure(ctx, status, message)
                    return
                }
                if (action == InstallFailureAction.LEGACY_INSTALLER) {
                    retryViaSystemInstaller(ctx, update, status, message)
                    return
                }
                showInstallFailure(ctx, status, message)
                offerInstallRetry(update)
            }
        }
    }

    /**
     * Hand a session-rejected update to the system installer UI, which owns the
     * consent such ROMs withhold from ordinary-app sessions. Runs on a worker:
     * the receiver is on the main thread and the legacy path stages (copies)
     * the APK before launching.
     */
    private fun retryViaSystemInstaller(
        ctx: Context,
        update: ReadyUpdate,
        status: Int,
        message: String
    ) {
        val activity = resolveActivity()?.takeIf { !it.isFinishing && !it.isDestroyed }
        if (activity == null) {
            showInstallFailure(ctx, status, message)
            offerInstallRetry(update)
            return
        }
        Thread({
            if (launchInstallerLegacy(activity, File(update.apkPath))) return@Thread
            showInstallFailure(ctx, status, message)
            offerInstallRetry(update)
        }, "LxUpdateInstaller").start()
    }

    private fun showInstallFailure(ctx: Context, status: Int, message: String) {
        showInstallErrorToast(ctx, ctx.getString(updateInstallErrorResource(status, message)))
    }

    private fun tryStartActivity(activity: Activity, intent: Intent): Boolean {
        return try {
            activity.startActivity(intent)
            true
        } catch (e: Exception) {
            Log.w(TAG, "Activity.startActivity for install confirm failed: ${e.message}")
            false
        }
    }

    /**
     * Surface the system install-confirm intent through a notification so
     * the user can still complete the install when the app is in the
     * background (the BroadcastReceiver path can't reliably start an
     * activity on Android 10+). Silently degrades on Android 13+ devices
     * where POST_NOTIFICATIONS hasn't been granted — that's no worse than
     * the prior behavior of losing the confirm UI entirely.
     */
    private fun postInstallConfirmNotification(ctx: Context, confirm: Intent) {
        val appCtx = ctx.applicationContext
        try {
            ensureNotificationChannel(appCtx)
            val pendingFlags = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
                PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
            } else {
                PendingIntent.FLAG_UPDATE_CURRENT
            }
            val pending = PendingIntent.getActivity(
                appCtx,
                NOTIFICATION_ID_INSTALL_CONFIRM,
                confirm,
                pendingFlags
            )
            val builder = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                android.app.Notification.Builder(appCtx, NOTIFICATION_CHANNEL_ID)
            } else {
                @Suppress("DEPRECATION")
                android.app.Notification.Builder(appCtx)
                    .setPriority(android.app.Notification.PRIORITY_HIGH)
            }
            val notification = builder
                .setSmallIcon(android.R.drawable.stat_sys_download_done)
                .setContentTitle(appCtx.getString(R.string.lx_update_notification_title))
                .setContentText(appCtx.getString(R.string.lx_update_notification_body))
                .setContentIntent(pending)
                .setAutoCancel(true)
                .build()
            val nm = appCtx.getSystemService(Context.NOTIFICATION_SERVICE)
                as? android.app.NotificationManager
            if (nm == null) {
                Log.w(TAG, "NotificationManager unavailable")
                return
            }
            nm.notify(NOTIFICATION_ID_INSTALL_CONFIRM, notification)
        } catch (e: Exception) {
            Log.w(TAG, "Failed to post install confirm notification: ${e.message}")
        }
    }

    private fun ensureNotificationChannel(appCtx: Context) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
        val nm = appCtx.getSystemService(Context.NOTIFICATION_SERVICE)
            as? android.app.NotificationManager ?: return
        if (nm.getNotificationChannel(NOTIFICATION_CHANNEL_ID) != null) return
        val channel = android.app.NotificationChannel(
            NOTIFICATION_CHANNEL_ID,
            appCtx.getString(R.string.lx_update_notification_channel),
            android.app.NotificationManager.IMPORTANCE_HIGH
        )
        nm.createNotificationChannel(channel)
    }

    private fun showInstallErrorToast(context: Context, message: String) {
        try {
            android.os.Handler(android.os.Looper.getMainLooper()).post {
                Toast.makeText(context.applicationContext, message, Toast.LENGTH_LONG).show()
            }
        } catch (e: Exception) {
            Log.w(TAG, "Failed to show install error toast: ${e.message}")
        }
    }

    private fun resolveInstallUri(activity: Activity, apkFile: File): Uri {
        if (!usesFileProviderUri()) {
            return Uri.fromFile(stageApkForLegacyInstaller(activity, apkFile))
        }

        return try {
            fileProviderUri(activity, apkFile)
        } catch (e: IllegalArgumentException) {
            Log.w(TAG, "APK path is outside FileProvider roots; staging to cache: ${apkFile.path}")
            fileProviderUri(activity, stageApkInCache(activity, apkFile))
        }
    }

    private fun usesFileProviderUri(): Boolean {
        return android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.N
    }

    private fun fileProviderUri(activity: Activity, apkFile: File): Uri {
        return androidx.core.content.FileProvider.getUriForFile(
            activity,
            "${activity.packageName}$FILE_PROVIDER_SUFFIX",
            apkFile
        )
    }

    private fun stageApkForLegacyInstaller(activity: Activity, apkFile: File): File {
        val updateDir = activity.getExternalFilesDir(Environment.DIRECTORY_DOWNLOADS)
            ?: activity.externalCacheDir
            ?: throw IllegalStateException("No external directory available for legacy APK install")
        if (!updateDir.exists() && !updateDir.mkdirs()) {
            throw IllegalStateException("Failed to create update dir: ${updateDir.path}")
        }

        val fileName = apkFile.name.takeIf { it.isNotBlank() } ?: "update.apk"
        val stagedFile = File(updateDir, fileName)
        if (apkFile.canonicalPath != stagedFile.canonicalPath) {
            apkFile.copyTo(stagedFile, overwrite = true)
        }
        return stagedFile
    }

    private fun stageApkInCache(activity: Activity, apkFile: File): File {
        val updateDir = File(activity.cacheDir, UPDATE_CACHE_DIR)
        if (!updateDir.exists() && !updateDir.mkdirs()) {
            throw IllegalStateException("Failed to create update cache dir: ${updateDir.path}")
        }

        val fileName = apkFile.name.takeIf { it.isNotBlank() } ?: "update.apk"
        val stagedFile = File(updateDir, fileName)
        if (apkFile.canonicalPath != stagedFile.canonicalPath) {
            apkFile.copyTo(stagedFile, overwrite = true)
        }
        return stagedFile
    }

    private fun grantInstallerReadPermissions(activity: Activity, intent: Intent, apkUri: Uri) {
        val flags = Intent.FLAG_GRANT_READ_URI_PERMISSION
        val installers = activity.packageManager.queryIntentActivities(
            intent,
            PackageManager.MATCH_DEFAULT_ONLY
        )
        installers.forEach { resolveInfo ->
            val packageName = resolveInfo.activityInfo?.packageName ?: return@forEach
            activity.grantUriPermission(packageName, apkUri, flags)
        }
    }
}
