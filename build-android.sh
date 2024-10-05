#!/bin/bash

clear

# Setup
BUILD_DIR=platform-build
mkdir -p $BUILD_DIR
cd $BUILD_DIR

# Create the jniLibs build directory
JNI_DIR=$HOME/Projects/save-android/app/src/main/jniLibs
mkdir -p $JNI_DIR

# Set up cargo-ndk
# cargo install cargo-ndk
rustup target add \
        aarch64-linux-android \
        armv7-linux-androideabi \

# Build the android libraries in the jniLibs directory
cargo ndk -o $JNI_DIR \
        --manifest-path ../Cargo.toml \
        -t arm64-v8a \
        -t armeabi-v7a \
        build --release 
