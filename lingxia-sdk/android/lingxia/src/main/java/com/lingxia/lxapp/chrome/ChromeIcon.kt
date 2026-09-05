package com.lingxia.lxapp.chrome

import android.graphics.Canvas
import android.graphics.Bitmap
import android.graphics.BlendMode
import android.graphics.BlendModeColorFilter
import android.graphics.ColorFilter
import android.graphics.Outline
import android.graphics.Paint
import android.graphics.PixelFormat
import android.graphics.PorterDuff
import android.graphics.PorterDuffColorFilter
import android.graphics.Rect
import android.graphics.RectF
import android.graphics.drawable.Drawable
import android.view.View
import android.view.ViewOutlineProvider
import android.widget.ImageView
import com.caverock.androidsvg.SVG
import java.io.File

/** Template glyph for native chrome. Raster formats go through the platform decoder; SVG is a path. */
internal object ChromeIcon {
    fun isTemplateGlyph(path: String): Boolean {
        return path.isBlank() ||
            path.endsWith(".svg", ignoreCase = true) ||
            path.startsWith("SF:", ignoreCase = true)
    }

    fun load(path: String, tint: Int, fallback: () -> Drawable): Drawable {
        val file = File(path)
        if (!file.exists()) return fallback()
        val loaded =
            if (path.endsWith(".svg", ignoreCase = true)) {
                loadSvg(file, tint)
            } else {
                Drawable.createFromPath(file.absolutePath)
            }
        val drawable = (loaded ?: fallback()).mutate()
        if (isTemplateGlyph(path) && loaded !is SvgTemplateDrawable) {
            drawable.setTint(tint)
        }
        return drawable
    }

    fun applyTo(imageView: ImageView, path: String) {
        if (isTemplateGlyph(path)) {
            imageView.scaleType = ImageView.ScaleType.FIT_CENTER
            imageView.clipToOutline = false
            imageView.outlineProvider = null
            return
        }
        imageView.scaleType = ImageView.ScaleType.CENTER_CROP
        imageView.clipToOutline = true
        imageView.outlineProvider =
            object : ViewOutlineProvider() {
                override fun getOutline(view: View, outline: Outline) {
                    val radius = 4f * view.resources.displayMetrics.density
                    outline.setRoundRect(0, 0, view.width, view.height, radius)
                }
            }
    }

    private fun loadSvg(file: File, tint: Int): Drawable? {
        val svg = runCatching { file.inputStream().use(SVG::getFromInputStream) }.getOrNull()
            ?: return null
        return SvgTemplateDrawable(svg, tint)
    }
}

/** Rasterize the complete SVG, then use its alpha as the host-tinted template mask. */
private class SvgTemplateDrawable(
    private val svg: SVG,
    private var tint: Int,
) : Drawable() {
    private val paint = Paint(Paint.ANTI_ALIAS_FLAG or Paint.FILTER_BITMAP_FLAG)
    private var rendered: Bitmap? = null

    override fun onBoundsChange(bounds: Rect) {
        rendered = null
    }

    override fun draw(canvas: Canvas) {
        if (bounds.isEmpty) return
        val bitmap = rendered ?: Bitmap.createBitmap(
            bounds.width(),
            bounds.height(),
            Bitmap.Config.ARGB_8888,
        ).also { target ->
            svg.renderToCanvas(
                Canvas(target),
                RectF(0f, 0f, target.width.toFloat(), target.height.toFloat()),
            )
            rendered = target
        }
        canvas.drawBitmap(bitmap, null, bounds, paint)
    }

    override fun setAlpha(alpha: Int) {
        paint.alpha = alpha
        invalidateSelf()
    }

    override fun setColorFilter(colorFilter: ColorFilter?) {
        paint.colorFilter = colorFilter
        invalidateSelf()
    }

    override fun setTint(tintColor: Int) {
        tint = tintColor
        paint.colorFilter = if (android.os.Build.VERSION.SDK_INT >= 29) {
            BlendModeColorFilter(tint, BlendMode.SRC_IN)
        } else {
            @Suppress("DEPRECATION")
            PorterDuffColorFilter(tint, PorterDuff.Mode.SRC_IN)
        }
        invalidateSelf()
    }

    @Deprecated("Drawable.getOpacity is deprecated but still abstract")
    override fun getOpacity(): Int = PixelFormat.TRANSLUCENT

    init {
        setTint(tint)
    }
}
