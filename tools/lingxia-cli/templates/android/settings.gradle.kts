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
        // mavenCentral resolves the LingXia SDK's transitive dependencies.
        // The LingXia SDK Maven repo itself is injected at build time by
        // `lingxia build` via a Gradle init script (settingsEvaluated hook),
        // since FAIL_ON_PROJECT_REPOS forbids declaring it in build.gradle.kts.
        mavenCentral()
    }
}

rootProject.name = "{{PROJECT_NAME}}"
include(":app")
