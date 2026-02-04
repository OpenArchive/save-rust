# Save Android (DWeb): Next Steps to Build an APK + Test

This is a **copy/paste checklist** for rebuilding the embedded Rust server (`save-rust`) and producing an Android debug APK (`Save-app-android`) you can install and test on-device.

## 0) What you’re building (quick mental model)

- Android loads the native library via `System.loadLibrary("save")` → expects **`libsave.so`**
- Rust runs an embedded Actix server on:
  - `http://localhost:8080/status`
  - `http://localhost:8080/health/ready` (**this is what Android uses for readiness**)
- The `.so` must be present under the Android app at `Save-app-android/app/src/main/jniLibs/<abi>/libsave.so`

## 1) One-time machine setup

Install prerequisites:

```bash
# ADB (for install/logcat)
sudo dnf install -y android-tools

# Java toolchain (needs `javac` for Gradle)
# (Either 17 or 21 is fine; the project targets Java 17 source/bytecode.)
sudo dnf install -y java-17-openjdk-devel

# Rust + Android targets (arm64 is enough for most phones)
rustup target add aarch64-linux-android

# Cargo NDK helper
cargo install cargo-ndk
```

Make sure your Android NDK is installed. This workspace already has one under:

- `/home/v/Android/Sdk/ndk/27.0.12077973` (also `25.1.8937393`)

`save-rust/build-android.sh` will auto-detect the latest NDK under `~/Android/Sdk/ndk/`, but you can also set it explicitly:

```bash
export ANDROID_NDK_HOME="$HOME/Android/Sdk/ndk/27.0.12077973"
# or
export ANDROID_NDK_ROOT="$ANDROID_NDK_HOME"
```

## 2) Build the Rust `.so` into the Android app

From the repo root:

```bash
cd save-rust
./build-android.sh
```

Expected output on disk:

- `Save-app-android/app/src/main/jniLibs/arm64-v8a/libsave.so`

Optional: additional ABIs (if you need them later)

- `armeabi-v7a` (older 32-bit devices)
- `x86_64` (emulator)

## 3) Build the Android debug APK

From the repo root:

```bash
cd Save-app-android

# If Gradle fails complaining that JAVA_COMPILER is missing,
# you likely have a JRE installed (java runtime) but not the JDK (javac).
# On Fedora:
sudo dnf install -y java-21-openjdk-devel

# Optionally point Gradle at a known JDK:
# export JAVA_HOME="/usr/lib/jvm/java-21-openjdk"

./gradlew :app:assembleDevDebug
```

Expected APK:

- `Save-app-android/app/build/outputs/apk/dev/debug/app-dev-debug.apk`

(If your output APK name differs, list the folder:)

```bash
ls -1 app/build/outputs/apk/dev/debug/
```

## 4) Install on device (wireless ADB recommended)

Pair/connect (Android 11+):

```bash
adb pair <ip>:<pairing-port>
adb connect <ip>:<debug-port>
```

Install:

```bash
adb install -r app/build/outputs/apk/dev/debug/app-dev-debug.apk
```

## 5) Smoke test checklist (what “good” looks like)

Start logs:

```bash
adb logcat -c
adb logcat -s SnowbirdBridge:* SnowbirdService:* save:* veilid:*
```

In the app:

- Open Save (dev build)
- Enable/Start DWeb server (“Snowbird”)
- Wait until it reports **Connected**

Backend readiness expectations:

- `/status` should return 200 quickly (HTTP server is listening)
- `/health/ready` should return 200 once Veilid/Iroh/Blobs initialization completes
- If initialization fails, you should now see a clear error in logcat (no more silent failure)

## 6) If it still gets stuck on “Connecting…”

Quick triage steps:

- Confirm the APK contains the updated native library:
  - rebuild `save-rust` (`./build-android.sh`) then rebuild APK (`assembleDevDebug`)
- Watch for a Veilid startup error (timeouts, permission issues, etc.) in `adb logcat`
- Confirm the app has permissions: `INTERNET` and `ACCESS_NETWORK_STATE`

## 7) Desktop fallback (fast iteration)

If you want to validate backend behavior without Android:

```bash
cd save-rust
RUST_LOG=info cargo run --bin save-server
```

Endpoints:

- `http://localhost:8080/status`
- `http://localhost:8080/health/ready`
