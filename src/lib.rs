//! A library for acquiring a backtrace at runtime
//!
//! This library is meant to supplement the `RUST_BACKTRACE=1` support of the
//! standard library by allowing an acquisition of a backtrace at runtime
//! programmatically. The backtraces generated by this library do not need to be
//! parsed, for example, and expose the functionality of multiple backend
//! implementations.
//!
//! # Usage
//!
//! First, add this to your Cargo.toml
//!
//! ```toml
//! [dependencies]
//! backtrace = "0.3"
//! ```
//!
//! Next:
//!
//! ```
//! fn main() {
//! # // Unsafe here so test passes on no_std.
//! # #[cfg(feature = "std")] {
//!     backtrace::trace(|frame| {
//!         let ip = frame.ip();
//!         let symbol_address = frame.symbol_address();
//!
//!         // Resolve this instruction pointer to a symbol name
//!         backtrace::resolve_frame(frame, |symbol| {
//!             if let Some(name) = symbol.name() {
//!                 // ...
//!             }
//!             if let Some(filename) = symbol.filename() {
//!                 // ...
//!             }
//!         });
//!
//!         true // keep going to the next frame
//!     });
//! }
//! # }
//! ```

#![doc(html_root_url = "https://docs.rs/backtrace")]
#![deny(missing_docs)]
#![no_std]
#![cfg_attr(
    all(feature = "std", target_env = "sgx", target_vendor = "fortanix"),
    feature(sgx_platform)
)]
#![warn(rust_2018_idioms)]
// When we're building as part of libstd, silence all warnings since they're
// irrelevant as this crate is developed out-of-tree.
#![cfg_attr(backtrace_in_libstd, allow(warnings))]

#[cfg(feature = "std")]
#[macro_use]
extern crate std;

// This is only used for gimli right now, so silence warnings elsewhere.
#[cfg_attr(not(target_os = "linux"), allow(unused_extern_crates))]
extern crate alloc;

pub use self::backtrace::{trace_unsynchronized, Frame};
mod backtrace;

pub use self::symbolize::resolve_frame_unsynchronized;
pub use self::symbolize::{resolve_unsynchronized, Symbol, SymbolName};
mod symbolize;

pub use self::types::BytesOrWideString;
mod types;

#[cfg(feature = "std")]
pub use self::symbolize::clear_symbol_cache;

mod print;
pub use print::{BacktraceFmt, BacktraceFrameFmt, PrintFmt};

cfg_if::cfg_if! {
    if #[cfg(feature = "std")] {
        pub use self::backtrace::trace;
        pub use self::symbolize::{resolve, resolve_frame};
        pub use self::capture::{Backtrace, BacktraceFrame, BacktraceSymbol};
        mod capture;
    }
}

#[allow(dead_code)]
struct Bomb {
    enabled: bool,
}

#[allow(dead_code)]
impl Drop for Bomb {
    fn drop(&mut self) {
        if self.enabled {
            panic!("cannot panic during the backtrace function");
        }
    }
}

#[allow(dead_code)]
#[cfg(feature = "std")]
mod lock {
    use std::boxed::Box;
    use std::cell::Cell;
    use std::sync::{Mutex, MutexGuard, Once};

    pub struct LockGuard(Option<MutexGuard<'static, ()>>);

    static mut LOCK: *mut Mutex<()> = 0 as *mut _;
    static INIT: Once = Once::new();
    thread_local!(static LOCK_HELD: Cell<bool> = Cell::new(false));

    impl Drop for LockGuard {
        fn drop(&mut self) {
            if self.0.is_some() {
                LOCK_HELD.with(|slot| {
                    assert!(slot.get());
                    slot.set(false);
                });
            }
        }
    }

    pub fn lock() -> LockGuard {
        if LOCK_HELD.with(|l| l.get()) {
            return LockGuard(None);
        }
        LOCK_HELD.with(|s| s.set(true));
        unsafe {
            INIT.call_once(|| {
                LOCK = Box::into_raw(Box::new(Mutex::new(())));
            });
            LockGuard(Some((*LOCK).lock().unwrap()))
        }
    }
}

