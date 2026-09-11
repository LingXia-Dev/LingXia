package com.lingxia.app.media

/**
 * URL 源。
 *
 * v1 框架传给引擎的 [headers] **恒为空**。[com.lingxia.lxapp.APIs.media.LxMediaPlayer]
 * 今日没有 headers 入口，预览 / `<LxVideo>` 也不传 Cookie。宿主不要实现
 * `http-header-fields` 并指望预览带上登录态。字段留下是为了以后只改
 * loadSource、不改引擎协议。
 */
data class UrlPlayerSource(
    val url: String,
    val headers: Map<String, String> = emptyMap(),
)

enum class UrlPlayerSurfaceKind {
    /** 全屏 MediaPreviewFragment。宿主引擎在 API ≥ 24 时允许 SurfaceView。 */
    PREVIEW,
    /** &lt;LxVideo&gt; / MediaSwiper。必须 TextureView。 */
    INLINE,
}

enum class UrlPlayerOutputKind {
    TEXTURE_VIEW,
    SURFACE_VIEW,
}

sealed class UrlPlayerOutput {
    data class Texture(val textureView: android.view.TextureView) : UrlPlayerOutput()
    data class Surface(val surfaceView: android.view.SurfaceView) : UrlPlayerOutput()
}

data class UrlPlayerVideoSize(
    val width: Int,
    val height: Int,
    val rotationDegrees: Int = 0,
)

enum class UrlPlayerErrorCode(val value: String) {
    ABORTED("aborted"),
    NETWORK("network"),
    TIMEOUT("timeout"),
    DECODE("decode"),
    UNSUPPORTED("unsupported"),
    DRM("drm"),
    SURFACE("surface"),
    INTERNAL("internal"),
    UNKNOWN("unknown"),
}

data class UrlPlayerError(
    val code: UrlPlayerErrorCode,
    val message: String,
    val nativeCode: String? = null,
    val httpStatus: Int? = null,
    val retryable: Boolean? = null,
)

sealed interface UrlPlayerEngineEvent {
    data class Prepared(
        val durationMs: Long?,
        val videoSize: UrlPlayerVideoSize?,
    ) : UrlPlayerEngineEvent

    data class BufferingChanged(
        val isBuffering: Boolean,
        val reason: UrlPlayerWaitingReason,
    ) : UrlPlayerEngineEvent

    data class PlayingChanged(val isPlaying: Boolean) : UrlPlayerEngineEvent
    data class TimeUpdate(val currentTimeMs: Long, val durationMs: Long?) : UrlPlayerEngineEvent
    data class SeekCompleted(val currentTimeMs: Long) : UrlPlayerEngineEvent
    data object FirstFrameRendered : UrlPlayerEngineEvent
    data object Ended : UrlPlayerEngineEvent
    data class Error(val error: UrlPlayerError) : UrlPlayerEngineEvent
}

enum class UrlPlayerWaitingReason(val value: String) {
    INITIAL("initial"),
    BUFFERING("buffering"),
    SEEKING("seeking"),
    SURFACE_REBIND("surface_rebind"),
    QUALITY_SWITCH("quality_switch"),
    DECODER("decoder"),
}

fun interface UrlPlayerEngineListener {
    fun onEngineEvent(event: UrlPlayerEngineEvent)
}

/**
 * 宿主 URL 播放引擎。所有方法在主线程调用；事件也必须投递到主线程。
 *
 * 引擎把像素画进 [attachOutput] 给的 View。不要自己往窗口里 addView。
 * [UrlPlayerEngineRequest.output] 只描述 View 种类与实例，**不是**绑定位。
 * [UrlPlayerEngineFactory.create] 可以 stash View 引用，**不得**注册 SurfaceTextureListener /
 * SurfaceHolder.Callback，也不得 setSurface。listener 只在 [attachOutput] 注册、
 * [detachOutput] 注销。
 * 不要实现 objectFit / 内容旋转 —— 0° fit 由框架给内层 View 按视频宽高比布局；
 * 90/270 旋的是 wrapper。引擎铺满内层矩形即可。
 */
interface UrlPlayerEngine {
    fun setListener(listener: UrlPlayerEngineListener?)

    fun setSource(source: UrlPlayerSource)
    fun attachOutput(output: UrlPlayerOutput)
    fun detachOutput()

    fun play()
    fun pause()
    fun stop()
    fun seek(positionMs: Long)

    fun setVolume(volume: Float)
    fun setMuted(muted: Boolean)
    fun setRate(rate: Float)
    fun setLoopEnabled(loopEnabled: Boolean)

    fun getCurrentTimeMs(): Long
    fun getDurationMs(): Long?
    fun isPlaying(): Boolean

    fun release()
}

data class UrlPlayerEngineRequest(
    val context: android.content.Context,
    val ownerKey: String,
    val surfaceKind: UrlPlayerSurfaceKind,
    /** 框架已创建的输出 View。create() 不得 addView，也不得在这里 bind Surface。 */
    val output: UrlPlayerOutput,
)

/**
 * Java：只需实现 [create]；[preferredOutput] 有 default = TEXTURE_VIEW。
 * 编译打开 `-Xjvm-default=all`，Java 8+ 可不当成抽象方法。
 */
interface UrlPlayerEngineFactory {
    fun preferredOutput(kind: UrlPlayerSurfaceKind): UrlPlayerOutputKind =
        UrlPlayerOutputKind.TEXTURE_VIEW

    /**
     * 主线程。可返回 `null` 表示这个 surface 用 SDK 默认 ExoPlayer。
     * 抛错由 SDK catch，视为 null（并记 `LxLog.e`）。
     * 不要在这里 `System.loadLibrary`、注册 surface callback、或阻塞式建 mpv context。
     */
    fun create(request: UrlPlayerEngineRequest): UrlPlayerEngine?
}
