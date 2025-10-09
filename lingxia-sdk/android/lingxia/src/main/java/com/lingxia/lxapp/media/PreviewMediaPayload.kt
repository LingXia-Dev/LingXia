package com.lingxia.lxapp.media

data class PreviewMediaPayload(
    val path: String,
    val type: Int,
    val coverPath: String?
) : java.io.Serializable
