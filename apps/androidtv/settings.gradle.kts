// Google's mirror of Maven Central comes after Central: Gradle only asks it
// when Central says "not found", which Central now and then answers for files
// it has (CI failed on kotlin-stdlib, error_prone_annotations, …). Written
// out twice: pluginManagement has to come first, before any val.

pluginManagement {
    repositories {
        google()
        mavenCentral()
        maven("https://maven-central.storage-download.googleapis.com/maven2/")
        gradlePluginPortal()
    }
}
dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
        maven("https://maven-central.storage-download.googleapis.com/maven2/")
    }
}
rootProject.name = "valo-tv"
include(":app")
