//! `xenon-hello` — punto de entrada mínimo de Rust `#![no_std]` para Xbox 360.
//!
//! Se enlaza contra libXenon + newlib con `xenon-gcc`; el `crt1.c` de libXenon
//! llama a `main(argc, argv)` y después a `exit()`.
//!
//! Cadena de arranque real:
//!   XeLL -> _start (newlib/libXenon) -> __crtmain -> main (esta función)
#![no_std]
#![no_main]

use core::ffi::{c_char, c_int};

/// `VIDEO_MODE_*` de <xenos/xenos_videomodes.h>
const VIDEO_MODE_AUTO: c_int = -1;

unsafe extern "C" {
    /// Inicializa el Xenos (framebuffer/CRTC). Requerido por `console_init()`.
    fn xenos_init(video_mode: c_int);
    /// Consola de texto sobre el framebuffer (fuente 8x16 de libXenon).
    fn console_init();
    /// newlib
    fn putchar(c: c_int) -> c_int;
}

/// Escritor mínimo sobre `putchar` para poder usar `core::fmt`.
struct Console;

impl core::fmt::Write for Console {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for b in s.bytes() {
            // La consola espera CRLF explícito.
            if b == b'\n' {
                unsafe { putchar(b'\r' as c_int) };
            }
            unsafe { putchar(b as c_int) };
        }
        Ok(())
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn main(_argc: c_int, _argv: *const *const c_char) -> c_int {
    unsafe {
        xenos_init(VIDEO_MODE_AUTO);
        console_init();
    }

    let _ = core::fmt::Write::write_fmt(
        &mut Console,
        format_args!(
            "hola desde Rust {} en Xenon/PPC32 BE\nentrada: main() via crt1 de libXenon\n",
            env!("CARGO_PKG_VERSION")
        ),
    );

    // Sin scheduler: nos quedamos aquí (crt1 sólo llamaría a exit() si volviéramos).
    loop {
        core::hint::spin_loop();
    }
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    let mut c = Console;
    let _ = core::fmt::Write::write_fmt(&mut c, format_args!("\nPANIC: {}\n", info));

    loop {
        core::hint::spin_loop();
    }
}
