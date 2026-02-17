#!/bin/bash

clear

# Auto-detect Android SDK if not provided
if [ -z "$ANDROID_HOME" ] && [ -d "$HOME/Android/Sdk" ]; then
  export ANDROID_HOME="$HOME/Android/Sdk"
fi

# Auto-detect Android NDK if not provided
if [ -z "$ANDROID_NDK_HOME" ] && [ -d "$HOME/Android/Sdk/ndk" ]; then
  NDK_LATEST="$(ls -1 "$HOME/Android/Sdk/ndk" 2>/dev/null | sort -V | tail -n 1)"
  if [ -n "$NDK_LATEST" ] && [ -d "$HOME/Android/Sdk/ndk/$NDK_LATEST" ]; then
    export ANDROID_NDK_HOME="$HOME/Android/Sdk/ndk/$NDK_LATEST"
  fi
fi

if [ -z "$ANDROID_NDK_HOME" ]; then
  echo "error: Could not find Android NDK."
  echo "note: Set ANDROID_NDK_HOME to your NDK installation root directory."
  echo "      Example: export ANDROID_NDK_HOME=\"\$HOME/Android/Sdk/ndk/27.0.12077973\""
  exit 1
fi

# Setup
BUILD_DIR=platform-build
mkdir -p $BUILD_DIR
cd $BUILD_DIR

# Create the jniLibs build directory
JNI_DIR=../../Save-app-android/app/src/main/jniLibs
mkdir -p $JNI_DIR

# Make sure we're on the latest all the time
#
# cargo update save-dweb-backend

# Add this target if we need to support older devices.
# armv7-linux-androideabi
#
rustup target add \
        aarch64-linux-android \
        x86_64-linux-android

# Build the android libraries in the jniLibs directory
#
# Add this target if we need to support older devices.
# armeabi-v7a
#
cargo ndk -o $JNI_DIR \
        --manifest-path ../Cargo.toml \
        -t arm64-v8a \
        -t x86_64 \
        build --release
