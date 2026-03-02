package com.lingxia.lxapp.APIs.media

data class PreviewMediaPayload(
    val path: String,
    val type: Int,
    val coverPath: String?,
    val rotate: Int?,
    val objectFit: String?
) : java.io.Serializable
