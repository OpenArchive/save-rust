#!/bin/bash

# Exit if anything fails
set -e

# Setup
BUILD_DIR=platform-build

if [ ! -d $BUILD_DIR ]; then
	mkdir $BUILD_DIR
fi

cd $BUILD_DIR

# Build static libs
for TARGET in aarch64-apple-ios x86_64-apple-ios aarch64-apple-ios-sim
do
    rustup target add $TARGET
    # Apple's App Sandbox disallows SysV semaphores; use POSIX semaphores instead
    cargo build -r --target=$TARGET # --features posix-sem
done

echo "Done with rustup"

# Create XCFramework zip
#
FRAMEWORK="HelloWorld.xcframework"
LIBNAME=libhello.a

# mkdir mac-lipo ios-sim-lipo
mkdir ios-sim-lipo

echo "Made lipo directories"

IOS_SIM_LIPO=ios-sim-lipo/$LIBNAME
# MAC_LIPO=mac-lipo/$LIBNAME

lipo -create -output $IOS_SIM_LIPO \
        ../target/aarch64-apple-ios-sim/release/$LIBNAME \
        ../target/x86_64-apple-ios/release/$LIBNAME

echo "Created $IOS_SIM_LIPO"

# lipo -create -output $MAC_LIPO \
#         ../target/aarch64-apple-darwin/release/$LIBNAME \
#         ../target/x86_64-apple-darwin/release/$LIBNAME

# echo "Created $MAC_LIPO"

#         -library $MAC_LIPO \

xcodebuild -create-xcframework \
        -library $IOS_SIM_LIPO \
        -library ../target/aarch64-apple-ios/release/$LIBNAME \
        -output $FRAMEWORK
zip -r $FRAMEWORK.zip $FRAMEWORK

# Cleanup
#
# rm -rf ios-sim-lipo mac-lipo $FRAMEWORK
rm -rf ios-sim-lipo $FRAMEWORK
