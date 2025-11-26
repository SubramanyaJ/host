# Android Build Guide for Pineapple

## Prerequisites

1. **Install Android NDK**
   - Download from: https://developer.android.com/ndk/downloads
   - Recommended version: NDK r26 or later
   - Set environment variable: `export NDK_HOME=/path/to/ndk`

2. **Install Rust and targets**
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android i686-linux-android
   ```

3. **Setup PATH for NDK toolchain**
   Add to your `.bashrc` or `.zshrc`:
   ```bash
   export NDK_HOME=$HOME/Android/Sdk/ndk/26.1.10909125
   export PATH=$NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin:$PATH
   ```

## Building

### Using the build script (Recommended)
```bash
cd /path/to/pineapple
chmod +x scripts/build-android.sh
./scripts/build-android.sh
```

### Manual build for specific architecture
```bash
# ARM64
cargo build --release --target aarch64-linux-android

# ARMv7
cargo build --release --target armv7-linux-androideabi

# x86_64
cargo build --release --target x86_64-linux-android

# x86
cargo build --release --target i686-linux-android
```

## Output

The compiled .so libraries will be in android/jniLibs/

## Why rustls?

This project uses rustls instead of OpenSSL because:
- No native dependencies
- Cross-platform builds
- No build complexity
- Smaller binaries
- Memory safe

The WebSocket client uses tokio-tungstenite with rustls-tls-native-roots.
