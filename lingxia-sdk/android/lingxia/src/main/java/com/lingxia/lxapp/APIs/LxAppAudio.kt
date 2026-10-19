package com.lingxia.lxapp.APIs

import android.annotation.SuppressLint
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.content.pm.PackageManager
import android.hardware.Sensor
import android.hardware.SensorEvent
import android.hardware.SensorEventListener
import android.hardware.SensorManager
import android.media.AudioAttributes
import android.media.AudioDeviceCallback
import android.media.AudioDeviceInfo
import android.media.AudioFocusRequest
import android.media.AudioManager
import android.media.MediaMetadataRetriever
import android.media.SoundPool
import android.net.Uri
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.os.PowerManager
import androidx.annotation.OptIn
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.PlaybackException
import androidx.media3.common.PlaybackParameters
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.DefaultDataSource
import androidx.media3.datasource.DefaultHttpDataSource
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import com.lingxia.app.Lingxia
import com.lingxia.app.LxLog
import com.lingxia.app.NativeApi
import com.lingxia.lxapp.NativeComponents.ComponentRouter
import org.json.JSONObject
import java.io.File

/**
 * Native backend of the host audio coordinator. Rust decides focus, queue,
 * and state; this object applies commands and reports what the platform did.
 * Everything runs on the main looper, in the order commands were sent.
 */
internal object LxAppAudio {
    private const val TAG = "LingXia.Audio"
    private const val CAP_EARPIECE = 1
    private const val CAP_PROXIMITY = 2

    private val main = Handler(Looper.getMainLooper())
    private var callbackId = 0L
    private val players = HashMap<Long, AudioPlayer>()
    private val sounds = HashMap<Long, SoundBank>()
    private val session = Session()

    @JvmStatic
    fun capabilities(): Int {
        val context = Lingxia.applicationContext() ?: return 0
        var bits = 0
        if (hasEarpiece(context)) bits = bits or CAP_EARPIECE
        val sensors = context.getSystemService(Context.SENSOR_SERVICE) as? SensorManager
        if (sensors?.getDefaultSensor(Sensor.TYPE_PROXIMITY) != null) bits = bits or CAP_PROXIMITY
        return bits
    }

    @JvmStatic
    fun attach(id: Long) {
        main.post {
            callbackId = id
            session.observeRoutes()
        }
    }

    @JvmStatic
    fun send(json: String) {
        main.post {
            try {
                handle(JSONObject(json))
            } catch (e: Exception) {
                LxLog.w(TAG, "audio command failed: $json", e)
            }
        }
    }

    /** A native video started or stopped producing audible output. */
    fun notifyVideoAudible(componentId: String, audible: Boolean) {
        val run = {
            emit(JSONObject().put("event", "video").put("componentId", componentId).put("playing", audible))
        }
        if (Looper.myLooper() == Looper.getMainLooper()) run() else main.post(run)
    }

    private fun emit(event: JSONObject) {
        val id = callbackId
        if (id == 0L) return
        NativeApi.onCallback(id, true, event.toString())
    }

    private fun event(name: String): JSONObject = JSONObject().put("event", name)

    private fun context(): Context? = Lingxia.applicationContext()

    private fun handle(command: JSONObject) {
        val op = command.getString("op")
        val id = command.optLong("id")
        when (op) {
            "player.create" -> {
                val context = context() ?: return
                players[id]?.release()
                players[id] = AudioPlayer(context, id, command.optLong("tickMs", 250L))
            }
            "player.load" -> players[id]?.load(
                command.getLong("req"),
                command.getString("uri"),
                headers(command.optJSONObject("headers")),
                command.optLong("startMs", 0L),
            )
            "player.play" -> players[id]?.play()
            "player.pause" -> players[id]?.pause()
            "player.seek" -> players[id]?.seek(command.getLong("req"), command.getLong("positionMs"))
            "player.unload" -> players[id]?.unload()
            "player.volume" -> players[id]?.setVolume(command.getDouble("volume").toFloat())
            "player.rate" -> players[id]?.setRate(command.getDouble("rate").toFloat())
            "player.dispose" -> players.remove(id)?.release()
            "sound.load" -> {
                val context = context() ?: return
                sounds.remove(id)?.release()
                sounds[id] = SoundBank(context, id).also {
                    it.load(command.getLong("req"), command.getString("path"))
                }
            }
            "sound.play" -> sounds[id]?.play(
                command.getLong("voice"),
                command.optDouble("volume", 1.0).toFloat(),
                command.optDouble("rate", 1.0).toFloat(),
                command.optDouble("pan", 0.0).toFloat(),
                command.optBoolean("loop", false),
            )
            "sound.stop" -> sounds[id]?.stop(command.getLong("voice"))
            "sound.dispose" -> sounds.remove(id)?.release()
            "session" -> session.apply(
                active = command.optBoolean("active"),
                focus = if (command.isNull("focus")) null else command.optString("focus"),
                earpiece = command.optString("output") == "earpiece",
                proximity = command.optBoolean("proximity"),
            )
            "video.pause" -> ComponentRouter.dispatchVideoCommand(command.getString("componentId"), "pause", "{}")
            else -> LxLog.w(TAG, "unknown audio op $op")
        }
    }

    private fun headers(obj: JSONObject?): Map<String, String> {
        if (obj == null) return emptyMap()
        val out = HashMap<String, String>()
        obj.keys().forEach { key -> out[key] = obj.optString(key) }
        return out
    }

    private fun hasEarpiece(context: Context): Boolean {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
            val audio = context.getSystemService(Context.AUDIO_SERVICE) as? AudioManager ?: return false
            return audio.getDevices(AudioManager.GET_DEVICES_OUTPUTS)
                .any { it.type == AudioDeviceInfo.TYPE_BUILTIN_EARPIECE }
        }
        return context.packageManager.hasSystemFeature(PackageManager.FEATURE_TELEPHONY)
    }

    private fun mediaAttributes(earpiece: Boolean): androidx.media3.common.AudioAttributes =
        androidx.media3.common.AudioAttributes.Builder()
            .setUsage(if (earpiece) C.USAGE_VOICE_COMMUNICATION else C.USAGE_MEDIA)
            .setContentType(if (earpiece) C.AUDIO_CONTENT_TYPE_SPEECH else C.AUDIO_CONTENT_TYPE_MUSIC)
            .build()

    private fun errorCode(error: PlaybackException): String = when (error.errorCode) {
        PlaybackException.ERROR_CODE_IO_FILE_NOT_FOUND -> "source_not_found"
        PlaybackException.ERROR_CODE_IO_BAD_HTTP_STATUS ->
            if (error.cause?.message?.contains("404") == true) "source_not_found" else "network"
        PlaybackException.ERROR_CODE_IO_NETWORK_CONNECTION_FAILED,
        PlaybackException.ERROR_CODE_IO_NETWORK_CONNECTION_TIMEOUT,
        PlaybackException.ERROR_CODE_IO_INVALID_HTTP_CONTENT_TYPE,
        PlaybackException.ERROR_CODE_IO_CLEARTEXT_NOT_PERMITTED,
        PlaybackException.ERROR_CODE_IO_UNSPECIFIED,
        PlaybackException.ERROR_CODE_TIMEOUT -> "network"
        PlaybackException.ERROR_CODE_PARSING_CONTAINER_MALFORMED,
        PlaybackException.ERROR_CODE_PARSING_CONTAINER_UNSUPPORTED,
        PlaybackException.ERROR_CODE_PARSING_MANIFEST_MALFORMED,
        PlaybackException.ERROR_CODE_PARSING_MANIFEST_UNSUPPORTED,
        PlaybackException.ERROR_CODE_DECODING_FORMAT_UNSUPPORTED,
        PlaybackException.ERROR_CODE_DECODING_FORMAT_EXCEEDS_CAPABILITIES,
        PlaybackException.ERROR_CODE_AUDIO_TRACK_INIT_FAILED -> "unsupported_format"
        else -> "decode_failed"
    }

    @OptIn(UnstableApi::class)
    private class AudioPlayer(private val context: Context, private val id: Long, tickMs: Long) : Player.Listener {
        private val player = ExoPlayer.Builder(context).build()
        private val tickMs = tickMs.coerceAtLeast(50L)
        private var loadReq: Long? = null
        private var seekReq: Long? = null
        private var buffering = false
        private val ticker = object : Runnable {
            override fun run() {
                emit(
                    event("player.progress").put("id", id)
                        .put("positionMs", player.currentPosition.coerceAtLeast(0))
                        .put("bufferedMs", player.bufferedPosition.coerceAtLeast(0))
                        .also { duration()?.let { d -> it.put("durationMs", d) } }
                )
                main.postDelayed(this, tickMs)
            }
        }

        init {
            player.addListener(this)
            player.setAudioAttributes(mediaAttributes(session.earpiece), false)
            player.setHandleAudioBecomingNoisy(false)
            player.playWhenReady = false
        }

        fun applyRoute(earpiece: Boolean) {
            player.setAudioAttributes(mediaAttributes(earpiece), false)
        }

        private fun duration(): Long? = player.duration.takeIf { it != C.TIME_UNSET && it > 0 }

        fun load(req: Long, uri: String, headers: Map<String, String>, startMs: Long) {
            stopTicker()
            player.playWhenReady = false
            player.stop()
            player.clearMediaItems()
            loadReq = req
            seekReq = null
            buffering = false
            val parsed = if (uri.startsWith("/")) Uri.fromFile(File(uri)) else Uri.parse(uri)
            val item = MediaItem.fromUri(parsed)
            val scheme = parsed.scheme?.lowercase()
            if (scheme == "http" || scheme == "https") {
                val http = DefaultHttpDataSource.Factory()
                    .setAllowCrossProtocolRedirects(true)
                    .setDefaultRequestProperties(headers)
                val factory = DefaultMediaSourceFactory(DefaultDataSource.Factory(context, http))
                player.setMediaSource(factory.createMediaSource(item), startMs)
            } else {
                player.setMediaItem(item, startMs)
            }
            player.prepare()
        }

        fun play() {
            player.playWhenReady = true
        }

        fun pause() {
            player.playWhenReady = false
        }

        fun seek(req: Long, positionMs: Long) {
            seekReq = req
            player.seekTo(positionMs)
        }

        fun unload() {
            stopTicker()
            loadReq = null
            seekReq = null
            player.playWhenReady = false
            player.stop()
            player.clearMediaItems()
        }

        fun setVolume(volume: Float) {
            player.volume = volume.coerceIn(0f, 1f)
        }

        fun setRate(rate: Float) {
            player.playbackParameters = PlaybackParameters(rate)
        }

        fun release() {
            stopTicker()
            player.removeListener(this)
            player.release()
        }

        private fun stopTicker() = main.removeCallbacks(ticker)

        override fun onPlaybackStateChanged(state: Int) {
            when (state) {
                Player.STATE_READY -> {
                    val req = loadReq
                    if (req != null) {
                        loadReq = null
                        emit(event("player.loaded").put("id", id).put("req", req).put("durationMs", duration() ?: JSONObject.NULL))
                    } else if (buffering) {
                        buffering = false
                        emit(event("player.buffering").put("id", id).put("buffering", false))
                    }
                }
                Player.STATE_BUFFERING -> if (loadReq == null && !buffering) {
                    buffering = true
                    emit(event("player.buffering").put("id", id).put("buffering", true))
                }
                Player.STATE_ENDED -> {
                    stopTicker()
                    player.playWhenReady = false
                    emit(event("player.ended").put("id", id))
                }
            }
        }

        override fun onIsPlayingChanged(isPlaying: Boolean) {
            if (isPlaying) {
                emit(event("player.playing").put("id", id).put("positionMs", player.currentPosition.coerceAtLeast(0)))
                stopTicker()
                main.postDelayed(ticker, tickMs)
            } else {
                stopTicker()
            }
        }

        override fun onPlayWhenReadyChanged(playWhenReady: Boolean, reason: Int) {
            if (!playWhenReady && player.playbackState != Player.STATE_ENDED) {
                emit(event("player.paused").put("id", id).put("positionMs", player.currentPosition.coerceAtLeast(0)))
            }
        }

        override fun onPositionDiscontinuity(
            oldPosition: Player.PositionInfo,
            newPosition: Player.PositionInfo,
            reason: Int,
        ) {
            if (reason != Player.DISCONTINUITY_REASON_SEEK) return
            val req = seekReq ?: return
            seekReq = null
            emit(event("player.seeked").put("id", id).put("req", req).put("positionMs", newPosition.positionMs))
        }

        override fun onPlayerError(error: PlaybackException) {
            stopTicker()
            val payload = event("player.error").put("id", id)
                .put("code", errorCode(error))
                .put("message", error.message ?: error.errorCodeName)
            loadReq?.let { payload.put("req", it) }
            loadReq = null
            emit(payload)
        }
    }

    /** One preloaded effect; a SoundPool per sound keeps voice limits independent. */
    private class SoundBank(context: Context, private val id: Long) {
        private val pool: SoundPool = SoundPool.Builder()
            .setMaxStreams(16)
            .setAudioAttributes(
                AudioAttributes.Builder()
                    .setUsage(AudioAttributes.USAGE_GAME)
                    .setContentType(AudioAttributes.CONTENT_TYPE_SONIFICATION)
                    .build()
            )
            .build()
        private var soundId = 0
        private var durationMs = 0L
        private val streams = HashMap<Long, Int>()
        private val endTimers = HashMap<Long, Runnable>()

        fun load(req: Long, path: String) {
            durationMs = probeDuration(path)
            pool.setOnLoadCompleteListener { _, _, status ->
                if (status == 0 && durationMs > 0) {
                    emit(event("sound.loaded").put("id", id).put("req", req).put("durationMs", durationMs))
                } else {
                    emit(
                        event("sound.error").put("id", id).put("req", req)
                            .put("code", if (status == 0) "unsupported_format" else "decode_failed")
                            .put("message", "SoundPool could not load $path")
                    )
                }
            }
            soundId = pool.load(path, 1)
            if (soundId == 0) {
                emit(event("sound.error").put("id", id).put("req", req).put("code", "decode_failed").put("message", "SoundPool rejected $path"))
            }
        }

        fun play(voice: Long, volume: Float, rate: Float, pan: Float, loop: Boolean) {
            val left = volume * (1f - pan).coerceAtMost(1f)
            val right = volume * (1f + pan).coerceAtMost(1f)
            val stream = pool.play(soundId, left, right, 1, if (loop) -1 else 0, rate.coerceIn(0.5f, 2f))
            if (stream == 0) {
                emit(event("sound.ended").put("id", id).put("voice", voice))
                return
            }
            streams[voice] = stream
            if (!loop) {
                // SoundPool reports no completion; the voice ends after its scaled duration.
                val done = Runnable {
                    endTimers.remove(voice)
                    if (streams.remove(voice) != null) {
                        emit(event("sound.ended").put("id", id).put("voice", voice))
                    }
                }
                endTimers[voice] = done
                main.postDelayed(done, (durationMs / rate.coerceIn(0.5f, 2f)).toLong() + 20)
            }
        }

        fun stop(voice: Long) {
            endTimers.remove(voice)?.let { main.removeCallbacks(it) }
            streams.remove(voice)?.let { pool.stop(it) }
        }

        fun release() {
            endTimers.values.forEach { main.removeCallbacks(it) }
            endTimers.clear()
            streams.clear()
            pool.release()
        }

        private fun probeDuration(path: String): Long {
            val retriever = MediaMetadataRetriever()
            return try {
                retriever.setDataSource(path)
                retriever.extractMetadata(MediaMetadataRetriever.METADATA_KEY_DURATION)?.toLongOrNull() ?: 0L
            } catch (e: Exception) {
                0L
            } finally {
                try {
                    retriever.release()
                } catch (_: Exception) {
                }
            }
        }
    }

    private class Session : AudioManager.OnAudioFocusChangeListener, SensorEventListener {
        var earpiece = false
            private set
        private var focus: String? = null
        private var focusRequest: Any? = null
        private var transientLoss = false
        private var wantsFocus: String? = null
        private var proximitySensor: Sensor? = null
        private var proximityLock: PowerManager.WakeLock? = null
        private var near = false
        private var observing = false
        private var lastRoute: String? = null

        private fun audio(): AudioManager? =
            context()?.getSystemService(Context.AUDIO_SERVICE) as? AudioManager

        fun apply(active: Boolean, focus: String?, earpiece: Boolean, proximity: Boolean) {
            val gain = when (if (active) focus else null) {
                "exclusive" -> "exclusive"
                "duck" -> "duck"
                else -> null
            }
            wantsFocus = gain
            if (gain != this.focus) {
                // Keep a transiently lost request alive so the regain can end the interruption.
                if (gain != null || !transientLoss) {
                    abandonFocus()
                    if (gain != null) requestFocus(gain)
                }
            }
            val routeEarpiece = active && earpiece
            if (routeEarpiece != this.earpiece) {
                this.earpiece = routeEarpiece
                applyOutput(routeEarpiece)
                players.values.forEach { it.applyRoute(routeEarpiece) }
                emitRoute("override", lost = false)
            }
            setProximity(active && proximity)
        }

        private fun requestFocus(gain: String) {
            val audio = audio() ?: return
            val gainType = if (gain == "exclusive") AudioManager.AUDIOFOCUS_GAIN
            else AudioManager.AUDIOFOCUS_GAIN_TRANSIENT_MAY_DUCK
            val usage = if (earpiece) AudioAttributes.USAGE_VOICE_COMMUNICATION else AudioAttributes.USAGE_MEDIA
            val result = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                val request = AudioFocusRequest.Builder(gainType)
                    .setAudioAttributes(AudioAttributes.Builder().setUsage(usage).build())
                    .setOnAudioFocusChangeListener(this, main)
                    .build()
                focusRequest = request
                audio.requestAudioFocus(request)
            } else {
                @Suppress("DEPRECATION")
                audio.requestAudioFocus(this, AudioManager.STREAM_MUSIC, gainType)
            }
            focus = gain
            if (result != AudioManager.AUDIOFOCUS_REQUEST_GRANTED) {
                focus = null
                emitInterruption("began", lossReason(audio), shouldResume = false)
            }
        }

        private fun abandonFocus() {
            val audio = audio() ?: return
            if (focus == null && focusRequest == null) return
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                (focusRequest as? AudioFocusRequest)?.let { audio.abandonAudioFocusRequest(it) }
            } else {
                @Suppress("DEPRECATION")
                audio.abandonAudioFocus(this)
            }
            focusRequest = null
            focus = null
            transientLoss = false
        }

        private fun lossReason(audio: AudioManager): String = when (audio.mode) {
            AudioManager.MODE_IN_CALL, AudioManager.MODE_RINGTONE -> "call"
            else -> "otherApp"
        }

        override fun onAudioFocusChange(change: Int) {
            val audio = audio() ?: return
            when (change) {
                AudioManager.AUDIOFOCUS_LOSS -> {
                    transientLoss = false
                    abandonFocus()
                    emitInterruption("began", lossReason(audio), shouldResume = false)
                }
                AudioManager.AUDIOFOCUS_LOSS_TRANSIENT -> {
                    transientLoss = true
                    emitInterruption("began", lossReason(audio), shouldResume = false)
                }
                AudioManager.AUDIOFOCUS_GAIN -> if (transientLoss) {
                    transientLoss = false
                    emitInterruption("ended", "otherApp", shouldResume = true)
                    if (wantsFocus == null) abandonFocus()
                }
            }
        }

        private fun emitInterruption(phase: String, reason: String, shouldResume: Boolean) {
            emit(event("interruption").put("phase", phase).put("reason", reason).put("shouldResume", shouldResume))
        }

        @SuppressLint("MissingPermission")
        private fun applyOutput(earpiece: Boolean) {
            val audio = audio() ?: return
            try {
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
                    if (earpiece) {
                        audio.mode = AudioManager.MODE_IN_COMMUNICATION
                        audio.availableCommunicationDevices
                            .firstOrNull { it.type == AudioDeviceInfo.TYPE_BUILTIN_EARPIECE }
                            ?.let { audio.setCommunicationDevice(it) }
                    } else {
                        audio.clearCommunicationDevice()
                        audio.mode = AudioManager.MODE_NORMAL
                    }
                } else {
                    audio.mode = if (earpiece) AudioManager.MODE_IN_COMMUNICATION else AudioManager.MODE_NORMAL
                    @Suppress("DEPRECATION")
                    audio.isSpeakerphoneOn = false
                }
            } catch (e: Exception) {
                LxLog.w(TAG, "audio output switch failed", e)
            }
        }

        private fun setProximity(enabled: Boolean) {
            val context = context() ?: return
            val sensors = context.getSystemService(Context.SENSOR_SERVICE) as? SensorManager ?: return
            if (enabled && proximitySensor == null) {
                val sensor = sensors.getDefaultSensor(Sensor.TYPE_PROXIMITY) ?: return
                proximitySensor = sensor
                sensors.registerListener(this, sensor, SensorManager.SENSOR_DELAY_NORMAL, main)
                val power = context.getSystemService(Context.POWER_SERVICE) as? PowerManager
                if (power != null && power.isWakeLockLevelSupported(PowerManager.PROXIMITY_SCREEN_OFF_WAKE_LOCK)) {
                    proximityLock = power.newWakeLock(PowerManager.PROXIMITY_SCREEN_OFF_WAKE_LOCK, "lingxia:audio-proximity")
                        .also { it.acquire(60 * 60 * 1000L) }
                }
            } else if (!enabled && proximitySensor != null) {
                sensors.unregisterListener(this)
                proximitySensor = null
                proximityLock?.let { if (it.isHeld) it.release(PowerManager.RELEASE_FLAG_WAIT_FOR_NO_PROXIMITY) }
                proximityLock = null
                if (near) {
                    near = false
                    emit(event("proximity").put("near", false))
                }
            }
        }

        override fun onSensorChanged(event: SensorEvent) {
            val sensor = proximitySensor ?: return
            val value = event.values.firstOrNull() ?: return
            val isNear = value < sensor.maximumRange && value < 5f
            if (isNear != near) {
                near = isNear
                emit(event("proximity").put("near", isNear))
            }
        }

        override fun onAccuracyChanged(sensor: Sensor?, accuracy: Int) = Unit

        fun observeRoutes() {
            val context = context() ?: return
            if (observing) {
                emitRoute("other", lost = false, force = true)
                return
            }
            observing = true
            context.registerReceiver(
                object : BroadcastReceiver() {
                    override fun onReceive(context: Context, intent: Intent) {
                        emitRoute("deviceRemoved", lost = true, force = true)
                    }
                },
                IntentFilter(AudioManager.ACTION_AUDIO_BECOMING_NOISY),
            )
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
                audio()?.registerAudioDeviceCallback(
                    object : AudioDeviceCallback() {
                        override fun onAudioDevicesAdded(added: Array<out AudioDeviceInfo>) {
                            emitRoute("deviceAdded", lost = false)
                        }

                        override fun onAudioDevicesRemoved(removed: Array<out AudioDeviceInfo>) {
                            emitRoute("deviceRemoved", lost = false)
                        }
                    },
                    main,
                )
            }
            emitRoute("other", lost = false, force = true)
        }

        private fun emitRoute(reason: String, lost: Boolean, force: Boolean = false) {
            val (output, name) = currentRoute()
            val key = "$output|$name"
            if (!force && key == lastRoute) return
            lastRoute = key
            val payload = event("route").put("output", output).put("reason", reason).put("lost", lost)
            if (name != null) payload.put("name", name)
            emit(payload)
        }

        private fun currentRoute(): Pair<String, String?> {
            val audio = audio() ?: return "speaker" to null
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
                val devices = audio.getDevices(AudioManager.GET_DEVICES_OUTPUTS)
                val ranked = listOf(
                    "bluetooth" to setOfNotNull(
                        AudioDeviceInfo.TYPE_BLUETOOTH_A2DP,
                        AudioDeviceInfo.TYPE_BLUETOOTH_SCO,
                        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) AudioDeviceInfo.TYPE_BLE_HEADSET else null,
                        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) AudioDeviceInfo.TYPE_BLE_SPEAKER else null,
                    ),
                    "headphones" to setOf(AudioDeviceInfo.TYPE_WIRED_HEADSET, AudioDeviceInfo.TYPE_WIRED_HEADPHONES),
                    "usb" to setOfNotNull(
                        AudioDeviceInfo.TYPE_USB_DEVICE,
                        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) AudioDeviceInfo.TYPE_USB_HEADSET else null,
                    ),
                    "hdmi" to setOf(AudioDeviceInfo.TYPE_HDMI),
                )
                for ((output, types) in ranked) {
                    devices.firstOrNull { it.type in types }?.let { device ->
                        return output to device.productName?.toString()?.takeIf { it.isNotBlank() }
                    }
                }
            } else {
                @Suppress("DEPRECATION")
                if (audio.isBluetoothA2dpOn) return "bluetooth" to null
                @Suppress("DEPRECATION")
                if (audio.isWiredHeadsetOn) return "headphones" to null
            }
            return (if (earpiece) "earpiece" else "speaker") to null
        }
    }
}
