pluginManagement {
    repositories {
        google {
            content {
                includeGroupByRegex("com\\.android.*")
                includeGroupByRegex("com\\.google.*")
                includeGroupByRegex("androidx.*")
            }
        }
        mavenCentral()
        gradlePluginPortal()
    }
}
dependencyResolutionManagement {
    // Included SDK projects use this build's catalog, not the SDK root's.
    versionCatalogs {
        create("libs") {
            from(files("../../../lingxia-sdk/android/gradle/libs.versions.toml"))
        }
    }
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
    }
}

rootProject.name = "lingxia-example"
include(":app")
include(":lingxia", ":lingxia-core", ":lingxia-camera", ":lingxia-scanner")
project(":lingxia").projectDir = file("../../../lingxia-sdk/android/lingxia-full")
project(":lingxia-core").projectDir = file("../../../lingxia-sdk/android/lingxia")
project(":lingxia-camera").projectDir = file("../../../lingxia-sdk/android/lingxia-camera")
project(":lingxia-scanner").projectDir = file("../../../lingxia-sdk/android/lingxia-scanner")
