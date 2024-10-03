#!/bin/bash

# Setup
BUILD_DIR=platform-build
mkdir $BUILD_DIR
cd $BUILD_DIR

# Create the jniLibs build directory
# JNI_DIR=jniLibs
JNI_DIR=$HOME/Projects/save-android/app/src/main/jniLibs
mkdir -p $JNI_DIR

# Set up cargo-ndk
# cargo install cargo-ndk
rustup target add \
        aarch64-linux-android

        # armv7-linux-androideabi \
        # x86_64-linux-android \
        # i686-linux-android

# Build the android libraries in the jniLibs directory
clear && cargo ndk -o $JNI_DIR \
        --manifest-path ../Cargo.toml \
        -t arm64-v8a \
        build --release 

# -t armeabi-v7a \
# -t x86 \
# -t x86_64 \

# Archive the dynamic libs
# cd $JNI_DIR
# tar -czvf ../EmbeddedMilliAndroid.tar.gz *
# cd -

# Cleanup
# rm -rf $JNI_DIR
