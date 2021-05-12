//! This crate provides backtrace support for `no_std` and embedded programs.
//!
//! This is done through by compiling LLVM's libunwind with certain flags to remove
//! all OS dependencies, including libc and any memory allocations.
//!
//! # Usage
//!
//! ### Setup
//!
//! There are two prerequisites for using this crate:
//!
//! 1. Unwind tables must be built into the binary, even with unwinding is set to
//!    abort. This can be done with the `-C force-unwind-tables` flag.
//!
//! 2. Several `__eh_frame_*` symbols need to be defined by the linker so that the
//!    the unwinding tables can be located by libunwind. This can be done by
//!    including the [`eh_frame.ld`] linker script fragment.
//!
//! Both of these can be done by setting `RUSTFLAGS`:
//!
//! ```sh
//! export RUSTFLAGS="-Cforce-unwind-tables -Clink-arg=-Wl,eh_frame.ld"
//! ```
//!
//! Note that these flags also apply to build-dependencies and proc
//! macros by default. This can be worked around by explicitly
//! specifying a target when invoking cargo:
//!
//! ```sh
//! # Applies RUSTFLAGS to everything
//! cargo build
//!
//! # Doesn't apply RUSTFLAGS to build dependencies and proc macros
//! cargo build --target x86_64-unknown-linux-gnu
//! ```
//!
//! [`eh_frame.ld`]: https://github.com/Amanieu/mini-backtrace/blob/master/eh_frame.ld
//!
//! ### Capturing backtraces
//!
//! Add the `mini-backtrace` crate as a dependency to your program:
//!
//! ```toml
//! [dependencies]
//! mini-backtrace = "0.1"
//! ```
//!
//! You can capture a backtrace by using the `Backtrace` type which returns a list
//! of frames as an `ArrayVec` of instruction pointer addresses.
//!
//! ```rust
//! use mini_backtrace::Backtrace;
//!
//! // Capture up to 16 frames. This is returned using an ArrayVec that doesn't
//! // perform any dynamic memory allocation.
//! let bt = Backtrace::<16>::capture();
//! println!("Backtrace:");
//! for frame in bt.frames {
//!     println!("  {:#x}", frame);
//! }
//! if bt.frames_omitted {
//!     println!(" ... <frames omitted>");
//! }
//! ```
//!
//! This will output:
//!
//! ```text
//! Backtrace:
//!   0x5587058c3eb1
//!   0x5587058c3cdb
//!   0x5587058c491e
//!   0x5587058c38b1
//!   0x5587058daf1a
//!   0x5587058c3890
//!   0x5587058c414c
//! ```
//!
//! ### Position-independent code
//!
//! If your code is executing at a different address than the one it is linked at
//! then you will need to fix up the frame pointer addresses to be relative to the
//! module base address. This can be done with the following function:
//!
//! ```rust
//! fn adjust_for_pic(ip: usize) -> usize {
//!     extern "C" {
//!         // Symbol defined by the linker
//!         static __executable_start: [u8; 0];
//!     }
//!     let base = unsafe { __executable_start.as_ptr() as usize };
//!     ip - base
//! }
//! ```
//!
//! After post-processing, the output should look like this:
//!
//! ```text
//! Backtrace:
//!   0x8eb1
//!   0x8cdb
//!   0x999e
//!   0x88b1
//!   0x1ffba
//!   0x8890
//!   0x91cc
//! ```
//!
//! Have a look at `examples/backtrace.rs` for a complete example.
//!
//! Note that `adjust_for_pic` should *only* be called for position-independent
//! binaries. Statically-linked binaries should emit unadjusted addresses so that
//! the backtraces can be correctly resolved.
//!
//! ### Resolving backtraces
//!
//! The addresses generated by `Backtrace` can be converted to human-readable
//! function names, filenames and line numbers by using the `addr2line` tool from
//! LLVM or binutils with [rustfilt] to demangle Rust symbol names.
//!
//! Simply run `addr2line -fipe /path/to/binary | rustfilt` in a terminal and then
//! paste the addresses from the backtrace:
//!
//! ```text
//! $ llvm-addr2line -fipe target/x86_64-unknown-linux-gnu/debug/examples/backtrace | rustfilt
//!   0x8ed1
//!   0x8ea6
//!   0x8e96
//!   0x8cdb
//!   0x99be
//!   0x88b1
//!   0x1ffda
//!   0x8890
//!   0x91ec
//! backtrace::bar at /home/amanieu/code/mini-backtrace/examples/backtrace.rs:15
//! backtrace::foo at /home/amanieu/code/mini-backtrace/examples/backtrace.rs:10
//! backtrace::main at /home/amanieu/code/mini-backtrace/examples/backtrace.rs:5
//! core::ops::function::FnOnce::call_once at /home/amanieu/.rustup/toolchains/nightly-x86_64-unknown-linux-gnu/lib/rustlib/src/rust/library/core/src/ops/function.rs:227
//! std::sys_common::backtrace::__rust_begin_short_backtrace at /home/amanieu/.rustup/toolchains/nightly-x86_64-unknown-linux-gnu/lib/rustlib/src/rust/library/std/src/sys_common/backtrace.rs:128
//! std::rt::lang_start::{{closure}} at /home/amanieu/.rustup/toolchains/nightly-x86_64-unknown-linux-gnu/lib/rustlib/src/rust/library/std/src/rt.rs:49
//! std::panicking::try at /rustc/676ee14729462585b969bbc52f32c307403f4126/library/std/src/panicking.rs:344
//!  (inlined by) std::panic::catch_unwind at /rustc/676ee14729462585b969bbc52f32c307403f4126/library/std/src/panic.rs:431
//!  (inlined by) std::rt::lang_start_internal at /rustc/676ee14729462585b969bbc52f32c307403f4126/library/std/src/rt.rs:34
//! std::rt::lang_start at /home/amanieu/.rustup/toolchains/nightly-x86_64-unknown-linux-gnu/lib/rustlib/src/rust/library/std/src/rt.rs:48
//! main at ??:0
//! ```
//!
//! [rustfilt]: https://github.com/luser/rustfilt
//!
//! ### Backtraces from signal/interrupt handlers
//!
//! The libunwind unwinder used by this crate is usually unable to unwind past
//! signal handler or interrupt handler frames. Instead, you can use
//! `Backtrace::capture_from_context` and pass in the register state at the point
//! where the exception occurred. In a signal handler this can be obtained through
//! the `uc_mcontext` field of `ucontext_t`.
//!
//! This is currently only implemented for:
//! - AArch64
//! - RISC-V (RV32 & RV64)

#![no_std]

use arrayvec::ArrayVec;
use core::mem::MaybeUninit;

#[allow(non_upper_case_globals)]
#[allow(non_camel_case_types)]
#[allow(non_snake_case)]
#[allow(dead_code)]
mod uw {
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}

cfg_if::cfg_if! {
    if #[cfg(target_arch = "aarch64")] {
        mod aarch64;
        pub use aarch64::Context;
    } else if #[cfg(any(target_arch = "riscv64", target_arch = "riscv32"))] {
        mod riscv;
        pub use riscv::Context;
    }
}

/// A backtrace consisting of a list of instruction pointer addresses.
///
/// The backtrace does not allocate any memory, which allows it to be used in
/// environments where dynamic allocation cannot be used such as signal handlers
/// or interrupt handlers.
///
/// The `N` generic constant controls the maximum number of entries that should
/// be included in the backtrace. Usually 16 frames are enough to get sufficient
/// context from a crash.
#[derive(Clone, Debug, Default)]
pub struct Backtrace<const N: usize> {
    /// List of instruction pointer addresses in each frame, from most recent to
    /// oldest.
    ///
    /// These are not precise return address: the addresses are adjusted so that
    /// they point within the bounds of the caller function. This avoids issues
    /// when a call instruction is the last instruction in a function, which
    /// would otherwise result in a return address pointing at the start of the
    /// next function.
    pub frames: ArrayVec<usize, N>,

    /// Whether any frames have been omitted due to exceeding the capacity of
    /// the `ArrayVec`.
    pub frames_omitted: bool,
}

impl<const N: usize> Backtrace<N> {
    /// Captures a backtrace from the current call point.
    ///
    /// The first frame of the backtrace is the caller of `Backtrace::capture`.
    #[inline(never)]
    pub fn capture() -> Self {
        unsafe {
            let mut unw_context = MaybeUninit::uninit();
            let mut unw_cursor = MaybeUninit::uninit();
            uw::unw_getcontext(unw_context.as_mut_ptr());
            uw::unw_init_local(unw_cursor.as_mut_ptr(), unw_context.as_mut_ptr());

            let mut result = Self::default();
            result.fill_from_cursor(unw_cursor.as_mut_ptr());
            result
        }
    }

    /// Captures a backtrace from the given register context.
    ///
    /// The first frame of the backtrace is the instruction pointer address in
    /// the register context.
    ///
    /// This function is useful for capturing backtraces from signal handlers
    /// since the unwinder may not be able to unwind past the signal frame.
    ///
    /// If no unwinding information is found for the instruction pointer address
    /// in the context then `None` is returned.
    #[cfg(any(target_arch = "aarch64", target_arch = "riscv64"))]
    pub fn capture_from_context(ctx: &Context) -> Option<Self> {
        unsafe {
            let mut unw_context = MaybeUninit::uninit();
            let mut unw_cursor = MaybeUninit::uninit();
            uw::unw_getcontext(unw_context.as_mut_ptr());
            uw::unw_init_local(unw_cursor.as_mut_ptr(), unw_context.as_mut_ptr());

            // Apply the register state to the cursor.
            ctx.apply(unw_cursor.as_mut_ptr());

            // Check if we actually have unwind info for the fault address. We
            // don't generate a backtrace if the fault happened outside our
            // executable.
            let mut unw_proc_info = MaybeUninit::uninit();
            if uw::unw_get_proc_info(unw_cursor.as_mut_ptr(), unw_proc_info.as_mut_ptr())
                != uw::UNW_ESUCCESS
            {
                return None;
            }

            // Add the instruction pointer address from the context as the first
            // frame of the backtrace.
            let mut result = Self::default();
            result.frames.push(ctx.ip());
            result.fill_from_cursor(unw_cursor.as_mut_ptr());
            Some(result)
        }
    }

    unsafe fn fill_from_cursor(&mut self, cursor: *mut uw::unw_cursor_t) {
        while uw::unw_step(cursor) > 0 {
            let mut ip = 0;
            uw::unw_get_reg(cursor, uw::UNW_REG_IP, &mut ip);

            // Adjust the IP to point within the function symbol. This should
            // only be done if the frame is not a signal frame.
            if uw::unw_is_signal_frame(cursor) > 0 {
                ip -= 1;
            }

            if self.frames.try_push(ip).is_err() {
                self.frames_omitted = true;
                break;
            }
        }
    }
}

#[test]
fn capture() {
    let bt = Backtrace::<16>::capture();
    assert!(bt.frames.len() > 1);
}