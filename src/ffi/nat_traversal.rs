/**
 * ffi/nat_traversal.rs
 * 
 * FFI bindings for NAT traversal functionality
 */

use super::*;
use crate::nat_traversal::{NatTraversal as RustNatTraversal, NatTraversalConfig as RustConfig};
use std::os::raw::c_char;
use std::ffi::CString;

/// Create a new NAT traversal instance
#[no_mangle]
pub extern "C" fn pineapple_nat_create(config: NatTraversalConfig) -> *mut NatTraversalHandle {
    let signalling_url = match c_str_to_rust(config.signalling_url) {
        Some(s) => s,
        None => {
            set_last_error("Invalid signalling URL");
            return std::ptr::null_mut();
        }
    };

    let stun_server_addr = match c_str_to_rust(config.stun_server_addr) {
        Some(s) => match s.parse() {
            Ok(addr) => addr,
            Err(e) => {
                set_last_error(&format!("Invalid STUN server address: {}", e));
                return std::ptr::null_mut();
            }
        },
        None => {
            set_last_error("Invalid STUN server address");
            return std::ptr::null_mut();
        }
    };

    let local_fingerprint = match c_str_to_rust(config.local_fingerprint) {
        Some(s) => s,
        None => {
            set_last_error("Invalid local fingerprint");
            return std::ptr::null_mut();
        }
    };

    if config.signing_key_bytes.is_null() {
        set_last_error("Null signing key");
        return std::ptr::null_mut();
    }

    let signing_key = unsafe {
        let key_slice = std::slice::from_raw_parts(config.signing_key_bytes, 32);
        match ed25519_dalek::SigningKey::try_from(key_slice) {
            Ok(key) => key,
            Err(e) => {
                set_last_error(&format!("Invalid signing key: {}", e));
                return std::ptr::null_mut();
            }
        }
    };

    let rust_config = RustConfig {
        signalling_url,
        stun_server_addr,
        local_fingerprint,
        signing_key,
        tcp_port: config.tcp_port,
    };

    let nat = Box::new(RustNatTraversal::new(rust_config));
    Box::into_raw(nat) as *mut NatTraversalHandle
}

/// Connect to peer using NAT traversal
/// Returns 0 on success, -1 on error
/// The resulting TCP stream is stored internally and can be retrieved with pineapple_nat_get_tcp_fd
#[no_mangle]
pub extern "C" fn pineapple_nat_connect(
    handle: *mut NatTraversalHandle,
    peer_fingerprint: *const c_char,
) -> i32 {
    if handle.is_null() {
        set_last_error("Null NAT traversal handle");
        return -1;
    }

    let peer_fp = match c_str_to_rust(peer_fingerprint) {
        Some(s) => s,
        None => {
            set_last_error("Invalid peer fingerprint");
            return -1;
        }
    };

    let nat = unsafe { &mut *(handle as *mut RustNatTraversal) };

    // This requires async runtime - for now, return error
    set_last_error("Async runtime required - use pineapple_nat_connect_blocking");
    -1
}

/// Get current connection state
#[no_mangle]
pub extern "C" fn pineapple_nat_get_state(handle: *const NatTraversalHandle) -> ConnectionState {
    if handle.is_null() {
        return ConnectionState::Failed;
    }

    let nat = unsafe { &*(handle as *const RustNatTraversal) };
    
    match nat.state() {
        crate::nat_traversal::ConnectionState::Idle => ConnectionState::Idle,
        crate::nat_traversal::ConnectionState::ConnectingSignalling => ConnectionState::ConnectingSignalling,
        crate::nat_traversal::ConnectionState::Registering => ConnectionState::Registering,
        crate::nat_traversal::ConnectionState::StunDiscovery => ConnectionState::StunDiscovery,
        crate::nat_traversal::ConnectionState::SendingOffer => ConnectionState::SendingOffer,
        crate::nat_traversal::ConnectionState::WaitingForOffer => ConnectionState::WaitingForOffer,
        crate::nat_traversal::ConnectionState::UdpHolePunching => ConnectionState::UdpHolePunching,
        crate::nat_traversal::ConnectionState::TcpConnecting => ConnectionState::TcpConnecting,
        crate::nat_traversal::ConnectionState::Connected => ConnectionState::Connected,
        crate::nat_traversal::ConnectionState::Failed(_) => ConnectionState::Failed,
    }
}

/// Free NAT traversal instance
#[no_mangle]
pub extern "C" fn pineapple_nat_free(handle: *mut NatTraversalHandle) {
    if !handle.is_null() {
        unsafe {
            let _ = Box::from_raw(handle as *mut RustNatTraversal);
        }
    }
}

/// Get state name as string
#[no_mangle]
pub extern "C" fn pineapple_state_to_string(state: ConnectionState) -> *const c_char {
    let s = match state {
        ConnectionState::Idle => "Idle",
        ConnectionState::ConnectingSignalling => "Connecting to signalling",
        ConnectionState::Registering => "Registering",
        ConnectionState::StunDiscovery => "STUN discovery",
        ConnectionState::SendingOffer => "Sending offer",
        ConnectionState::WaitingForOffer => "Waiting for offer",
        ConnectionState::UdpHolePunching => "UDP hole punching",
        ConnectionState::TcpConnecting => "TCP connecting",
        ConnectionState::Connected => "Connected",
        ConnectionState::Failed => "Failed",
    };

    let c_str = CString::new(s).unwrap();
    c_str.into_raw()
}
