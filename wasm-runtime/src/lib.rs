// Disable `std` library and `main` entrypoint, because they are not available in WebAssembly.
// OBS: `std` and `main` are only available when running tests.
#![cfg_attr(all(target_arch = "wasm32", not(test)), no_std, no_main)]

// Override the default panic handler when compilling to WebAssembly.
// Reference: https://doc.rust-lang.org/nomicon/panic-handler.html
#[cfg(target_arch = "wasm32")]
#[panic_handler]
unsafe fn panic(_info: &core::panic::PanicInfo) -> ! {
    core::arch::wasm32::unreachable()
}

#[cfg(target_arch = "wasm32")] // Only available when compiling to WebAssembly.
pub mod ext {
    #[link(wasm_import_module = "env")] // Add the import to the "env" namespace.
    extern "C" {
        #[allow(clippy::missing_safety_doc)]
        pub fn console_log(ptr: *const u8, len: u32);
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub mod ext {
    #[allow(clippy::missing_safety_doc)]
    pub unsafe fn console_log(ptr: *const u8, len: u32) {
        let slice = core::slice::from_raw_parts(ptr, len as usize);
        if let Ok(message) = core::str::from_utf8(slice) {
            println!("{message}");
        }
    }
}

// static MESSAGE: &str = "hello, world!";

/// Logs a message to the console.
fn log(message: &'static str) {
    unsafe {
        #[allow(clippy::cast_possible_truncation)]
        ext::console_log(message.as_ptr(), message.len() as u32);
    }
}

/// Adds two numbers.
#[no_mangle]
pub extern "C" fn add(a: u32, b: u32) -> u32 {
    log("hello, world!");
    a + b
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_add() {
        let result = add(1, 2);
        assert_eq!(result, 3);
    }
}
