package com.lingxia.lxapp.NativeComponents.Components

import android.graphics.RectF
import android.view.View
import android.view.ViewGroup
import android.widget.FrameLayout
import android.view.TextureView
import com.lingxia.lxapp.APIs.media.LxMediaCommand
import com.lingxia.lxapp.APIs.media.LxMediaEvent
import com.lingxia.lxapp.APIs.media.LxMediaObjectFit
import com.lingxia.lxapp.APIs.media.LxMediaPlayer
import com.lingxia.lxapp.APIs.media.LxMediaPlayerConfig
import com.lingxia.lxapp.APIs.media.LxMediaQuality
import com.lingxia.lxapp.APIs.media.LxMediaSource
import com.lingxia.lxapp.LxApp
import com.lingxia.lxapp.NativeComponents.LxNativeComponent
import com.lingxia.lxapp.NativeComponents.LxNativeComponentFactory

class VideoComponentFactory : LxNativeComponentFactory {
    override fun make(id: String, initialProps: Map<String, Any?>, eventSink: (Map<String, Any>) -> Unit) =
        VideoComponent(id, initialProps, eventSink)
}

class VideoComponent(
    override val id: String,
    private val initialProps: Map<String, Any?>,
    eventSink: (Map<String, Any>) -> Unit
) : LxNativeComponent {

    private var player: LxMediaPlayer? = null
    private var context: android.content.Context? = null
    private val eventSinkRef = eventSink
    private var lastFrame: RectF? = null

    override val view: View get() = player?.view ?: FrameLayout(context!!)

    override fun mount(host: ViewGroup) {
        context = LxApp.getCurrentActivity() ?: host.context
        player = LxMediaPlayer(context!!, eventSinkRef, typedEventSink = { event ->
            if (event is LxMediaEvent.FullscreenChange && !event.fullScreen) {
                lastFrame?.let { player?.setFrame(it.left, it.top, it.width(), it.height()) }
            }
        }, componentId = id)
        player?.update(makeConfig(initialProps))
        host.addView(player!!.view)
    }

    override fun update(props: Map<String, Any?>) {
        player?.update(makeConfig(props))
    }

    override fun setFrame(frame: RectF) {
        lastFrame = RectF(frame)
        if (player?.isFullscreen() != true) {
            player?.setFrame(frame.left, frame.top, frame.width(), frame.height())
        }
    }

    override fun focus() {
        lastFrame?.let { player?.setFrame(it.left, it.top, it.width(), it.height()) }
        player?.view?.requestLayout()
    }

    override fun blur() {}

    override fun handleCommand(name: String, params: Map<String, Any?>?) {
        val command = makeCommand(name, params) ?: return
        player?.handle(command)
    }

    internal fun handleStreamDecoderEvent(event: String, detail: Map<String, Any?>) {
        player?.handleStreamDecoderEvent(event, detail)
    }

    fun acquireStreamTextureView(): TextureView? {
        return player?.acquireStreamTextureView()
    }

    fun releaseStreamTextureView() {
        player?.releaseStreamTextureView()
    }

    override fun unmount() {
        player?.pause()
        player?.exitFullscreen()
        player?.detach()
        player = null
    }

    companion object {
        fun makeConfig(props: Map<String, Any?>): LxMediaPlayerConfig {
            val config = LxMediaPlayerConfig()

            // Parse source
            (props["source"] as? Map<*, *>)?.let { source ->
                val type = source["type"] as? String
                val value = source["value"] as? String
                if (type != null && value != null) {
                    config.source = when (type) {
                        "url" -> LxMediaSource.Url(value)
                        "file" -> LxMediaSource.FilePath(value)
                        else -> null
                    }
                }
            }

            // Fallback to src if no source
            if (config.source == null) {
                (props["src"] as? String)?.let { config.src = it }
            }

            (props["poster"] as? String)?.let { config.poster = it }
            (props["autoplay"] as? Boolean)?.let { config.autoplay = it }
            (props["loop"] as? Boolean)?.let { config.loop = it }
            (props["muted"] as? Boolean)?.let { config.muted = it }
            (props["volume"] as? Number)?.let { config.volume = it.toDouble() }
            (props["controls"] as? Boolean)?.let { config.controls = it }
            (props["progressBar"] as? Boolean)?.let { config.progressBar = it }
            (props["cornerRadius"] as? Number)?.let { config.cornerRadius = it.toDouble() }

            config.qualities = parseQualities(props["qualities"])
            config.speeds = parseRates(props["playbackRates"])

            (props["showControlsOnInit"] as? Boolean)?.let { config.showControlsOnInit = it }
            (props["objectFit"] as? String)?.let { config.objectFit = LxMediaObjectFit.fromString(it) }
            // Rotate video content (0/90/180/270).
            (props["rotate"] as? Number)?.let { config.rotateDegrees = it.toInt() }

            return config
        }

        private fun parseQualities(value: Any?): List<LxMediaQuality>? {
            val list = (value as? List<*>)?.mapNotNull(::parseQualityEntry).orEmpty()
            return list.takeIf { it.isNotEmpty() }
        }

        private fun parseQualityEntry(entry: Any?): LxMediaQuality? = when (entry) {
            is Map<*, *> -> {
                val label = entry["label"] as? String ?: return null
                val url = entry["url"] as? String
                LxMediaQuality(label, url)
            }
            else -> null
        }

        private fun parseRates(value: Any?): List<Double>? {
            val list = (value as? List<*>)?.mapNotNull { (it as? Number)?.toDouble() }.orEmpty()
            return list.takeIf { it.isNotEmpty() }
        }

        fun makeCommand(name: String, params: Map<String, Any?>?): LxMediaCommand? {
            return when (name) {
                "play" -> LxMediaCommand.Play
                "pause" -> LxMediaCommand.Pause
                "stop" -> LxMediaCommand.Stop
                "notifyEnded" -> LxMediaCommand.NotifyEnded
                "seek" -> {
                    val time = (params?.get("time") as? Number)?.toDouble() ?: return null
                    LxMediaCommand.Seek(time)
                }
                "setDuration" -> {
                    val duration = (params?.get("duration") as? Number)?.toDouble() ?: return null
                    LxMediaCommand.SetDuration(duration)
                }
                "setVolume" -> {
                    val volume = (params?.get("volume") as? Number)?.toDouble() ?: return null
                    LxMediaCommand.SetVolume(volume)
                }
                "setMuted" -> {
                    val muted = params?.get("muted") as? Boolean ?: return null
                    LxMediaCommand.SetMuted(muted)
                }
                "setPlaybackRate" -> {
                    val rate = (params?.get("rate") as? Number)?.toDouble() ?: return null
                    LxMediaCommand.SetPlaybackRate(rate)
                }
                "enterFullscreen" -> LxMediaCommand.EnterFullscreen
                "exitFullscreen" -> LxMediaCommand.ExitFullscreen
                else -> null
            }
        }
    }
}
