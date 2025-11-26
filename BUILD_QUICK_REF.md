# Quick Build Reference

## TL;DR - Just Build It

### Desktop
```bash
cargo build --release
```

### Android
```bash
export NDK_HOME=$HOME/Android/Sdk/ndk/26.1.10909125
./scripts/build-android.sh
```

### iOS
```bash
./scripts/build-ios-xcframework.sh
```

## Prerequisites Checklist

### All Platforms
- [ ] Rust 1.75.0+ installed: `rustup --version`
- [ ] Cargo available: `cargo --version`

### Android
- [ ] Android NDK r25c+ installed
- [ ] NDK_HOME environment variable set
- [ ] NDK bin directory in PATH

### iOS (macOS only)
- [ ] Xcode 14.0+ installed
- [ ] Xcode command line tools: `xcode-select --install`

## Android NDK Setup

### Linux/macOS
```bash
# Download NDK from https://developer.android.com/ndk/downloads
# Or install via Android Studio SDK Manager

# Set environment variable (add to ~/.bashrc or ~/.zshrc)
export NDK_HOME=$HOME/Android/Sdk/ndk/26.1.10909125
export PATH=$NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin:$PATH

# Install Rust targets
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android i686-linux-android
```

### Windows
```cmd
REM Download NDK from https://developer.android.com/ndk/downloads
REM Or install via Android Studio SDK Manager

REM Set environment variable (System Properties -> Environment Variables)
set NDK_HOME=C:\Users\YourName\AppData\Local\Android\Sdk\ndk\26.1.10909125
set PATH=%NDK_HOME%\toolchains\llvm\prebuilt\windows-x86_64\bin;%PATH%

REM Install Rust targets
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android i686-linux-android
```

## Build Outputs

### Desktop
- **Linux**: `target/release/libpineapple.so`
- **macOS**: `target/release/libpineapple.dylib`
- **Windows**: `target/release/pineapple.dll`

### Android
```
android/jniLibs/
├── arm64-v8a/libpineapple.so      (most devices)
├── armeabi-v7a/libpineapple.so    (older devices)
├── x86_64/libpineapple.so         (emulators)
└── x86/libpineapple.so            (old emulators)
```

### iOS
- **Universal library**: `target/universal/release/libpineapple.a`
- **XCFramework**: `Pineapple.xcframework/` (multiple architectures)

## Common Issues

### "NDK_HOME not set"
```bash
# Find your NDK
ls $HOME/Android/Sdk/ndk/
# Or on Windows
dir %LOCALAPPDATA%\Android\Sdk\ndk

# Set it
export NDK_HOME=$HOME/Android/Sdk/ndk/26.1.10909125  # Use your version
```

### "linker not found"
```bash
# Ensure NDK bin is in PATH
export PATH=$NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin:$PATH

# On macOS use darwin-x86_64:
export PATH=$NDK_HOME/toolchains/llvm/prebuilt/darwin-x86_64/bin:$PATH

# On Windows use windows-x86_64:
set PATH=%NDK_HOME%\toolchains\llvm\prebuilt\windows-x86_64\bin;%PATH%
```

### "OpenSSL error" (shouldn't happen)
If you see OpenSSL errors, you're using an old version. Update to the latest code which uses rustls:
```bash
git pull origin main
cargo clean
cargo build --release
```

### Build is slow
First build is always slow. Subsequent builds are faster due to incremental compilation.

Speed up with:
```bash
# Use more CPU cores
cargo build --release -j 8

# Or set in .cargo/config.toml
[build]
jobs = 8
```

## Verification

### Check dependencies
```bash
cargo tree | grep -E "rustls|tungstenite"
# Should show rustls, NOT openssl
```

### Verify Android libraries
```bash
ls -lh android/jniLibs/*/*.so
# Should show 4 libraries, each 2-5 MB

# Check architecture (Linux)
file android/jniLibs/arm64-v8a/libpineapple.so
# Should output: ELF 64-bit LSB shared object, ARM aarch64
```

### Test the library
```bash
cargo test
# All tests should pass
```

## Integration with Flutter

Copy Android libraries to your Flutter project:
```bash
cp -r android/jniLibs /path/to/your_flutter_app/android/app/src/main/
```

Or for iOS, drag `Pineapple.xcframework` into Xcode.

## Need Help?

1. **Read the docs**: 
   - [PORT.md](PORT.md) - Full API reference
   - [ANDROID_BUILD.md](ANDROID_BUILD.md) - Detailed Android guide
   - [RUSTLS_MIGRATION.md](RUSTLS_MIGRATION.md) - TLS implementation details

2. **Check logs**: Enable debug output
   ```bash
   export RUST_LOG=pineapple=debug
   cargo build --release
   ```

3. **File an issue**: https://github.com/SubramanyaJ/pineapple/issues

## Success!

When everything works, you'll have:
- ✅ Native libraries built for all target platforms
- ✅ No OpenSSL build issues (thanks to rustls!)
- ✅ Libraries ready to integrate with Flutter
- ✅ Full NAT traversal capabilities

Time to build the Flutter app! 🚀
