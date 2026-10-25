# Builds the unsigned APK for tools/package/android.sh, which signs it; the build context is
# the repository root. The `toolchain` stage alone is what android.sh runs keytool and
# apksigner in.
#
# SDL is built here from the release the sdl3 crate binds (android.sh reads it from
# Cargo.lock), because its Java half must be the same release as libSDL3.so: they call each
# other over JNI by name, and a mismatch fails only at runtime.
FROM ubuntu:24.04 AS toolchain

ARG ABI=arm64-v8a
ARG TARGET=aarch64-linux-android
ARG SDL_VERSION
ARG RUST_VERSION
ARG NDK_VERSION=29.0.14206865
ARG CMDLINE_TOOLS=commandlinetools-linux-16111833_latest.zip
ARG PLATFORM=android-36
ARG BUILD_TOOLS=36.1.0
ARG GRADLE_VERSION=9.8.0
# the oldest Android the native libraries are linked against; minSdk in app/build.gradle.kts
ARG API=26
ENV DEBIAN_FRONTEND=noninteractive
ENV ABI=$ABI TARGET=$TARGET

RUN apt-get update \
  && apt-get install -y --no-install-recommends \
       openjdk-21-jdk-headless curl unzip ca-certificates cmake ninja-build gcc libc6-dev \
  && rm -rf /var/lib/apt/lists/*

ENV ANDROID_HOME=/opt/android-sdk
RUN mkdir -p $ANDROID_HOME/cmdline-tools \
  && curl -fsSL https://dl.google.com/android/repository/$CMDLINE_TOOLS -o /tmp/tools.zip \
  && unzip -q /tmp/tools.zip -d $ANDROID_HOME/cmdline-tools \
  && mv $ANDROID_HOME/cmdline-tools/cmdline-tools $ANDROID_HOME/cmdline-tools/latest \
  && rm /tmp/tools.zip \
  && yes | $ANDROID_HOME/cmdline-tools/latest/bin/sdkmanager --licenses > /dev/null \
  && $ANDROID_HOME/cmdline-tools/latest/bin/sdkmanager \
       "platforms;$PLATFORM" "build-tools;$BUILD_TOOLS" "ndk;$NDK_VERSION" > /dev/null
ENV ANDROID_NDK_HOME=$ANDROID_HOME/ndk/$NDK_VERSION
ENV NDK_BIN=$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin
# apksigner, for android.sh
ENV PATH="$ANDROID_HOME/build-tools/$BUILD_TOOLS:$PATH"

RUN curl -fsSL https://services.gradle.org/distributions/gradle-$GRADLE_VERSION-bin.zip -o /tmp/gradle.zip \
  && unzip -q /tmp/gradle.zip -d /opt \
  && rm /tmp/gradle.zip
ENV PATH="/opt/gradle-$GRADLE_VERSION/bin:$PATH"

# libSDL3.so into /jniLibs, and the Java half stays at /sdl/android-project
WORKDIR /sdl
RUN curl -fsSL https://github.com/libsdl-org/SDL/releases/download/release-${SDL_VERSION}/SDL3-${SDL_VERSION}.tar.gz \
     | tar xz --strip-components=1 \
  && cmake -S . -B build -G Ninja \
       -DCMAKE_TOOLCHAIN_FILE=$ANDROID_NDK_HOME/build/cmake/android.toolchain.cmake \
       -DANDROID_ABI=$ABI -DANDROID_PLATFORM=android-$API \
       -DCMAKE_BUILD_TYPE=Release -DSDL_SHARED=ON -DSDL_STATIC=OFF -DSDL_TEST_LIBRARY=OFF \
       -DSDL_TESTS=OFF -DSDL_EXAMPLES=OFF \
  && cmake --build build \
  && mkdir -p /jniLibs/$ABI \
  && cp build/libSDL3.so /jniLibs/$ABI/ \
  && rm -rf build

# The pinned toolchain (rust-toolchain.toml), with only the Android target: RUSTUP_TOOLCHAIN
# overrides the file, which would also fetch the wasm target.
RUN curl https://sh.rustup.rs -sSf | bash -s -- -y --profile minimal --default-toolchain none \
  && /root/.cargo/bin/rustup toolchain install $RUST_VERSION --profile minimal --target $TARGET
ENV PATH="/root/.cargo/bin:${PATH}" RUSTUP_TOOLCHAIN=$RUST_VERSION

# sdl3-sys finds no pkg-config here and links a bare `-lSDL3`, so it only needs to be told
# where libSDL3.so is
ENV CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER=$NDK_BIN/aarch64-linux-android${API}-clang \
    CC_aarch64_linux_android=$NDK_BIN/aarch64-linux-android${API}-clang \
    AR_aarch64_linux_android=$NDK_BIN/llvm-ar \
    CARGO_TARGET_AARCH64_LINUX_ANDROID_RUSTFLAGS="-L native=/jniLibs/arm64-v8a" \
    CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER=$NDK_BIN/x86_64-linux-android${API}-clang \
    CC_x86_64_linux_android=$NDK_BIN/x86_64-linux-android${API}-clang \
    AR_x86_64_linux_android=$NDK_BIN/llvm-ar \
    CARGO_TARGET_X86_64_LINUX_ANDROID_RUSTFLAGS="-L native=/jniLibs/x86_64"

# the Android Gradle plugin in its own layer, so a source change does not fetch it again
WORKDIR /app/android
COPY android/settings.gradle.kts android/build.gradle.kts android/gradle.properties ./
COPY android/app/build.gradle.kts app/
RUN gradle --no-daemon -q help \
      -PsdlJavaDir=/sdl/android-project/app/src/main/java -PjniLibsDir=/jniLibs \
      -PversionCode=1 -PversionName=0

FROM toolchain AS build
ARG VERSION_CODE
ARG VERSION_NAME

WORKDIR /app
COPY . .

# The crates and the build outputs are kept between builds in cache mounts.
RUN --mount=type=cache,target=/root/.cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build --locked --release --example solar-jetman-android --features android \
      --target $TARGET \
  && $NDK_BIN/llvm-strip -o /jniLibs/$ABI/libsolar_jetman_android.so \
       target/$TARGET/release/examples/libsolar_jetman_android.so \
  && $NDK_BIN/llvm-readelf -d /jniLibs/$ABI/libsolar_jetman_android.so | grep NEEDED

WORKDIR /app/android
RUN gradle --no-daemon assembleRelease \
      -PsdlJavaDir=/sdl/android-project/app/src/main/java -PjniLibsDir=/jniLibs \
      -PversionCode=$VERSION_CODE -PversionName=$VERSION_NAME

CMD ["true"]
