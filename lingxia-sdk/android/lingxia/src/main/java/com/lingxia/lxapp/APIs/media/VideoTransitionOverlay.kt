package com.lingxia.lxapp.APIs.media

import android.graphics.Bitmap
import android.graphics.Color
import android.view.View
import android.view.ViewGroup
import android.widget.FrameLayout
import android.widget.ImageView

/**
 * Thin ImageView placed above the player surface to bridge visual gaps during
 * media-item transitions.
 *
 * Used by [LxMediaPlayer] to show a still bitmap (e.g. the next playlist item's
 * pre-extracted first frame) during the brief window between
 * `onMediaItemTransition` and `onRenderedFirstFrame` of the new item.
 *
 * Design notes:
 * - Owns the [ImageView] attached to a host [ViewGroup]; never holds the parent.
 *   [attach] is idempotent; [detach] cleanly removes the view.
 * - Does not own bitmaps — [LocalVideoFrameCache] does. [hide] just clears the
 *   ImageView's drawable reference; the bitmap stays in cache.
 * - Mirrors [ImageView.scaleType], corner radius, and rotation to ExoPlayer's
 *   active gravity so the overlay's pixels align with the live video pixels;
 *   any mismatch would produce a visible jump on transition.
 * - All operations are main-thread (ImageView contract); callers from worker
 *   threads must marshal.
 */
internal class VideoTransitionOverlay(private val host: ViewGroup) {

    private var imageView: ImageView? = null
    private var lastShown: Bitmap? = null
    private var pendingObjectFit: LxMediaObjectFit? = null
    private var pendingCornerRadiusPx: Float = 0f
    private var pendingRotation: Int = 0
    private var pendingScaleX: Float = 1f
    private var pendingScaleY: Float = 1f

    /**
     * Mount the overlay above the host's existing children. Idempotent:
     * a second call with the same host is a no-op.
     */
    fun attach() {
        if (imageView != null) return
        val view = ImageView(host.context).apply {
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
            scaleType = pendingObjectFit?.toScaleType() ?: ImageView.ScaleType.CENTER_CROP
            visibility = View.GONE
            // Transparent background so a hidden overlay never tints the video.
            setBackgroundColor(Color.TRANSPARENT)
            // Matches ExoPlayer SurfaceView's default behavior.
            setLayerType(View.LAYER_TYPE_HARDWARE, null)
            rotation = pendingRotation.toFloat()
            scaleX = pendingScaleX
            scaleY = pendingScaleY
            // Corner radius is applied by the parent (host already clips).
        }
        host.addView(view)
        imageView = view
    }

    /**
     * Show [bitmap]. No-op if the same bitmap is already visible (avoids a
     * needless `setImageBitmap` which triggers invalidate + measure).
     * Caller-owned bitmap; we never recycle it.
     */
    fun show(bitmap: Bitmap) {
        val view = imageView ?: return
        if (lastShown === bitmap && view.visibility == View.VISIBLE) return
        view.setImageBitmap(bitmap)
        view.visibility = View.VISIBLE
        lastShown = bitmap
    }

    /** Hide and release our reference to the bitmap. The cache still owns it. */
    fun hide() {
        val view = imageView ?: return
        if (view.visibility == View.GONE && lastShown == null) return
        view.visibility = View.GONE
        view.setImageBitmap(null)
        lastShown = null
    }

    /** Mirror player's objectFit so overlay pixel layout matches the video. */
    fun setObjectFit(fit: LxMediaObjectFit) {
        pendingObjectFit = fit
        imageView?.scaleType = fit.toScaleType()
    }

    /** Mirror player's display rotation. */
    fun setRotation(degrees: Int) {
        pendingRotation = degrees
        imageView?.rotation = degrees.toFloat()
    }

    /**
     * Mirror the player's inline rotation transform (rotation + scale around
     * center). Mirrors [LxMediaPlayer.applyInlineDisplayRotationTransform] so
     * the overlay's pixel layout stays in sync with the rotated video.
     *
     * If called pre-attach, the latest values are stashed and replayed on the
     * ImageView at attach time; in practice the host always attaches before
     * applying transforms, so this is defensive only.
     */
    fun applyInlineTransform(degrees: Int, scaleX: Float, scaleY: Float) {
        pendingRotation = degrees
        pendingScaleX = scaleX
        pendingScaleY = scaleY
        val view = imageView ?: return
        view.pivotX = view.width / 2f
        view.pivotY = view.height / 2f
        view.rotation = degrees.toFloat()
        view.scaleX = scaleX
        view.scaleY = scaleY
    }

    /**
     * Mirror player's corner radius (px). Implemented via the host's outline
     * provider when the host clips to outline; this just records the value.
     * Kept on the overlay's API surface for future use if the host doesn't
     * already clip (e.g. fullscreen overlay).
     */
    fun setCornerRadiusPx(radius: Float) {
        pendingCornerRadiusPx = radius
        // Currently relies on host's clipToOutline (LxMediaPlayer's container
        // already does this). If a future host doesn't, apply via outline here.
    }

    /** Detach from the host. Idempotent. */
    fun detach() {
        val view = imageView ?: return
        view.setImageBitmap(null)
        if (view.parent === host) host.removeView(view)
        imageView = null
        lastShown = null
    }

    private fun LxMediaObjectFit.toScaleType(): ImageView.ScaleType = when (this) {
        LxMediaObjectFit.COVER -> ImageView.ScaleType.CENTER_CROP
        LxMediaObjectFit.CONTAIN -> ImageView.ScaleType.FIT_CENTER
        LxMediaObjectFit.FILL -> ImageView.ScaleType.FIT_XY
        LxMediaObjectFit.FIT -> ImageView.ScaleType.FIT_CENTER
    }
}
