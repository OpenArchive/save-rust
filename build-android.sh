#!/bin/bash

clear

# Setup
BUILD_DIR=platform-build
mkdir -p $BUILD_DIR
cd $BUILD_DIR

# Create the jniLibs build directory
JNI_DIR=$HOME/Projects/save-android/app/src/main/jniLibs
mkdir -p $JNI_DIR

# Make sure we're on the latest all the time
#
cargo update save-dweb-backend

# Add this target if we need to support older devices.
# armv7-linux-androideabi
#
rustup target add \
        aarch64-linux-android 

# Build the android libraries in the jniLibs directory
#
# Add this target if we need to support older devices.
# armeabi-v7a
#
cargo ndk -o $JNI_DIR \
        --manifest-path ../Cargo.toml \
        -t arm64-v8a \
        build --release 
