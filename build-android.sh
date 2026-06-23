#!/bin/bash

clear

# Auto-detect Android SDK if not provided
if [ -z "$ANDROID_HOME" ]; then
  if [ -d "$HOME/Android/Sdk" ]; then
    export ANDROID_HOME="$HOME/Android/Sdk"
  elif [ -d "$HOME/Library/Android/sdk" ]; then
    export ANDROID_HOME="$HOME/Library/Android/sdk"
  fi
fi

# Auto-detect Android NDK if not provided
if [ -z "$ANDROID_NDK_HOME" ] && [ -n "$ANDROID_HOME" ] && [ -d "$ANDROID_HOME/ndk" ]; then
  NDK_LATEST=""
  for ndk_dir in "$ANDROID_HOME/ndk"/*/; do
    [ -d "$ndk_dir" ] || continue
    candidate="$(basename "$ndk_dir")"
    if [ -z "$NDK_LATEST" ] || [ "$(printf '%s\n' "$NDK_LATEST" "$candidate" | sort -V | tail -n 1)" = "$candidate" ]; then
      NDK_LATEST="$candidate"
    fi
  done
  if [ -n "$NDK_LATEST" ] && [ -d "$ANDROID_HOME/ndk/$NDK_LATEST" ]; then
    export ANDROID_NDK_HOME="$ANDROID_HOME/ndk/$NDK_LATEST"
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

# arm64-v8a: modern 64-bit ARM devices (primary production ABI)
# armv7-linux-androideabi: older 32-bit ARM devices (armeabi-v7a)
# x86_64-linux-android: emulators and x86_64 devices (dev/CI)
rustup target add \
        aarch64-linux-android \
        armv7-linux-androideabi \
        x86_64-linux-android

# Build the android libraries in the jniLibs directory
cargo ndk -o $JNI_DIR \
        --manifest-path ../Cargo.toml \
        -t arm64-v8a \
        -t armeabi-v7a \
        -t x86_64 \
        build --release