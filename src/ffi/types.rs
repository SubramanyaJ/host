/**
 * ffi/types.rs
 * 
 * Common FFI types and structures
 */

use std::os::raw::c_char;

/// Opaque handle for NatTraversal instance
#[repr(C)]
pub struct NatTraversalHandle {
    _private: [u8; 0],
}

/// Opaque handle for Session instance
#[repr(C)]
pub struct SessionHandle {
    _private: [u8; 0],
}

/// Connection state enum (matches ConnectionState)
#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ConnectionState {
    Idle = 0,
    ConnectingSignalling = 1,
    Registering = 2,
    StunDiscovery = 3,
    SendingOffer = 4,
    WaitingForOffer = 5,
    UdpHolePunching = 6,
    TcpConnecting = 7,
    Connected = 8,
    Failed = 9,
}

/// FFI-safe buffer structure
#[repr(C)]
pub struct ByteBuffer {
    pub data: *mut u8,
    pub len: usize,
    pub capacity: usize,
}

impl ByteBuffer {
    /// Create from Vec<u8>
    pub fn from_vec(mut vec: Vec<u8>) -> Self {
        let data = vec.as_mut_ptr();
        let len = vec.len();
        let capacity = vec.capacity();
        std::mem::forget(vec);
        Self { data, len, capacity }
    }

    /// Convert to Vec<u8>
    pub unsafe fn into_vec(self) -> Vec<u8> {
        Vec::from_raw_parts(self.data, self.len, self.capacity)
    }

    /// Create empty buffer
    pub fn empty() -> Self {
        Self {
            data: std::ptr::null_mut(),
            len: 0,
            capacity: 0,
        }
    }
}

/// Free a ByteBuffer
#[no_mangle]
pub extern "C" fn pineapple_free_buffer(buffer: ByteBuffer) {
    if !buffer.data.is_null() {
        unsafe {
            let _ = Vec::from_raw_parts(buffer.data, buffer.len, buffer.capacity);
        }
    }
}

/// Configuration for NAT traversal
#[repr(C)]
pub struct NatTraversalConfig {
    pub signalling_url: *const c_char,
    pub stun_server_addr: *const c_char,
    pub local_fingerprint: *const c_char,
    pub signing_key_bytes: *const u8,
    pub tcp_port: u16,
}

/// Callback type for connection state changes
pub type StateCallback = extern "C" fn(state: ConnectionState, user_data: *mut std::ffi::c_void);

/// Callback type for log messages
pub type LogCallback = extern "C" fn(level: i32, message: *const c_char, user_data: *mut std::ffi::c_void);
