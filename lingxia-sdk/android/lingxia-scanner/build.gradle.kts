import com.vanniktech.maven.publish.AndroidSingleVariantLibrary

plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.android")
    id("maven-publish")
    id("com.vanniktech.maven.publish") version "0.34.0" apply false
}

android {
    namespace = "com.lingxia.modules.full.scanner"
    compileSdk = 35
    defaultConfig {
        minSdk = 23
        consumerProguardFiles("consumer-rules.pro")
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_11
        targetCompatibility = JavaVersion.VERSION_11
    }
    kotlinOptions { jvmTarget = "11" }
}

dependencies {
    implementation(project(":lingxia-core"))
    implementation(libs.androidx.appcompat)
    implementation(libs.androidx.camera.core)
    implementation(libs.androidx.camera.camera2)
    implementation(libs.androidx.camera.lifecycle)
    implementation(libs.androidx.camera.view)
    implementation(libs.mlkit.barcode.scanning)
    // ProcessCameraProvider exposes ListenableFuture; ML Kit selects the empty Guava placeholder.
    implementation("com.google.guava:guava:33.3.1-android")
}

if (rootProject.name == "lingxia-sdk") {
    apply(plugin = "com.vanniktech.maven.publish")
    extensions.configure<com.vanniktech.maven.publish.MavenPublishBaseExtension> {
        coordinates("io.github.lingxia-dev", "lingxia-scanner", (project.findProperty("version") as String?) ?: "0.0.1")
        configure(AndroidSingleVariantLibrary(variant = "release", sourcesJar = true, publishJavadocJar = true))
    }
}
publishing {
    repositories {
        maven {
            name = "localExample"
            url = uri(project.findProperty("LOCAL_MAVEN_REPO_DIR") as String?
                ?: File(rootProject.projectDir, "../../target/maven").absolutePath)
        }
    }
}
