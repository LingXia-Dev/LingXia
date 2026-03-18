package com.lingxia.lxapp.APIs.media

internal data class PreviewMediaPayload(
    val path: String,
    val type: Int,
    val coverPath: String?,
    val rotate: Int?,
    val objectFit: String?,
    val durationMs: Long?
) : java.io.Serializable
