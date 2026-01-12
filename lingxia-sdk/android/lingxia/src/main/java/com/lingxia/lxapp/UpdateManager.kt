package com.lingxia.lxapp

import android.app.Dialog
import android.content.Intent
import android.graphics.Color
import android.graphics.drawable.GradientDrawable
import android.net.Uri
import android.provider.Settings
import android.util.Log
import android.util.TypedValue
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.widget.Button
import android.widget.LinearLayout
import android.widget.ProgressBar
import android.widget.ScrollView
import android.widget.TextView
import java.io.File
import java.lang.ref.WeakReference
import java.util.concurrent.CountDownLatch
import org.json.JSONObject

/**
 * Manages application updates with progress tracking
 */
object UpdateManager {
    private const val TAG = "LingXia.UpdateManager"

    private var progressDialog: Dialog? = null
    private var progressBar: ProgressBar? = null
    private var progressText: TextView? = null
    private var pendingUpdatePromptCallbackId: Long? = null
    private var activityRef: WeakReference<LxAppActivity>? = null
    private var pendingInstallPath: String? = null

    /**
     * Initialize UpdateManager with the current activity.
     */
    @JvmStatic
    fun init(activity: LxAppActivity?) {
        activityRef = if (activity == null) null else WeakReference(activity)
        if (activity != null) {
            tryShowPendingPrompt()
            tryInstallPendingUpdate()
        }
    }

    private fun resolveActivity(): LxAppActivity? {
        return activityRef?.get() ?: LxApp.getCurrentActivity()
    }

    /**
     * Show download progress dialog
     */
    @JvmStatic
    fun showDownloadProgress() {
        val activity = resolveActivity() ?: run {
            Log.e(TAG, "No current activity to show progress dialog")
            return
        }

        activity.runOnUiThread {
            if (progressDialog?.isShowing == true) {
                return@runOnUiThread
            }
            dismissDownloadProgressInternal()

            val context = activity.applicationContext
            val title = context.getString(R.string.lx_update_downloading)

            // Create a custom dialog
            val dialog = Dialog(activity)
            dialog.window?.setBackgroundDrawableResource(android.R.color.transparent)
            dialog.setCancelable(false)

            // Create a container with title and progress
            val container = LinearLayout(activity).apply {
                orientation = LinearLayout.VERTICAL
                setPadding(48, 48, 48, 48)

                val bg = GradientDrawable().apply {
                    setColor(Color.WHITE)
                    cornerRadius = 16f
                }
                background = bg

                layoutParams = ViewGroup.LayoutParams(
                    ViewGroup.LayoutParams.WRAP_CONTENT,
                    ViewGroup.LayoutParams.WRAP_CONTENT
                )
            }

            // Title
            val titleView = TextView(activity).apply {
                text = title
                setTextColor(Color.parseColor("#1F2937"))
                textSize = 18f
                setTypeface(null, android.graphics.Typeface.BOLD)
                layoutParams = LinearLayout.LayoutParams(
                    LinearLayout.LayoutParams.WRAP_CONTENT,
                    LinearLayout.LayoutParams.WRAP_CONTENT
                ).apply {
                    bottomMargin = 24
                }
            }

            progressText = TextView(activity).apply {
                text = "0%"
                textSize = 16f
                setTextColor(Color.parseColor("#6B7280"))
                gravity = android.view.Gravity.CENTER
                layoutParams = LinearLayout.LayoutParams(
                    LinearLayout.LayoutParams.MATCH_PARENT,
                    LinearLayout.LayoutParams.WRAP_CONTENT
                ).apply {
                    bottomMargin = 8
                }
            }

            progressBar = ProgressBar(activity, null, android.R.attr.progressBarStyleHorizontal).apply {
                max = 100
                progress = 0
                isIndeterminate = true
                layoutParams = LinearLayout.LayoutParams(
                    300,
                    LinearLayout.LayoutParams.WRAP_CONTENT
                )
            }

            container.addView(titleView)
            container.addView(progressText)
            container.addView(progressBar)

            dialog.setContentView(container)
            progressDialog = dialog
            progressDialog?.show()
        }
    }

    /**
     * Update download progress
     * Called from Rust layer during download
     *
     * @param progress Progress percentage (0-100)
     */
    @JvmStatic
    fun updateDownloadProgress(progress: Int) {
        val activity = resolveActivity() ?: return

        activity.runOnUiThread {
            progressBar?.isIndeterminate = false
            progressBar?.progress = progress
            progressText?.text = "$progress%"
        }
    }

    /**
     * Dismiss download progress dialog
     */
    @JvmStatic
    fun dismissDownloadProgress() {
        if (android.os.Looper.myLooper() == android.os.Looper.getMainLooper()) {
            dismissDownloadProgressInternal()
            return
        }

        android.os.Handler(android.os.Looper.getMainLooper()).post {
            dismissDownloadProgressInternal()
        }
    }

    private fun dismissDownloadProgressInternal() {
        try {
            progressDialog?.dismiss()
        } catch (e: Exception) {
            Log.w(TAG, "Failed to dismiss progress dialog: ${e.message}")
        } finally {
            progressDialog = null
            progressBar = null
            progressText = null
        }
    }

    private var pendingUpdateInfo: UpdateInfo? = null

    data class UpdateInfo(
        val version: String,
        val sizeBytes: Long,
        val releaseNotes: List<String>?
    )

    /**
     * Prompt the user to confirm update installation.
     *
     * @param callbackId Callback ID for result
     * @param updateInfoJson Optional JSON with update details: {"version":"1.2.0","size":15728640,"releaseNotes":["..."]}
     */
    @JvmStatic
    fun showUpdatePrompt(callbackId: Long, updateInfoJson: String? = null) {
        val updateInfo = parseUpdateInfo(updateInfoJson)
        val activity = resolveActivity()
        if (activity == null) {
            Log.w(TAG, "No current activity; deferring update prompt")
            pendingUpdatePromptCallbackId = callbackId
            pendingUpdateInfo = updateInfo
            return
        }
        showUpdatePromptInternal(activity, callbackId, updateInfo)
    }

    @JvmStatic
    fun tryShowPendingPrompt() {
        val callbackId = pendingUpdatePromptCallbackId ?: return
        val activity = resolveActivity() ?: return
        val info = pendingUpdateInfo
        pendingUpdatePromptCallbackId = null
        pendingUpdateInfo = null
        showUpdatePromptInternal(activity, callbackId, info)
    }

    @JvmStatic
    fun tryInstallPendingUpdate() {
        val apkPath = pendingInstallPath ?: return
        val activity = resolveActivity() ?: return
        pendingInstallPath = null
        if (!launchInstaller(activity, apkPath)) {
            Log.e(TAG, "Failed to launch installer for pending update: $apkPath")
        }
    }

    private fun parseUpdateInfo(json: String?): UpdateInfo? {
        if (json.isNullOrEmpty()) return null
        return try {
            val obj = JSONObject(json)
            val version = obj.optString("version", "")
            val size = obj.optLong("size", 0L)
            val notesArray = obj.optJSONArray("releaseNotes")
            val notes = if (notesArray != null) {
                (0 until notesArray.length()).map { notesArray.getString(it) }
            } else null

            UpdateInfo(version, size, notes)
        } catch (e: Exception) {
            Log.w(TAG, "Failed to parse update info JSON", e)
            null
        }
    }

    private fun showUpdatePromptInternal(
        activity: LxAppActivity,
        callbackId: Long,
        updateInfo: UpdateInfo?
    ) {
        activity.runOnUiThread {
            if (activity.isFinishing || activity.isDestroyed) {
                Log.w(TAG, "Activity not ready for update prompt dialog")
                NativeApi.onCallback(callbackId, false, "1000")
                return@runOnUiThread
            }

            val dialog = createUpdateDialog(activity, callbackId, updateInfo)
            dialog.show()
        }
    }

    private fun dp(context: android.content.Context, value: Int): Int {
        return TypedValue.applyDimension(
            TypedValue.COMPLEX_UNIT_DIP,
            value.toFloat(),
            context.resources.displayMetrics
        ).toInt()
    }

    private fun createUpdateDialog(
        activity: LxAppActivity,
        callbackId: Long,
        updateInfo: UpdateInfo?
    ): Dialog {
        val dialog = Dialog(activity)
        dialog.window?.setBackgroundDrawableResource(android.R.color.transparent)
        dialog.setCancelable(false)

        // Main container with gradient background
        val container = LinearLayout(activity).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(dp(activity, 24), dp(activity, 24), dp(activity, 24), dp(activity, 24))

            // Gradient background
            val gradient = GradientDrawable().apply {
                colors = intArrayOf(
                    Color.parseColor("#F8F9FA"),
                    Color.parseColor("#FFFFFF")
                )
                gradientType = GradientDrawable.LINEAR_GRADIENT
                orientation = GradientDrawable.Orientation.TOP_BOTTOM
                cornerRadius = dp(activity, 16).toFloat()
            }
            background = gradient

            layoutParams = ViewGroup.LayoutParams(
                dp(activity, 320),
                ViewGroup.LayoutParams.WRAP_CONTENT
            )
        }

        // Header with title and close button
        val header = LinearLayout(activity).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            ).apply {
                bottomMargin = dp(activity, 16)
            }
        }

        // Title and version
        val titleContainer = LinearLayout(activity).apply {
            orientation = LinearLayout.VERTICAL
            layoutParams = LinearLayout.LayoutParams(
                0,
                LinearLayout.LayoutParams.WRAP_CONTENT,
                1f
            )
        }

        val titleView = TextView(activity).apply {
            text = activity.getString(R.string.lx_update_title)
            setTextColor(Color.parseColor("#1F2937"))
            textSize = 20f
            setTypeface(null, android.graphics.Typeface.BOLD)
            setCompoundDrawablesWithIntrinsicBounds(0, 0, 0, 0)
            compoundDrawablePadding = 0
        }

        val versionView = TextView(activity).apply {
            text = updateInfo?.let { "v${it.version}" } ?: ""
            setTextColor(Color.parseColor("#6B7280"))
            textSize = 14f
            visibility = if (updateInfo != null) View.VISIBLE else View.GONE
        }

        titleContainer.addView(titleView)
        titleContainer.addView(versionView)

        // Close button (X)
        val closeButton = TextView(activity).apply {
            text = "✕"
            setTextColor(Color.parseColor("#9CA3AF"))
            textSize = 24f
            gravity = Gravity.CENTER
            layoutParams = LinearLayout.LayoutParams(
                dp(activity, 36),
                dp(activity, 36)
            )
            setOnClickListener {
                dialog.dismiss()
                NativeApi.onCallback(callbackId, false, "2000")
            }
        }

        header.addView(titleContainer)
        header.addView(closeButton)
        container.addView(header)

        // Release notes (if available)
        if (updateInfo?.releaseNotes != null && updateInfo.releaseNotes.isNotEmpty()) {
            val notesScroll = ScrollView(activity).apply {
                layoutParams = LinearLayout.LayoutParams(
                    LinearLayout.LayoutParams.MATCH_PARENT,
                    dp(activity, 120)
                ).apply {
                    bottomMargin = dp(activity, 16)
                }
            }

            val notesContainer = LinearLayout(activity).apply {
                orientation = LinearLayout.VERTICAL
                setPadding(dp(activity, 12), dp(activity, 12), dp(activity, 12), dp(activity, 12))

                val bg = GradientDrawable().apply {
                    setColor(Color.parseColor("#F3F4F6"))
                    cornerRadius = dp(activity, 8).toFloat()
                }
                background = bg
            }

            updateInfo.releaseNotes.forEach { note ->
                val noteView = TextView(activity).apply {
                    text = "• $note"
                    setTextColor(Color.parseColor("#4B5563"))
                    textSize = 14f
                    layoutParams = LinearLayout.LayoutParams(
                        LinearLayout.LayoutParams.MATCH_PARENT,
                        LinearLayout.LayoutParams.WRAP_CONTENT
                    ).apply {
                        bottomMargin = dp(activity, 4)
                    }
                }
                notesContainer.addView(noteView)
            }

            notesScroll.addView(notesContainer)
            container.addView(notesScroll)
        } else {
            // Message when no release notes
            val messageView = TextView(activity).apply {
                text = activity.getString(R.string.lx_update_message)
                setTextColor(Color.parseColor("#6B7280"))
                textSize = 14f
                layoutParams = LinearLayout.LayoutParams(
                    LinearLayout.LayoutParams.MATCH_PARENT,
                    LinearLayout.LayoutParams.WRAP_CONTENT
                ).apply {
                    bottomMargin = dp(activity, 16)
                }
            }
            container.addView(messageView)
        }

        // Size info (if available)
        if (updateInfo != null && updateInfo.sizeBytes > 0) {
            val sizeView = TextView(activity).apply {
                val sizeMB = updateInfo.sizeBytes / (1024.0 * 1024.0)
                text = String.format("%.1f MB", sizeMB)
                setTextColor(Color.parseColor("#9CA3AF"))
                textSize = 13f
                gravity = Gravity.END
                layoutParams = LinearLayout.LayoutParams(
                    LinearLayout.LayoutParams.MATCH_PARENT,
                    LinearLayout.LayoutParams.WRAP_CONTENT
                ).apply {
                    bottomMargin = dp(activity, 16)
                }
            }
            container.addView(sizeView)
        }

        // Confirm button
        val confirmButton = Button(activity).apply {
            text = activity.getString(R.string.lx_update_confirm)
            setTextColor(Color.WHITE)
            textSize = 16f
            setTypeface(null, android.graphics.Typeface.BOLD)
            isAllCaps = false

            val buttonBg = GradientDrawable().apply {
                setColor(Color.parseColor("#3B82F6"))
                cornerRadius = dp(activity, 12).toFloat()
            }
            background = buttonBg

            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                dp(activity, 52)
            )

            setOnClickListener {
                showDownloadProgress()
                dialog.dismiss()
                val result = JSONObject().apply {
                    put("confirm", true)
                    put("cancel", false)
                }
                NativeApi.onCallback(callbackId, true, result.toString())
            }
        }

        container.addView(confirmButton)
        dialog.setContentView(container)

        return dialog
    }

    /**
     * Install an application update from the given APK file path.
     * Opens the system installer to prompt the user to install the APK.
     *
     * @param apkPath Absolute file path to the downloaded APK file
     * @return true if the installer was launched, false otherwise
     */
    @JvmStatic
    fun installUpdate(apkPath: String): Boolean {
        val activity = resolveActivity()
        if (activity == null) {
            Log.w(TAG, "No current activity; deferring update install")
            pendingInstallPath = apkPath
            return true
        }

        return if (android.os.Looper.myLooper() == android.os.Looper.getMainLooper()) {
            launchInstaller(activity, apkPath)
        } else {
            val latch = CountDownLatch(1)
            val result = BooleanArray(1)
            activity.runOnUiThread {
                result[0] = launchInstaller(activity, apkPath)
                latch.countDown()
            }
            latch.await()
            result[0]
        }
    }

    private fun launchInstaller(activity: LxAppActivity, apkPath: String): Boolean {
        try {
            val apkFile = File(apkPath)
            if (!apkFile.exists()) {
                Log.e(TAG, "APK file does not exist: $apkPath")
                return false
            }

            val pkgInfo = activity.packageManager.getPackageArchiveInfo(apkPath, 0)
            if (pkgInfo == null) {
                Log.e(TAG, "Invalid APK (cannot parse package): $apkPath")
                return false
            }
            if (pkgInfo.packageName != activity.packageName) {
                Log.e(
                    TAG,
                    "APK package mismatch: ${pkgInfo.packageName} vs ${activity.packageName}"
                )
                return false
            }

            if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.O) {
                if (!activity.packageManager.canRequestPackageInstalls()) {
                    Log.w(TAG, "Install permission not granted; opening settings")
                    pendingInstallPath = apkPath
                    val intent = Intent(
                        Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES,
                        Uri.parse("package:${activity.packageName}")
                    )
                    activity.startActivity(intent)
                    return true
                }
            }

            val apkUri = if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.N) {
                // Use FileProvider for Android 7.0+
                androidx.core.content.FileProvider.getUriForFile(
                    activity,
                    "${activity.packageName}.fileprovider",
                    apkFile
                )
            } else {
                android.net.Uri.fromFile(apkFile)
            }

            val intent = Intent(Intent.ACTION_VIEW).apply {
                setDataAndType(apkUri, "application/vnd.android.package-archive")
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            }

            activity.startActivity(intent)
            return true
        } catch (e: Exception) {
            Log.e(TAG, "Failed to install update from: $apkPath", e)
            return false
        }
    }
}
