# Pineapple Rust Library - Port Documentation

## Overview

This document describes the complete API, integration points, and build instructions for the Pineapple Rust library with NAT traversal capabilities.

## Table of Contents

1. [Architecture](#architecture)
2. [FFI API Reference](#ffi-api-reference)
3. [NAT Traversal State Machine](#nat-traversal-state-machine)
4. [Signalling Server Integration](#signalling-server-integration)
5. [STUN Server Integration](#stun-server-integration)
6. [Message Schemas](#message-schemas)
7. [Build Instructions](#build-instructions)
8. [Environment Variables](#environment-variables)
9. [Integration Guide](#integration-guide)

---

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                   Flutter/Dart App                      │
│                    (FFI Bindings)                       │
└────────────────────────┬────────────────────────────────┘
                         │ C-ABI FFI
                         ▼
┌─────────────────────────────────────────────────────────┐
│              Pineapple Rust Library                     │
├─────────────────────────────────────────────────────────┤
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │ NAT Traversal│  │   Session    │  │   Ratchet    │  │
│  │   Module     │  │   (PQXDH)    │  │ (Encryption) │  │
│  └──────┬───────┘  └──────────────┘  └──────────────┘  │
│         │                                                │
│  ┌──────┴───────┬───────────┬────────────┐             │
│  │ Signalling   │   STUN    │    UDP     │             │
│  │   Client     │  Client   │ Punching   │             │
│  └──────────────┴───────────┴────────────┘             │
└─────────────────────────────────────────────────────────┘
           │              │              │
           ▼              ▼              ▼
    ┌───────────┐  ┌───────────┐  ┌───────────┐
    │Signalling │  │   STUN    │  │ Peer UDP  │
    │  Server   │  │  Server   │  │  Socket   │
    └───────────┘  └───────────┘  └───────────┘
```

---

## FFI API Reference

### Initialization

#### `pineapple_init() -> i32`
Initialize the library. Must be called once before any other functions.

**Returns:** `0` on success, `-1` on error

**Example:**
```c
int result = pineapple_init();
if (result != 0) {
    // Handle error
}
```

#### `pineapple_version() -> *const c_char`
Get library version string.

**Returns:** Null-terminated string (must be freed with `pineapple_free_string`)

### NAT Traversal Functions

#### `pineapple_nat_create(config: NatTraversalConfig) -> *mut NatTraversalHandle`
Create a new NAT traversal instance.

**Parameters:**
- `config.signalling_url`: WebSocket URL (e.g., "wss://signal.example.com:8443")
- `config.stun_server_addr`: STUN server address (e.g., "stun.example.com:3478")
- `config.local_fingerprint`: Local identity fingerprint (hex string)
- `config.signing_key_bytes`: Pointer to 32-byte Ed25519 signing key
- `config.tcp_port`: Local TCP port (0 for random)

**Returns:** Opaque handle or NULL on error

**Example:**
```c
NatTraversalConfig config = {
    .signalling_url = "wss://signal.example.com:8443",
    .stun_server_addr = "stun.example.com:3478",
    .local_fingerprint = "abc123...",
    .signing_key_bytes = key_buffer,
    .tcp_port = 0
};
NatTraversalHandle* handle = pineapple_nat_create(config);
```

#### `pineapple_nat_connect(handle, peer_fingerprint) -> i32`
Connect to peer using NAT traversal pipeline.

**Parameters:**
- `handle`: NAT traversal handle
- `peer_fingerprint`: Peer's identity fingerprint

**Returns:** `0` on success, `-1` on error

**Note:** This is an async function. For blocking operation, use the async runtime wrapper in your language binding.

#### `pineapple_nat_get_state(handle) -> ConnectionState`
Get current connection state.

**Returns:** ConnectionState enum value

**Connection States:**
```c
enum ConnectionState {
    Idle = 0,
    ConnectingSignalling = 1,
    Registering = 2,
    StunDiscovery = 3,
    SendingOffer = 4,
    WaitingForOffer = 5,
    UdpHolePunching = 6,
    TcpConnecting = 7,
    Connected = 8,
    Failed = 9
}
```

#### `pineapple_nat_free(handle)`
Free NAT traversal instance.

### Session Functions

#### `pineapple_session_send(handle, message_data, message_len) -> ByteBuffer`
Send encrypted message.

**Parameters:**
- `handle`: Session handle
- `message_data`: Pointer to message bytes
- `message_len`: Length of message

**Returns:** ByteBuffer containing encrypted message (must be freed with `pineapple_free_buffer`)

#### `pineapple_session_receive(handle, message_data, message_len) -> ByteBuffer`
Receive and decrypt message.

**Parameters:**
- `handle`: Session handle
- `message_data`: Pointer to encrypted message bytes
- `message_len`: Length of message

**Returns:** ByteBuffer containing decrypted plaintext

### Memory Management

#### `pineapple_free_string(ptr: *mut c_char)`
Free a string allocated by the library.

#### `pineapple_free_buffer(buffer: ByteBuffer)`
Free a ByteBuffer allocated by the library.

### Error Handling

#### `pineapple_last_error() -> *const c_char`
Get last error message.

**Returns:** Error string or NULL if no error

#### `pineapple_clear_error()`
Clear the last error.

---

## NAT Traversal State Machine

### Complete Pipeline Sequence

```
1. IDLE
   ↓
2. CONNECTING_SIGNALLING
   • Open TLS WebSocket to signalling server
   • Timeout: 10 seconds
   • Retry: 3 attempts with exponential backoff
   ↓
3. REGISTERING
   • Send: { type: "register", fingerprint: "..." }
   • Wait for: { type: "register_ack", success: true }
   • Timeout: 5 seconds
   ↓
4. STUN_DISCOVERY
   • Bind UDP socket on port 0 (random)
   • Send STUN Binding Request to STUN server
   • Receive STUN Binding Response with XOR-MAPPED-ADDRESS
   • Extract: external_ip, external_port
   • Timeout: 5 seconds per attempt
   • Retry: 3 attempts
   ↓
5. SENDING_OFFER
   • Generate nonce = random_u64()
   • Send: {
       type: "offer",
       target_fingerprint: "peer_id",
       external_ip: "1.2.3.4",
       external_port: 54321,
       local_ip: "192.168.1.100",
       local_port: 54321,
       nonce: 12345,
       fingerprint: "my_id"
     }
   • Wait for: { type: "forward_offer", from_fingerprint: "peer_id", ... }
   • Timeout: 60 seconds
   ↓
6. UDP_HOLE_PUNCHING
   • Construct ProbePacket:
     - nonce: random_u64()
     - tcp_port: local_tcp_port_to_use
     - signature: Ed25519 signature over (nonce || tcp_port)
   • Send UDP probes to [peer_external_addr, peer_local_addr] every 200ms
   • Listen for peer's probe packet
   • Validate signature using peer's Ed25519 public key
   • Extract peer's TCP port
   • Timeout: 30 seconds
   ↓
7. TCP_CONNECTING
   • Bind TCP socket to local_tcp_port
   • Set SO_REUSEADDR and SO_REUSEPORT
   • Perform TCP simultaneous open to peer_external_ip:peer_tcp_port
   • Send SYN packets repeatedly (100ms interval)
   • Accept incoming SYN from peer
   • Timeout: 10 seconds
   ↓
8. CONNECTED
   • Close UDP socket
   • Close signalling WebSocket
   • Return connected TCP stream
   • TCP stream is now ready for pineapple Session handshake
```

### Timing and Retry Policies

| Phase | Initial Timeout | Retry Count | Retry Backoff | Total Max Time |
|-------|----------------|-------------|---------------|----------------|
| Signalling Connect | 10s | 3 | Exponential (2x) | ~30s |
| Registration | 5s | 2 | Fixed | 10s |
| STUN Discovery | 5s | 3 | Fixed | 15s |
| Offer Exchange | 60s | 1 | None | 60s |
| UDP Hole Punch | 30s | 1 | None | 30s |
| TCP Simultaneous Open | 10s | 1 | None | 10s |

**Total worst-case time:** ~155 seconds

---

## Signalling Server Integration

### Server Endpoint

**URL Format:** `wss://<host>:<port>`

**Example:** `wss://signal.example.com:8443`

### Message Schemas

All messages are JSON over WebSocket Text frames.

#### 1. Register

**Client → Server:**
```json
{
  "type": "register",
  "fingerprint": "ed25519_public_key_hex_64chars"
}
```

**Server → Client:**
```json
{
  "type": "register_ack",
  "success": true,
  "message": "Registered successfully"
}
```

**Error Response:**
```json
{
  "type": "register_ack",
  "success": false,
  "message": "Fingerprint already registered"
}
```

#### 2. Offer

**Client A → Server:**
```json
{
  "type": "offer",
  "target_fingerprint": "peer_ed25519_public_key_hex",
  "external_ip": "203.0.113.45",
  "external_port": 54321,
  "local_ip": "192.168.1.100",
  "local_port": 54321,
  "nonce": 9876543210,
  "fingerprint": "my_ed25519_public_key_hex"
}
```

**Server → Client B (forwarded):**
```json
{
  "type": "forward_offer",
  "from_fingerprint": "sender_ed25519_public_key_hex",
  "external_ip": "203.0.113.45",
  "external_port": 54321,
  "local_ip": "192.168.1.100",
  "local_port": 54321,
  "nonce": 9876543210
}
```

#### 3. Keepalive

**Client ↔ Server:**
```json
{
  "type": "keepalive"
}
```

**Frequency:** Every 30 seconds

#### 4. Error

**Server → Client:**
```json
{
  "type": "error",
  "message": "Target peer not registered"
}
```

### Connection Requirements

- **Protocol:** WebSocket over TLS 1.2+
- **Certificate:** Valid TLS certificate (no self-signed in production)
- **Heartbeat:** Client must send keepalive every 30s or server may disconnect
- **Max Message Size:** 4096 bytes
- **Timeout:** Idle connections closed after 60s without activity

### TLS Implementation

**This library uses `rustls` for TLS**, a pure Rust TLS implementation:

- **No native dependencies:** 100% Rust code, no OpenSSL required
- **Cross-platform:** Builds identically on all platforms (Linux, macOS, Windows, Android, iOS)
- **Memory safe:** Written in Rust with all its safety guarantees
- **Standards compliant:** TLS 1.2 and TLS 1.3 support
- **Certificate validation:** Uses system root certificates via `webpki-roots`

**Why not OpenSSL?**
- OpenSSL requires native C compilation and linking
- Cross-compilation for Android/iOS is complex and error-prone
- rustls eliminates all build-time dependencies
- Smaller binary size and better performance in many cases

**Crate configuration:**
```toml
tokio-tungstenite = { version = "0.21", features = ["rustls-tls-native-roots"] }
rustls = "0.21"
webpki-roots = "0.25"
```

---

## STUN Server Integration

### Server Address

**Format:** `<host>:<port>`

**Example:** `stun.example.com:3478`

**Protocol:** UDP

### STUN Message Format

#### Binding Request (Client → Server)

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|0 0|  Message Type (0x0001)    |       Message Length          |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                         Magic Cookie (0x2112A442)             |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                                                               |
|                   Transaction ID (96 bits)                    |
|                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

**Message Type:** `0x0001` (Binding Request)

**Message Length:** `0` (no attributes in request)

**Magic Cookie:** `0x2112A442`

**Transaction ID:** 12 random bytes

#### Binding Response (Server → Client)

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|0 0|  Message Type (0x0101)    |       Message Length          |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                         Magic Cookie (0x2112A442)             |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                                                               |
|                   Transaction ID (96 bits)                    |
|                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|         Attribute Type (XOR-MAPPED-ADDRESS = 0x0020)          |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|         Attribute Length      | Family (0x01) |  XOR-Port     |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                        XOR-IP Address                         |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

**Message Type:** `0x0101` (Binding Response - Success)

**XOR-MAPPED-ADDRESS Attribute:**
- **Type:** `0x0020`
- **Length:** 8 bytes (for IPv4)
- **Family:** `0x01` (IPv4) or `0x02` (IPv6)
- **XOR-Port:** `port ^ (magic_cookie >> 16)`
- **XOR-IP:** `ip_address ^ magic_cookie`

### UDP Probe Packet Format

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                    Magic ("PNPL" = 0x504E504C)                |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                                                               |
|                         Nonce (64 bits)                       |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|          TCP Port             |                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+                               +
|                                                               |
|                    Ed25519 Signature (64 bytes)               |
|                                                               |
|                                                               |
+                                               +-+-+-+-+-+-+-+-+
|                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

**Total Length:** 78 bytes

**Signature Covers:** `"PINEAPPLE_PROBE" || nonce || tcp_port`

**Verification:** Use peer's Ed25519 public key (obtained from signalling exchange or pre-shared)

---

## Message Schemas

### Pineapple Ratchet Message (over TCP)

After NAT traversal completes, the TCP stream carries length-prefixed pineapple ratchet messages:

```
[4 bytes: length] [length bytes: message data]
```

**Message Data Structure:**
```
[32 bytes: X25519 public key]
[8 bytes: counter (big-endian u64)]
[12 bytes: nonce]
[4 bytes: ciphertext length]
[ciphertext_length bytes: encrypted payload]
```

**Total overhead:** 56 bytes + ciphertext

---

## Build Instructions

### Prerequisites

- **Rust:** 1.75.0 or later
- **Cargo:** Latest stable
- **Android NDK:** r25c or later (for Android)
- **Xcode:** 14.0+ (for iOS)
- **cmake:** 3.20+ (for some dependencies)

### Desktop Build (Linux/macOS/Windows)

```bash
# Clone the repository
git clone https://github.com/SubramanyaJ/pineapple.git
cd pineapple

# Build library
cargo build --release

# The library will be at:
# - target/release/libpineapple.so (Linux)
# - target/release/libpineapple.dylib (macOS)
# - target/release/pineapple.dll (Windows)
```

### Android Build

```bash
# Install rust android targets
rustup target add aarch64-linux-android
rustup target add armv7-linux-androideabi
rustup target add x86_64-linux-android
rustup target add i686-linux-android

# Set NDK path
export NDK_HOME=$HOME/Android/Sdk/ndk/25.2.9519653

# Create cargo config
mkdir -p .cargo
cat > .cargo/config.toml << EOF
[target.aarch64-linux-android]
ar = "$NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-ar"
linker = "$NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android30-clang"

[target.armv7-linux-androideabi]
ar = "$NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-ar"
linker = "$NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin/armv7a-linux-androideabi30-clang"

[target.x86_64-linux-android]
ar = "$NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-ar"
linker = "$NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin/x86_64-linux-android30-clang"

[target.i686-linux-android]
ar = "$NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-ar"
linker = "$NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin/i686-linux-android30-clang"
EOF

# Build for all Android architectures
cargo build --release --target aarch64-linux-android
cargo build --release --target armv7-linux-androideabi
cargo build --release --target x86_64-linux-android
cargo build --release --target i686-linux-android

# Libraries will be at:
# target/aarch64-linux-android/release/libpineapple.so (arm64-v8a)
# target/armv7-linux-androideabi/release/libpineapple.so (armeabi-v7a)
# target/x86_64-linux-android/release/libpineapple.so (x86_64)
# target/i686-linux-android/release/libpineapple.so (x86)
```

Use the provided build script:
```bash
./scripts/build-android.sh
```

### iOS Build

```bash
# Install rust iOS targets
rustup target add aarch64-apple-ios
rustup target add x86_64-apple-ios
rustup target add aarch64-apple-ios-sim

# Install cargo-lipo
cargo install cargo-lipo

# Build universal library
cargo lipo --release

# The library will be at:
# target/universal/release/libpineapple.a

# Create XCFramework
./scripts/build-ios-xcframework.sh
```

### Generate C Header

```bash
# Install cbindgen
cargo install cbindgen

# Generate header
cbindgen --config cbindgen.toml --crate pineapple --output pineapple.h
```

---

## Environment Variables

### Required

- `SIGNALLING_SERVER_URL`: WebSocket URL of signalling server (e.g., `wss://signal.example.com:8443`)
- `STUN_SERVER_ADDR`: STUN server address (e.g., `stun.example.com:3478`)

### Optional

- `RUST_LOG`: Log level (trace, debug, info, warn, error)
- `PINEAPPLE_TIMEOUT_SIGNALLING`: Signalling connection timeout in seconds (default: 10)
- `PINEAPPLE_TIMEOUT_STUN`: STUN query timeout in seconds (default: 5)
- `PINEAPPLE_TIMEOUT_UDP_PUNCH`: UDP hole punching timeout in seconds (default: 30)
- `PINEAPPLE_TIMEOUT_TCP`: TCP simultaneous open timeout in seconds (default: 10)

---

## Integration Guide

### For Flutter Apps

1. **Add Rust library to Flutter project:**

```yaml
# pubspec.yaml
dependencies:
  ffi: ^2.0.0

ffigen:
  output: 'lib/pineapple_bindings.dart'
  headers:
    entry-points:
      - 'rust/pineapple.h'
```

2. **Load library in Dart:**

```dart
import 'dart:ffi' as ffi;
import 'package:ffi/ffi.dart';
import 'pineapple_bindings.dart';

final dylib = ffi.DynamicLibrary.open('libpineapple.so');
final pineapple = PineappleBindings(dylib);

// Initialize
pineapple.pineapple_init();
```

3. **Create NAT traversal instance:**

```dart
final config = ffi.Struct.create<NatTraversalConfig>();
config.signalling_url = 'wss://signal.example.com:8443'.toNativeUtf8();
config.stun_server_addr = 'stun.example.com:3478'.toNativeUtf8();
config.local_fingerprint = myFingerprint.toNativeUtf8();
config.signing_key_bytes = signingKeyBytes.allocatePointer();
config.tcp_port = 0;

final handle = pineapple.pineapple_nat_create(config);
```

4. **Connect to peer:**

```dart
final result = pineapple.pineapple_nat_connect(
  handle,
  peerFingerprint.toNativeUtf8(),
);

if (result != 0) {
  final error = pineapple.pineapple_last_error();
  print('Connection failed: ${error.toDartString()}');
}
```

5. **Monitor connection state:**

```dart
Timer.periodic(Duration(milliseconds: 100), (timer) {
  final state = pineapple.pineapple_nat_get_state(handle);
  
  switch (state) {
    case ConnectionState.Connected:
      print('Connected!');
      timer.cancel();
      break;
    case ConnectionState.Failed:
      print('Connection failed');
      timer.cancel();
      break;
    default:
      print('State: $state');
  }
});
```

### Android-Specific Setup

1. **Copy libraries to jniLibs:**

```
android/app/src/main/jniLibs/
├── arm64-v8a/
│   └── libpineapple.so
├── armeabi-v7a/
│   └── libpineapple.so
├── x86_64/
│   └── libpineapple.so
└── x86/
    └── libpineapple.so
```

2. **Update AndroidManifest.xml:**

```xml
<manifest>
    <uses-permission android:name="android.permission.INTERNET" />
    <uses-permission android:name="android.permission.ACCESS_NETWORK_STATE" />
</manifest>
```

### iOS-Specific Setup

1. **Add XCFramework to Xcode project:**
   - Drag `Pineapple.xcframework` into Xcode project
   - Ensure "Copy items if needed" is checked
   - Add to "Frameworks, Libraries, and Embedded Content"
   - Set to "Embed & Sign"

2. **Update Info.plist:**

```xml
<key>NSAppTransportSecurity</key>
<dict>
    <key>NSAllowsArbitraryLoads</key>
    <false/>
    <key>NSExceptionDomains</key>
    <dict>
        <key>signal.example.com</key>
        <dict>
            <key>NSExceptionAllowsInsecureHTTPLoads</key>
            <false/>
            <key>NSIncludesSubdomains</key>
            <true/>
        </dict>
    </dict>
</dict>
```

---

## Testing

### Unit Tests

```bash
cargo test
```

### Integration Tests

```bash
# Start test servers (in separate terminals)
cd ../signalling_server && cargo run
cd ../stun_server && docker-compose up

# Run integration tests
cargo test --test integration -- --test-threads=1
```

---

## Troubleshooting

### Common Issues

1. **"Connection timeout" during signalling:**
   - Verify signalling server is reachable
   - Check TLS certificate is valid
   - Ensure firewall allows outbound WebSocket connections

2. **"STUN query failed":**
   - Verify STUN server address is correct
   - Check UDP port 3478 is not blocked
   - Try alternative STUN servers

3. **"UDP hole punching timeout":**
   - Symmetric NAT may prevent hole punching
   - Try different network (mobile data vs WiFi)
   - Increase timeout in environment variables

4. **"TCP simultaneous open failed":**
   - NAT may have closed hole before TCP attempt
   - Reduce delay between UDP success and TCP attempt
   - Try TURN relay as fallback (not currently implemented)

### Debug Logging

Enable detailed logging:

```bash
export RUST_LOG=pineapple=trace
cargo run
```

---

## Performance Characteristics

- **Memory:** ~2MB per NAT traversal instance
- **Bandwidth:** 
  - STUN query: ~100 bytes
  - Signalling: ~500 bytes per offer
  - UDP probes: 78 bytes every 200ms
  - Ratchet overhead: 56 bytes per message
- **CPU:** <1% during traversal, <0.1% during messaging
- **Battery Impact:** Low (async I/O, minimal polling)

---

## Security Considerations

1. **All signalling must use TLS** (wss://)
2. **UDP probes must be signed** with Ed25519
3. **Peer identity verified** through fingerprint exchange
4. **No plaintext secrets** in logs or memory dumps
5. **Forward secrecy** via Double Ratchet
6. **Post-quantum security** via ML-KEM-1024

---

## Version History

- **1.0.0** (2025-01-XX): Initial release with NAT traversal
  - TLS WebSocket signalling
  - STUN discovery
  - UDP hole punching
  - TCP simultaneous open
  - FFI bindings for Flutter
  - Android and iOS support

---

## Support

For issues and questions:
- **GitHub:** https://github.com/SubramanyaJ/pineapple/issues
- **Email:** subramanyajaradhya@gmail.com

## License

See LICENSE file in repository root.
