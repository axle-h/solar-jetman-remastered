# Android

An APK for Android handhelds with a built-in pad (Retroid, AYN, Anbernic and the like), played with the pad or any controller Android sees. **There are no touch controls.** Landscape only, Android 8.0 or later.

## Building

```
tools/package/android.sh                  # target/package/solar-jetman-VERSION-android-arm64.apk
ABI=x86_64 tools/package/android.sh       # ...-android-x86_64.apk, for the Android emulator
DEPLOY=1 tools/package/android.sh         # and `adb install` it on the device adb sees
```

Everything builds in Docker from `android/Dockerfile`: the Android SDK and NDK, Gradle, the pinned Rust toolchain, and SDL built for Android from source at the release `sdl3-sys` binds (read from `Cargo.lock`), since SDL's Java half and `libSDL3.so` must be the same release. Crates and build output are kept in BuildKit cache mounts, so a rebuild after a source change is quick. The script then runs `scan-release` on the game's library in the APK, as the other packages do (`SJ_SKIP_SCAN=1` for a dry run).

The first build makes the signing key `android/release.keystore` (gitignored). **Keep it**: Android installs an update only over an app signed with the same key, and uninstalling to change keys deletes the player's folder. `ANDROID_KEYSTORE` and `ANDROID_KEYSTORE_PASSWORD` choose another key. Without adb, copy the APK to the device and open it in a file manager, allowing installs from unknown sources.

## How it fits together

- The program is `src/app/main.rs` either way. On Android it is built as a library by the `solar-jetman-android` example target (`Cargo.toml`, behind the `android` feature): `src/app/android.rs` takes in `main.rs` with `#[path]` and exports the `SDL_main` that SDL's activity calls. This is why the app's modules name each other with `super::` rather than `crate::`.
- `android/` is a Gradle project with little in it: `MainActivity` (SDL's `SDLActivity`, loading `libSDL3.so` and the game), the manifest and a vector launcher icon.
- The ROM is chosen in Android's file picker (SDL's file dialog). It hands back a `content://` URI, which the program reads through SDL rather than `std::fs`.
- The player's folder is `Android/data/com.ax_h.solarjetman/files/` on shared storage (SDL's external storage path), or the app's internal storage when shared storage cannot be written.

## Differences from the desktop

- The system's back button is the port menu's key (bound to it on the Keyboard page, as `AC BACK`), so it opens and closes the menu rather than leaving the game. Select+Start also opens it. The manifest's `enableOnBackInvokedCallback="false"` keeps Android 16 sending the key at all.
- Switching to another app opens the port menu, which pauses the game and its sound.
- Fullscreen is on by default (it hides the system bars), and the Video page has no window scale or fullscreen rows.
- Errors go to logcat (`adb logcat -s SDL/APP`) and, if the program stops, a message box.
- The command line options do not exist: an Android app is started without arguments.
