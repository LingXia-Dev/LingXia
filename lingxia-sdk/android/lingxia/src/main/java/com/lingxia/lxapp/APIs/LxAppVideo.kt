package com.lingxia.lxapp.APIs

import com.lingxia.lxapp.SameLevel.VideoPlayerRegistry

class LxAppVideo {
    companion object {
        @JvmStatic
        fun setVideoPlayerCallback(componentId: String, callbackId: Long) {
            VideoPlayerRegistry.registerCallback(componentId, callbackId)
        }

        @JvmStatic
        fun unregisterVideoPlayer(componentId: String) {
            VideoPlayerRegistry.unregisterCallback(componentId)
        }

        @JvmStatic
        fun dispatchVideoCommand(componentId: String, name: String, paramsJson: String) {
            VideoPlayerRegistry.dispatchCommand(componentId, name, paramsJson)
        }
    }
}
