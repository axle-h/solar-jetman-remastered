// SDL's activity and a manifest around the game's library; builds only through
// tools/package/android.sh, which passes where SDL's Java and the native libraries are.
pluginManagement {
    repositories {
        google()
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

rootProject.name = "solar-jetman"
include(":app")
