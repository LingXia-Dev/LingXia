# Public host APIs that product apps call from Java/Kotlin. Host R8 must
# not strip them — the library itself is not minified.
-keep class com.lingxia.app.Lingxia { *; }
-keep class com.lingxia.app.media.** { *; }
