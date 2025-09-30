package com.lingxia.lxapp.media

data class PreviewMediaPayload(
    val url: String,
    val type: Int,
    val coverUrl: String?
) : java.io.Serializable
