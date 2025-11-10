// Disable `std` library and `main` entrypoint, because they are not available in WebAssembly.
// OBS: `std` and `main` are only available when running tests.
#![cfg_attr(not(feature = "std"), no_std, no_main)]

use const_hex as hex;
use parity_scale_codec::Decode;
use wasm_types::Message;

#[cfg(not(feature = "std"))]
#[macro_use]
extern crate alloc;

#[cfg(not(feature = "std"))]
#[global_allocator]
static mut ALLOC: dlmalloc::GlobalDlmalloc = dlmalloc::GlobalDlmalloc;

// Override the default panic handler when compiling to WebAssembly.
// Reference: https://doc.rust-lang.org/nomicon/panic-handler.html
#[cfg(all(
    not(feature = "std"),
    target_arch = "wasm32",
    any(target_os = "unknown", target_os = "none")
))]
#[panic_handler]
unsafe fn panic(_info: &core::panic::PanicInfo) -> ! {
    core::arch::wasm32::unreachable();
}

// Código externo
pub mod ext {
    // import to the "env" namespace.
    #[cfg(target_family = "wasm")] // Only available when compiling to WebAssembly.
    #[link(wasm_import_module = "env")]
    extern "C" {
        #[allow(clippy::missing_safety_doc)]
        pub fn console_log(ptr: *const u8, len: u32);

        pub fn get_input(ptr: *mut u8, len: &mut u32);
    }

    #[cfg(not(target_family = "wasm"))]
    #[allow(clippy::missing_safety_doc)]
    pub unsafe fn console_log(ptr: *const u8, len: u32) {
        let slice = core::slice::from_raw_parts(ptr, len as usize);
        if let Ok(message) = core::str::from_utf8(slice) {
            println!("{message}");
        }
    }

    #[cfg(not(target_family = "wasm"))]
    #[allow(clippy::missing_safety_doc)]
    #[allow(clippy::missing_const_for_fn)]
    pub unsafe fn get_input(_ptr: *mut u8, len: &mut u32) {
        *len = 0;
    }
}

/// Logs a message to the console.
fn log(message: &str) {
    unsafe {
        #[allow(clippy::cast_possible_truncation)]
        ext::console_log(message.as_ptr(), message.len() as u32);
    }
}

/// Adds two numbers.
#[no_mangle]
#[allow(clippy::missing_safety_doc)]
pub unsafe extern "C" fn add(a: u32, b: u32) -> u32 {
    log("hello, world!");
    a + b
}

/// Indica se a call foi processada com sucesso ou não.
const OK: u32 = 1;
const FAILURE: u32 = 0;

/// Le e decoda uma struct enviada pelo Host.
#[no_mangle]
#[allow(clippy::missing_safety_doc)]
pub unsafe extern "C" fn call(input_size: u32) -> u32 {
    // Alloca espaço na Heap
    // 8192 capacity 8192
    let mut input = vec![0u8; input_size as usize];
    let mut length = input_size;
    {
        let buffer = input.as_mut_slice();
        ext::get_input(buffer.as_mut_ptr(), &mut length);
    }
    if length as usize > input.len() {
        return FAILURE;
    }
    // 200 capacity 8192
    input.set_len(length as usize);

    // Transforma um o pointeiro em slice.
    // let mut bytes = core::slice::from_raw_parts(ptr, len as usize);

    // Imprime os bytes em hexadecimal.
    log(format!("recebido: {}", hex::encode_prefixed(&input)).as_str());

    // Tenta decodar a mensagem.
    let Ok(point) = Message::decode(&mut input.as_ref()) else {
        log("não foi possível decodar a mensagem.");
        return FAILURE;
    };

    // Imprime a mensagem.
    let message = format!("mensagem: {point:?}");
    log(message.as_str());
    OK
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_add() {
        let result = unsafe { add(1, 2) };
        assert_eq!(result, 3);
    }
}
