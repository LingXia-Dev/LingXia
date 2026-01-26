plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.android")
    id("maven-publish")
}

android {
    namespace = "com.lingxia.lxapp"
    compileSdk = 35

    defaultConfig {
        minSdk = 29
        lint.targetSdk = 35

        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro"
            )
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_11
        targetCompatibility = JavaVersion.VERSION_11
    }
    kotlinOptions {
        jvmTarget = "11"
    }

    // Enable publishing of the release variant
    publishing {
        singleVariant("release") {
            // withSourcesJar() // optional
            // withJavadocJar() // optional
        }
    }
}

dependencies {
    // LingXia WebView JAR (built by Rust build.rs or Makefile and placed in Gradle build directory)
    api(files("${layout.buildDirectory.get()}/generated/lingxia-webview/lingxia-webview.jar"))

    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.appcompat)
    implementation(libs.material)
    implementation("androidx.viewpager2:viewpager2:1.0.0")
    implementation("androidx.media3:media3-exoplayer:1.4.1")
    implementation("androidx.media3:media3-ui:1.4.1")
    implementation("androidx.camera:camera-core:1.3.4")
    implementation("androidx.camera:camera-camera2:1.3.4")
    implementation("androidx.camera:camera-lifecycle:1.3.4")
    implementation("androidx.camera:camera-view:1.3.4")
    implementation("androidx.camera:camera-video:1.3.4")
    implementation("androidx.exifinterface:exifinterface:1.3.6")
    implementation("com.google.mlkit:barcode-scanning:17.2.0")
}

publishing {
    publications {
        create<MavenPublication>("release") {
            groupId = "com.lingxia"
            artifactId = "lingxia"
            version = (project.findProperty("version") as String?) ?: "0.0.1"
            afterEvaluate {
                from(components["release"])
            }
        }
    }
    repositories {
        maven {
            name = "localExample"
            val repoDirProp = project.findProperty("LOCAL_MAVEN_REPO_DIR") as String?
            // Default to the workspace Rust cargo target directory
            val fallback = File(rootProject.projectDir, "../../target/maven").absolutePath
            url = uri(repoDirProp ?: fallback)
        }
    }
}
