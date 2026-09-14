import com.vanniktech.maven.publish.AndroidSingleVariantLibrary

plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.android")
    id("maven-publish")
    id("com.vanniktech.maven.publish") version "0.34.0" apply false
}

android {
    namespace = "com.lingxia.modules.full"
    compileSdk = 35
    defaultConfig {
        minSdk = 24
        consumerProguardFiles("consumer-rules.pro")
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_11
        targetCompatibility = JavaVersion.VERSION_11
    }
    kotlinOptions { jvmTarget = "11" }
}

dependencies {
    api(project(":lingxia-core"))
    api(project(":lingxia-camera"))
    api(project(":lingxia-scanner"))
    implementation(libs.androidx.appcompat)
    implementation(libs.androidx.webkit)
    implementation(libs.material)
}

if (rootProject.name == "lingxia-sdk") {
    apply(plugin = "com.vanniktech.maven.publish")
    extensions.configure<com.vanniktech.maven.publish.MavenPublishBaseExtension> {
        coordinates("io.github.lingxia-dev", "lingxia", (project.findProperty("version") as String?) ?: "0.0.1")
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
