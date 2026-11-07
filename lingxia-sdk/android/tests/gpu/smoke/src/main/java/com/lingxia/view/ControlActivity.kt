package com.lingxia.view

import android.app.Activity
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.drawable.GradientDrawable
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import android.util.Log
import android.view.FrameMetrics
import android.view.Gravity
import android.view.InputDevice
import android.view.MotionEvent
import android.view.View
import android.view.ViewGroup
import android.view.Window
import android.view.animation.LinearInterpolator
import android.view.animation.OvershootInterpolator
import android.widget.FrameLayout
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.TextView
import androidx.recyclerview.widget.LinearLayoutManager
import androidx.recyclerview.widget.RecyclerView
import kotlin.math.roundToInt

/**
 * Plain Android controls for the GPU perf-lab scenes, judged by the same bar (perf-judge.ts):
 * frame time is the gap between presented frames, a refresh with no new frame while the scene
 * changes counts as dropped, input runs from a MotionEvent's time to the commit of the first
 * frame drawn after it (the View's input metric ends later, at the refresh the frame is shown on). `adb shell am start -n com.lingxia.view.smoke/.ControlActivity --es scene
 * list|data|anim|text|touch`; the verdict is one `PerfControl` log line.
 */
class ControlActivity : Activity() {
    private val main = Handler(Looper.getMainLooper())
    private val density get() = resources.displayMetrics.density
    private fun dp(value: Number) = (value.toFloat() * density).roundToInt()

    private val vsyncs = ArrayList<Long>()
    private val totals = ArrayList<Long>()
    private val inputs = ArrayList<Long>()
    private val pending = ArrayDeque<Long>()
    private var measuring = false
    private var periodNs = 16_666_667L

    override fun onCreate(state: Bundle?) {
        super.onCreate(state)
        val scene = intent.getStringExtra("scene") ?: "list"
        periodNs = (1e9 / (display?.refreshRate ?: 60f)).toLong()
        val root = FrameLayout(this).apply { setBackgroundColor(Color.rgb(246, 247, 251)) }
        setContentView(root)
        val run: () -> Unit = when (scene) {
            "list" -> list(root, data = false)
            "data" -> list(root, data = true)
            "anim" -> springs(root)
            "text" -> relayout(root)
            "touch" -> drag(root)
            else -> error("unknown scene $scene")
        }
        window.addOnFrameMetricsAvailableListener(::frame, main)
        // Settle first, then drive and measure for 5 s, as the perf-lab runs do.
        main.postDelayed({
            measuring = true
            run()
            main.postDelayed({ report(scene) }, RUN_MS)
        }, 1_000)
    }

    private fun frame(@Suppress("UNUSED_PARAMETER") window: Window, metrics: FrameMetrics, @Suppress("UNUSED_PARAMETER") dropped: Int) {
        if (!measuring) return
        val vsync = metrics.getMetric(FrameMetrics.INTENDED_VSYNC_TIMESTAMP)
        val total = metrics.getMetric(FrameMetrics.TOTAL_DURATION)
        vsyncs += vsync
        totals += total
        // Events handled for this frame are those that happened before its vsync.
        while (pending.isNotEmpty() && pending.first() <= vsync) inputs += vsync + total - pending.removeFirst()
    }

    private fun report(scene: String) {
        measuring = false
        // Longer than four refreshes is the scene resting (an animation between flips), which
        // the View's demand-gated statistics do not count either.
        val gaps = vsyncs.zipWithNext { a, b -> b - a }.filter { it > 0 && it <= periodNs * 4 }
        val dropped = gaps.sumOf { maxOf(0, (it.toDouble() / periodNs).roundToInt() - 1) }
        val shown = gaps.size + dropped
        val frameP95 = pct(gaps, 0.95)
        val inputP95 = pct(inputs, 0.95)
        val period = periodNs / 1e6
        val framesPass = frameP95 != null && frameP95 <= period * 2.5 && dropped <= shown * 0.1
        val pass = if (scene == "touch") inputP95 != null && inputP95 <= period * 5 else framesPass
        Log.i("PerfControl", "scene=$scene pass=$pass frames p50/p95 ${ms(pct(gaps, 0.5))}/${ms(frameP95)} dropped $dropped/$shown " +
            "totalDuration p95 ${ms(pct(totals, 0.95))} input p50/p95 ${ms(pct(inputs, 0.5))}/${ms(inputP95)} (${inputs.size})")
        finish()
    }

    private fun pct(values: List<Long>, q: Double): Double? =
        values.sorted().takeIf { it.isNotEmpty() }?.let { it[((it.size - 1) * q).toInt()] / 1e6 }

    private fun ms(value: Double?) = value?.let { "%.1f".format(it) } ?: "-"

    // LIST glides 90 rows over the run; DATA moves the anchor two rows per 16 ms tick.
    private fun list(root: FrameLayout, data: Boolean): () -> Unit {
        val images = (0 until 16).map { i ->
            Bitmap.createBitmap(dp(40), dp(40), Bitmap.Config.ARGB_8888).also {
                Canvas(it).drawCircle(dp(20).toFloat(), dp(20).toFloat(), dp(20).toFloat(),
                    Paint(Paint.ANTI_ALIAS_FLAG).apply { color = Color.HSVToColor(floatArrayOf(i * 22.5f, 0.5f, 0.9f)) })
            }
        }
        val list = RecyclerView(this).apply {
            layoutManager = LinearLayoutManager(this@ControlActivity)
            adapter = Rows(images)
        }
        root.addView(list, FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, dp(384), Gravity.CENTER))
        return {
            if (data) {
                var tick = 0
                val step = object : Runnable {
                    override fun run() {
                        if (!measuring) return
                        tick++
                        (list.layoutManager as LinearLayoutManager).scrollToPositionWithOffset((tick * 2) % 1000, 0)
                        main.postDelayed(this, 16)
                    }
                }
                main.post(step)
            } else {
                val rowHeight = list.getChildAt(0)?.height ?: dp(56)
                list.smoothScrollBy(0, rowHeight * 90, LinearInterpolator(), RUN_MS.toInt())
            }
        }
    }

    private inner class Rows(val images: List<Bitmap>) : RecyclerView.Adapter<RecyclerView.ViewHolder>() {
        override fun getItemCount() = 1000
        override fun onCreateViewHolder(parent: ViewGroup, type: Int): RecyclerView.ViewHolder {
            val row = LinearLayout(parent.context).apply {
                orientation = LinearLayout.HORIZONTAL
                gravity = Gravity.CENTER_VERTICAL
                setPadding(dp(12), dp(8), dp(12), dp(8))
            }
            row.addView(ImageView(parent.context), LinearLayout.LayoutParams(dp(40), dp(40)))
            val texts = LinearLayout(parent.context).apply { orientation = LinearLayout.VERTICAL; setPadding(dp(12), 0, dp(12), 0) }
            texts.addView(TextView(parent.context).apply { textSize = 14f; maxLines = 1 })
            texts.addView(TextView(parent.context).apply { textSize = 12f; maxLines = 1; setTextColor(Color.GRAY) })
            row.addView(texts, LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f))
            row.addView(TextView(parent.context).apply {
                textSize = 12f; setTextColor(Color.WHITE); setPadding(dp(8), 0, dp(8), 0)
                background = GradientDrawable().apply { cornerRadius = dp(16).toFloat(); setColor(Color.rgb(0, 150, 80)) }
            })
            row.layoutParams = RecyclerView.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT)
            return object : RecyclerView.ViewHolder(row) {}
        }
        override fun onBindViewHolder(holder: RecyclerView.ViewHolder, position: Int) {
            val row = holder.itemView as LinearLayout
            (row.getChildAt(0) as ImageView).setImageBitmap(images[position % 16])
            val texts = row.getChildAt(1) as LinearLayout
            (texts.getChildAt(0) as TextView).text = "Row ${position + 1}"
            (texts.getChildAt(1) as TextView).text = "A second line of text to lay out in every row"
            (row.getChildAt(2) as TextView).text = (position % 100).toString()
        }
    }

    // ANIM: 250 dots spring to the other side together every 600 ms.
    private fun springs(root: FrameLayout): () -> Unit {
        val box = FrameLayout(this)
        root.addView(box, FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, dp(320), Gravity.CENTER))
        val size = dp(12); val gap = dp(4); val columns = (resources.displayMetrics.widthPixels - gap) / (size + gap)
        val dots = (0 until 250).map { i ->
            View(this).apply {
                background = GradientDrawable().apply { shape = GradientDrawable.OVAL; setColor(Color.rgb(0, 150, 80)) }
                box.addView(this, FrameLayout.LayoutParams(size, size).apply {
                    leftMargin = gap + (i % columns) * (size + gap); topMargin = gap + (i / columns) * (size + gap)
                })
            }
        }
        return {
            var phase = false
            val flip = object : Runnable {
                override fun run() {
                    if (!measuring) return
                    phase = !phase
                    for (dot in dots) dot.animate().translationY(if (phase) dp(32).toFloat() else 0f)
                        .scaleX(if (phase) 1.5f else 1f).scaleY(if (phase) 1.5f else 1f)
                        .setDuration(500).setInterpolator(OvershootInterpolator(2f)).start()
                    main.postDelayed(this, 600)
                }
            }
            main.post(flip)
        }
    }

    // TEXT: a mixed-script paragraph x4 whose box width and typed suffix change every tick.
    private fun relayout(root: FrameLayout): () -> Unit {
        val text = TextView(this).apply { textSize = 14f }
        val column = FrameLayout(this)
        column.addView(text, FrameLayout.LayoutParams(dp(200), ViewGroup.LayoutParams.WRAP_CONTENT))
        root.addView(column, FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, dp(320), Gravity.CENTER))
        return {
            var tick = 0
            val step = object : Runnable {
                override fun run() {
                    if (!measuring) return
                    tick++
                    text.text = PARAGRAPH.repeat(4) + "typing… 輸入中 ⌨️ ".repeat((tick / 6) % 20)
                    text.layoutParams = text.layoutParams.apply { width = dp(200 + (tick * 3) % 160) }
                    main.postDelayed(this, 16)
                }
            }
            main.post(step)
        }
    }

    // TOUCH: a 96 px box follows a drag across and back, events delivered at their planned times.
    private fun drag(root: FrameLayout): () -> Unit {
        val box = View(this).apply { setBackgroundColor(Color.rgb(0, 150, 80)) }
        root.addView(box, FrameLayout.LayoutParams(96, 96, Gravity.CENTER_VERTICAL))
        var grabbed = 0f
        box.setOnTouchListener { view, event ->
            when (event.actionMasked) {
                MotionEvent.ACTION_DOWN -> grabbed = event.rawX - view.translationX
                MotionEvent.ACTION_MOVE -> view.translationX = event.rawX - grabbed
            }
            true
        }
        return {
            val start = SystemClock.uptimeMillis() + 50
            val down = start
            val (x0, y) = IntArray(2).also { box.getLocationOnScreen(it) }.let { it[0] + 48f to it[1] + 48f }
            val distance = dp(220).toFloat()
            var at = 0L
            fun send(action: Int, x: Float, time: Long) = main.postAtTime({
                val event = MotionEvent.obtain(down, time, action, x, y, 0).apply { source = InputDevice.SOURCE_TOUCHSCREEN }
                val origin = IntArray(2).also { window.decorView.getLocationOnScreen(it) }
                event.offsetLocation(-origin[0].toFloat(), -origin[1].toFloat())
                pending.addLast(time * 1_000_000)
                window.decorView.dispatchTouchEvent(event)
                event.recycle()
            }, time)
            send(MotionEvent.ACTION_DOWN, x0, start)
            // Across in 900 ms and back, 8 ms apart, as the spec's two drags do.
            while (at <= 1_800) {
                val travel = if (at <= 900) at / 900f else (1_800 - at) / 900f
                send(MotionEvent.ACTION_MOVE, x0 + distance * travel, start + at)
                at += 8
            }
            send(MotionEvent.ACTION_UP, x0, start + 1_800)
        }
    }

    companion object {
        const val RUN_MS = 5_000L
        const val PARAGRAPH = "LingXia 灵匣 lays out mixed scripts: 文字排版需要处理中文、English words, emoji 🚀✨🎉 and numbers 12345 together. "
    }
}
