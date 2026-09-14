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
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
    }
}

rootProject.name = "lingxia-sdk"
include(":lingxia", ":lingxia-core", ":lingxia-camera", ":lingxia-scanner")

project(":lingxia-core").projectDir = file("lingxia")
project(":lingxia").projectDir = file("lingxia-full")
include(":compatibility")
