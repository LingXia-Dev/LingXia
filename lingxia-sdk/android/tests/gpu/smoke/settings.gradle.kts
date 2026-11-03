pluginManagement { repositories { google(); mavenCentral(); gradlePluginPortal() } }
dependencyResolutionManagement {
    versionCatalogs { create("libs") { from(files("../../../gradle/libs.versions.toml")) } }
    repositories { google(); mavenCentral() }
}
rootProject.name = "LingXiaViewSmoke"
include(":lingxia-core")
project(":lingxia-core").projectDir = file("../../../lingxia")
