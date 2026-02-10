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
        // LingXia SDK local Maven repository
        maven {
            url = uri("../target/maven")
        }
        google()
        mavenCentral()
    }
}

rootProject.name = "{{PROJECT_NAME}}"
include(":app")
