use std::error::Error;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, ATOMIC_BOOL_INIT, Ordering};
use std::process;

static SUPPRESS_STDERR: AtomicBool = ATOMIC_BOOL_INIT;
static SUPPRESS_STDOUT: AtomicBool = ATOMIC_BOOL_INIT;

/// This method should be called before any thread is spawned.
pub fn suppress_stderr() {
    SUPPRESS_STDERR.store(true, Ordering::Release);
}

pub fn suppress_stdout() {
    SUPPRESS_STDOUT.store(true, Ordering::Release);
}

#[inline]
pub fn is_stderr_enable() -> bool {
    SUPPRESS_STDERR.load(Ordering::Acquire)
}

#[inline]
pub fn is_stdout_enable() -> bool {
    SUPPRESS_STDOUT.load(Ordering::Acquire)
}

#[macro_export]
macro_rules! fatal {
    ($fmt:expr) => {
        if !$crate::is_stderr_enable() {
            eprint!(concat!($fmt, "\n"))
        }
    };
    ($fmt:expr, $($arg:tt)*) => {
        if !$crate::is_stderr_enable() {
            eprint!(concat!($fmt, "\n"), $($arg)*)
        }
    };
}

#[macro_export]
macro_rules! output {
    ($fmt:expr) => {
        if !$crate::is_stdout_enable() {
            print!($fmt)
        }
    };
    ($fmt:expr, $($arg:tt)*) => {
        if !$crate::is_stdout_enable() {
            print!($fmt, $($arg)*)
        }
    };
}

#[macro_export]
macro_rules! outputln {
    ($fmt:expr) => (output!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => (output!(concat!($fmt, "\n"), $($arg)*));
}

pub fn parse<T>(s: &str) -> T
    where T: FromStr,
          T::Err: Error
{
    match s.parse() {
        Ok(t) => t,
        Err(e) => {
            fatal!("failed to parse {:?}: {}", s, e);
            process::exit(-1);
        }
    }
}
