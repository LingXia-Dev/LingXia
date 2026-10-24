pluginManagement { repositories { google(); mavenCentral(); gradlePluginPortal() } }
dependencyResolutionManagement {
    versionCatalogs { create("libs") { from(files("../../lingxia-sdk/android/gradle/libs.versions.toml")) } }
    repositories { google(); mavenCentral() }
}
rootProject.name = "LingXiaUiSmoke"
include(":lingxia-core")
project(":lingxia-core").projectDir = file("../../lingxia-sdk/android/lingxia")
