/**
 * ffi/session.rs
 * 
 * FFI bindings for pineapple session functionality
 */

use super::*;
use crate::{Session as RustSession, pqxdh};
use std::os::raw::c_char;

/// Create a new user identity
#[no_mangle]
pub extern "C" fn pineapple_user_new() -> ByteBuffer {
    let user = pqxdh::User::new();
    
    // Serialize user to bytes (you'll need to implement serialization)
    // For now, return empty buffer
    ByteBuffer::empty()
}

/// Create session as initiator (Alice)
#[no_mangle]
pub extern "C" fn pineapple_session_new_initiator(
    alice_bytes: ByteBuffer,
    bob_bytes: ByteBuffer,
) -> *mut SessionHandle {
    // This is a placeholder - proper implementation would deserialize users
    // and create session
    std::ptr::null_mut()
}

/// Create session as responder (Bob)
#[no_mangle]
pub extern "C" fn pineapple_session_new_responder(
    bob_bytes: ByteBuffer,
    init_message_bytes: ByteBuffer,
) -> *mut SessionHandle {
    // Placeholder
    std::ptr::null_mut()
}

/// Send message through session
#[no_mangle]
pub extern "C" fn pineapple_session_send(
    handle: *mut SessionHandle,
    message_data: *const u8,
    message_len: usize,
) -> ByteBuffer {
    if handle.is_null() || message_data.is_null() {
        set_last_error("Invalid arguments");
        return ByteBuffer::empty();
    }

    let session = unsafe { &mut *(handle as *mut RustSession) };
    let message = unsafe { std::slice::from_raw_parts(message_data, message_len) };

    match session.send_bytes(message) {
        Ok(msg) => {
            // Serialize ratchet message
            let serialized = crate::network::serialize_ratchet_message(&msg);
            ByteBuffer::from_vec(serialized)
        }
        Err(e) => {
            set_last_error(&format!("Send failed: {}", e));
            ByteBuffer::empty()
        }
    }
}

/// Receive message through session
#[no_mangle]
pub extern "C" fn pineapple_session_receive(
    handle: *mut SessionHandle,
    message_data: *const u8,
    message_len: usize,
) -> ByteBuffer {
    if handle.is_null() || message_data.is_null() {
        set_last_error("Invalid arguments");
        return ByteBuffer::empty();
    }

    let session = unsafe { &mut *(handle as *mut RustSession) };
    let message_bytes = unsafe { std::slice::from_raw_parts(message_data, message_len) };

    // Deserialize ratchet message
    let msg = match crate::network::deserialize_ratchet_message(message_bytes) {
        Ok(m) => m,
        Err(e) => {
            set_last_error(&format!("Deserialization failed: {}", e));
            return ByteBuffer::empty();
        }
    };

    match session.receive(msg) {
        Ok(plaintext) => ByteBuffer::from_vec(plaintext),
        Err(e) => {
            set_last_error(&format!("Receive failed: {}", e));
            ByteBuffer::empty()
        }
    }
}

/// Free session instance
#[no_mangle]
pub extern "C" fn pineapple_session_free(handle: *mut SessionHandle) {
    if !handle.is_null() {
        unsafe {
            let _ = Box::from_raw(handle as *mut RustSession);
        }
    }
}
