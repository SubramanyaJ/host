# Pineapple 🍍

A quantum-safe peer-to-peer secure messaging system with NAT traversal, implemented in Rust.

## Features

- **Post-Quantum Cryptography**: ML-KEM-1024 for key encapsulation
- **Perfect Forward Secrecy**: Double Ratchet protocol
- **NAT Traversal**: Direct peer-to-peer connections without relay servers
  - STUN for NAT discovery
  - UDP hole punching
  - TCP simultaneous open
  - WebSocket signalling server
- **Cross-Platform**: Native support for Linux, macOS, Windows, Android, and iOS
- **Zero Native Dependencies**: Pure Rust TLS implementation (rustls)
- **FFI Bindings**: C-compatible API for Flutter integration

## Architecture

```
┌─────────────────────────────────────┐
│        Flutter Application          │
└──────────────┬──────────────────────┘
               │ FFI
┌──────────────▼──────────────────────┐
│      Pineapple Rust Library         │
├─────────────────────────────────────┤
│  • PQXDH Handshake (ML-KEM-1024)    │
│  • Double Ratchet Encryption        │
│  • NAT Traversal (STUN + Punching)  │
│  • TLS WebSocket Signalling         │
└─────────────────────────────────────┘
```

## Quick Start

### 1. Build the Desktop Application

```bash
cargo build --release
```

### 2. Setup Required Servers

You need two servers running:

1. **Signalling Server** (TLS WebSocket on port 8443)
2. **STUN Server** (UDP on port 3478)

See the `signalling_server` and `stun_server` directories for complete server implementations.

### 3. Run with NAT Traversal (Recommended)

**Configure environment variables:**

```bash
# On Linux/macOS
export SIGNALLING_URL="wss://your-server.com:8443"
export STUN_SERVER="your-server.com:3478"
export LOCAL_FINGERPRINT="alice"  # Unique identifier for this peer

# On Windows PowerShell
$env:SIGNALLING_URL="wss://your-server.com:8443"
$env:STUN_SERVER="your-server.com:3478"
$env:LOCAL_FINGERPRINT="alice"
```

**Run the application:**

Peer 1 (Alice):
```bash
./target/release/pineapple nat bob
```

Peer 2 (Bob):
```bash
./target/release/pineapple nat alice
```

The peers will automatically:
1. Connect to the signalling server
2. Discover their external IPs via STUN
3. Exchange connection info through signalling
4. Perform UDP hole punching
5. Establish TCP connection via simultaneous open
6. Hand off to encrypted session

### 4. Legacy Direct Connection Mode (No NAT Traversal)

If you have direct network access (no NAT), you can use the legacy modes:

**Listener (Alice):**
```bash
./target/release/pineapple listen 8080
```

**Connector (Bob):**
```bash
./target/release/pineapple connect 192.168.1.100:8080
```

⚠️ **Note:** Direct mode does NOT work behind NAT. Use NAT traversal mode for real-world scenarios.

## NAT Traversal Pipeline

The complete NAT traversal sequence:

1. **Connect to Signalling Server**: TLS WebSocket connection established
2. **Register**: Send identity fingerprint to signalling server
3. **STUN Discovery**: Query STUN server for external IP:port
4. **Exchange Offers**: Both peers exchange their endpoints via signalling
5. **UDP Hole Punching**: Send signed probe packets to peer's endpoints
6. **TCP Exchange**: Exchange TCP ports over established UDP connection
7. **TCP Simultaneous Open**: Both peers simultaneously connect TCP sockets
8. **Handoff**: Close UDP and signalling, hand TCP stream to PQXDH/ratchet

See [PORT.md](PORT.md) for detailed state machine, message schemas, and timing specifications.

## Mobile Builds

### Android Build

```bash
export NDK_HOME=$HOME/Android/Sdk/ndk/26.1.10909125
./scripts/build-android.sh
```

See [ANDROID_BUILD.md](ANDROID_BUILD.md) for detailed instructions.

### iOS Build

```bash
./scripts/build-ios-xcframework.sh
```

The XCFramework will be created at `target/ios/PineappleFFI.xcframework`.

## Configuration

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `SIGNALLING_URL` | TLS WebSocket signalling server URL | `wss://your-server.com:8443` |
| `STUN_SERVER` | STUN server address (host:port) | `your-server.com:3478` |
| `LOCAL_FINGERPRINT` | Unique identifier for this peer | Random ID |

### Server Setup

You must deploy the signalling and STUN servers on a machine with a **public static IP**.

**Quick server deployment:**

```bash
# Clone the server implementations
cd E:\host\signalling_server
docker-compose up -d

cd E:\host\stun_server
docker-compose up -d
```

See server-specific README files for detailed deployment instructions:
- [signalling_server/README.md](../signalling_server/README.md)
- [stun_server/README.md](../stun_server/README.md)

## TLS Implementation

This project uses **rustls**, a modern TLS library written in Rust:

- ✅ No OpenSSL dependency
- ✅ No native C compilation required  
- ✅ Perfect cross-platform support
- ✅ Memory safe by design
- ✅ Faster builds
- ✅ Smaller binaries

See [RUSTLS_MIGRATION.md](RUSTLS_MIGRATION.md) for the full rationale.

## Documentation

- **[PORT.md](PORT.md)**: Complete API reference, FFI bindings, NAT traversal pipeline
- **[ANDROID_BUILD.md](ANDROID_BUILD.md)**: Android build guide and troubleshooting
- **[RUSTLS_MIGRATION.md](RUSTLS_MIGRATION.md)**: Why we use rustls instead of OpenSSL
- **[BUILD_QUICK_REF.md](BUILD_QUICK_REF.md)**: Quick build reference

## Security

- **Post-quantum security**: ML-KEM-1024 (formerly Kyber-1024)
- **Authentication**: Ed25519 signatures
- **Key agreement**: X25519 ECDH
- **Encryption**: AES-256-GCM
- **KDF**: HKDF-SHA3-256
- **Hashing**: BLAKE3

## Requirements

### Desktop
- Rust 1.75.0 or later
- No other dependencies (pure Rust with rustls)

### Mobile
- Android NDK r25c+ (for Android builds)
- Xcode 14.0+ (for iOS builds)

### Servers (Deployment)
- Docker and Docker Compose (recommended)
- Public static IP address
- Open ports: 8443 (signalling), 3478 (STUN)

## Project Structure

```
pineapple/
├── src/
│   ├── pqxdh/          # Post-quantum handshake
│   ├── ratchet/        # Double ratchet encryption
│   ├── nat_traversal/  # NAT traversal implementation
│   │   ├── mod.rs            # Main NAT traversal state machine
│   │   ├── signalling.rs     # TLS WebSocket signalling client
│   │   ├── stun.rs           # STUN client implementation
│   │   ├── hole_punching.rs  # UDP hole punching
│   │   ├── tcp_connect.rs    # TCP simultaneous open
│   │   └── types.rs          # Core types and config
│   ├── ffi/            # C FFI bindings
│   ├── session.rs      # Session management
│   ├── network.rs      # Network utilities
│   ├── messages.rs     # Message serialization
│   ├── lib.rs          # Library entry point
│   └── main.rs         # CLI application
├── scripts/
│   ├── build-android.sh
│   ├── build-android.bat
│   └── build-ios-xcframework.sh
├── android/
│   └── jniLibs/        # Android native libraries
└── .cargo/
    └── config.toml     # Cross-compilation configuration
```

## Testing

### Unit Tests

```bash
cargo test
```

### Integration Tests (Requires Running Servers)

**Start the servers first:**

```bash
cd E:\host\signalling_server
docker-compose up -d

cd E:\host\stun_server
docker-compose up -d
```

**Configure test environment:**

```bash
export SIGNALLING_URL="wss://localhost:8443"
export STUN_SERVER="localhost:3478"
```

**Run integration tests:**

```bash
cargo test --test integration
```

### Manual End-to-End Test

**Terminal 1 (Peer Alice):**
```bash
export SIGNALLING_URL="wss://your-server.com:8443"
export STUN_SERVER="your-server.com:3478"
export LOCAL_FINGERPRINT="alice"
./target/release/pineapple nat bob
```

**Terminal 2 (Peer Bob):**
```bash
export SIGNALLING_URL="wss://your-server.com:8443"
export STUN_SERVER="your-server.com:3478"
export LOCAL_FINGERPRINT="bob"
./target/release/pineapple nat alice
```

Both peers should successfully connect and you can exchange messages!

## Performance

- **Memory**: ~2MB per NAT traversal instance
- **Ratchet overhead**: 56 bytes per message
- **NAT traversal time**: ~5-30 seconds (typical)
- **CPU usage**: <1% during traversal, <0.1% during messaging
- **Binary size**: ~3MB (release build, stripped)

## Troubleshooting

### NAT Traversal Fails

1. **Check server connectivity:**
   ```bash
   # Test signalling server
   curl -k https://your-server.com:8443
   
   # Test STUN server (requires stun tool)
   stunclient your-server.com 3478
   ```

2. **Check firewall rules:**
   - Ensure signalling server port 8443 is open (TCP)
   - Ensure STUN server port 3478 is open (UDP)

3. **Check environment variables:**
   ```bash
   echo $SIGNALLING_URL
   echo $STUN_SERVER
   echo $LOCAL_FINGERPRINT
   ```

4. **Enable debug logging:**
   ```bash
   RUST_LOG=debug ./target/release/pineapple nat <peer>
   ```

### Build Errors

**OpenSSL-related errors:**

This project uses rustls, not OpenSSL. If you see OpenSSL errors, clean and rebuild:

```bash
cargo clean
cargo build --release
```

**Android NDK not found:**

```bash
export NDK_HOME=$HOME/Android/Sdk/ndk/26.1.10909125
# Or wherever your NDK is installed
```

See [ANDROID_BUILD.md](ANDROID_BUILD.md) for more details.

### Connection Issues in Direct Mode

Direct mode (`listen`/`connect`) does NOT work behind NAT. Always use NAT traversal mode (`nat`) for real-world scenarios.

## Contributing

Contributions welcome! Please ensure:
- All tests pass: `cargo test`
- Code is formatted: `cargo fmt`
- No clippy warnings: `cargo clippy`

## License

See [LICENSE](LICENSE) file.

## Author

Subramanya J <subramanyajaradhya@gmail.com>

## Repository

https://github.com/SubramanyaJ/pineapple

## Related Projects

- **Signalling Server**: `E:\host\signalling_server`
- **STUN Server**: `E:\host\stun_server`
- **Flutter App**: `E:\host\pineapple_app`
