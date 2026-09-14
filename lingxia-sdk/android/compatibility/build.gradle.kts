plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.android)
}
android {
    namespace = "com.lingxia.compatibility"
    compileSdk = 35
    defaultConfig {
        applicationId = "com.lingxia.compatibility"
        minSdk = 21
        targetSdk = 35
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }
    buildFeatures { buildConfig = true }
    buildTypes {
        debug {
            isMinifyEnabled = providers.gradleProperty("compatibility.minify").orNull == "true"
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
        }
    }
    flavorDimensions += "modules"
    productFlavors {
        create("core") { dimension = "modules" }
        create("full") { dimension = "modules"; minSdk = 24 }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_11
        targetCompatibility = JavaVersion.VERSION_11
    }
    kotlinOptions { jvmTarget = "11" }
}
dependencies {
    "coreImplementation"(project(":lingxia-core"))
    "fullImplementation"(project(":lingxia"))
    androidTestImplementation("androidx.test:runner:1.6.2")
    androidTestImplementation("androidx.test:core:1.6.1")
    androidTestImplementation("androidx.test.ext:junit:1.2.1")
    implementation(libs.media3.exoplayer)
}
