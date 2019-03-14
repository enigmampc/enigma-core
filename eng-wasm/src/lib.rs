#![no_std]
#![feature(slice_concat_ext)]
#![deny(unused_extern_crates)]

/// Enigma implementation of bindings to the Enigma runtime.
/// This crate should be used in contracts.
#[macro_use]
extern crate serde_json;
extern crate serde;
#[macro_use]
mod internal_std;
mod rand_wasm;
pub extern crate eng_pwasm_abi;

pub use internal_std::*;
pub use rand_wasm::*;
pub use serde_json::Value;
pub use eng_pwasm_abi::types::*;


pub mod external {
    extern "C" {
        pub fn write_state (key: *const u8, key_len: u32, value: *const u8, value_len: u32);
        pub fn read_state_len (key: *const u8, key_len: u32) -> i32;
        pub fn read_state (key: *const u8, key_len: u32, value_holder: *const u8);
        pub fn eprint(str_ptr: *const u8, str_len: u32);
        pub fn fetch_function_name_length() -> i32;
        pub fn fetch_function_name(name_holder: *const u8);
        pub fn fetch_args_length() -> i32;
        pub fn fetch_args(name_holder: *const u8);
        pub fn fetch_types_length() -> i32;
        pub fn fetch_types(name_holder: *const u8);
        pub fn write_eth_bridge(payload: *const u8, payload_len: u32, address: *const u8);
        pub fn gas(amount: u32);
        pub fn ret(payload: *const u8, payload_len: u32);
        pub fn rand(payload: *const u8, payload_len: u32);
    }
}

#[no_mangle]
pub fn print(msg: &str) -> i32 {
    unsafe { external::eprint(msg.as_ptr(), msg.len() as u32); }
    0
}

#[macro_export]
macro_rules! eprint {
    ( $($arg: tt)* ) => (
    $crate::print( &eformat!( $($arg)* ) )
    );
}

/// Write to state
pub fn write<T>(key: &str, _value: T) where T: serde::Serialize {
    let value = json!(_value);
    let value_vec = serde_json::to_vec(&value).unwrap();
    unsafe { external::write_state(key.as_ptr(), key.len() as u32, value_vec.as_ptr(), value_vec.len() as u32) }
}

/// Read from state
pub fn read<T>(key: &str) -> Option<T> where for<'de> T: serde::Deserialize<'de> {
    let val_len = unsafe { external::read_state_len(key.as_ptr(), key.len() as u32) };
    let value_holder: Vec<u8> = iter::repeat(0).take(val_len as usize).collect();
    unsafe { external::read_state(key.as_ptr(), key.len() as u32, value_holder.as_ptr()) };
    let value: Value = serde_json::from_slice(&value_holder).map_err(|_| print("failed unwrapping from_slice in read_state")).expect("read_state failed");
    if value.is_null() {
        return None;
    }
    Some(serde_json::from_value(value.clone()).map_err(|_| print("failed unwrapping from_value in read_state")).expect("read_state failed"))
}

pub fn write_ethereum_bridge(payload: &[u8], address: &Address){
    unsafe {
        external::write_eth_bridge(payload.as_ptr(), payload.len() as u32, address.as_ptr())
    };
}

#[macro_export]
 macro_rules! write_state {
     ( $($key: expr => $val: expr),+ ) => {
         {
             $(
                 $crate::write($key, $val);
             )+
         }
     }
 }

#[macro_export]
 macro_rules! read_state {
     ( $key: expr ) => {
         {
             $crate::read($key)
         }
     }
 }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn what() {
        print("TEST!");
    }
}

