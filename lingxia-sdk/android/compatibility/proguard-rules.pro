# The separate instrumentation APK calls stdlib entry points that the empty
# target app does not. Preserve those for the runner, not for SDK consumers.
-keep class kotlin.** { *; }
# Called by the separate test APK, which is not part of target reachability.
-keep class com.lingxia.compatibility.PlaybackProbe { *; }
