plugins {
    id("com.android.application")
}

// tools/package/android.sh passes all of these
fun required(name: String): String =
    providers.gradleProperty(name).orNull
        ?: throw GradleException("-P$name is required: build with tools/package/android.sh")

android {
    namespace = "com.ax_h.solarjetman"
    compileSdk = 36

    defaultConfig {
        applicationId = "com.ax_h.solarjetman"
        // the adaptive icon is the only icon, and 26 is where they start
        minSdk = 26
        targetSdk = 36
        versionCode = required("versionCode").toInt()
        versionName = required("versionName")
    }

    sourceSets {
        getByName("main") {
            // must be the same SDL release as libSDL3.so: the two call each other by name
            java.directories.add(required("sdlJavaDir"))
            jniLibs.directories.add(required("jniLibsDir"))
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            // android.sh signs it
            signingConfig = null
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    lint {
        checkReleaseBuilds = false
    }
}
