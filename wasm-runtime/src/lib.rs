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

/// Adds two numbers.
#[no_mangle]
pub const extern "C" fn add(a: u32, b: u32) -> u32 {
    a + b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
