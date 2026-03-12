plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.android)
}

val requestedMinSdk = (project.findProperty("MIN_SDK") as String?)?.toIntOrNull() ?: 29

android {
    namespace = "com.lingxia.example.lxapp"
    compileSdk = 35

    defaultConfig {
        applicationId = "com.lingxia.example.lxapp"
        minSdk = requestedMinSdk
        targetSdk = 35
        versionCode = 1
        versionName = "1.0"

        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro"
            )
            // Sign release with debug keystore for local installs
            signingConfig = signingConfigs.getByName("debug")
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_11
        targetCompatibility = JavaVersion.VERSION_11
    }
    kotlinOptions {
        jvmTarget = "11"
    }
}

dependencies {
    val lingxiaSdkVersion = (project.findProperty("LINGXIA_SDK_VERSION") as String?)
        ?: System.getenv("LINGXIA_SDK_VERSION")
        ?: "dev"
    // Depend on local Maven-published SDK
    implementation("com.lingxia:lingxia:$lingxiaSdkVersion")

    // App's own dependencies
    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.appcompat)
    implementation(libs.material)
}
