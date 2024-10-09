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
# armv7-linux-androideabi
rustup target add \
        aarch64-linux-android \
         \

# Build the android libraries in the jniLibs directory
# armeabi-v7a
cargo ndk -o $JNI_DIR \
        --manifest-path ../Cargo.toml \
        -t arm64-v8a \
        build --release 
