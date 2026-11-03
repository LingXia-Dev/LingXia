plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.android) apply false
}
android {
    namespace = "com.lingxia.view"
    compileSdk = 35
    defaultConfig {
        applicationId = "com.lingxia.view.smoke"
        minSdk = 23
        targetSdk = 35
        versionCode = 1
        versionName = "0.1"
        ndk { abiFilters += "arm64-v8a" }
    }
    sourceSets["main"].assets.srcDir(providers.gradleProperty("uiAssets").getOrElse("../../examples/hello-view/dist"))
}
dependencies { implementation(project(":lingxia-core")) }
