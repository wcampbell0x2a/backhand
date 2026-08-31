//! Macros that keep this library's text out of a compiled binary
//!
//! With the `error-strings` feature off, the log macros drop the call and
//! `err_text!` gives an empty string, so no format string reaches the binary.
//! A log call must have the shape `level!("literal" [, value]...)`, because the
//! disabled arms below do not accept `tracing` structured fields.

// No call site uses `debug!` or `warn!` today. The shim keeps all five levels so
// that a new call site does not have to add a level first.
#![allow(unused_macros)]

#[cfg(feature = "error-strings")]
macro_rules! trace {
    ($($arg:tt)*) => { ::tracing::trace!($($arg)*) };
}

#[cfg(feature = "error-strings")]
macro_rules! debug {
    ($($arg:tt)*) => { ::tracing::debug!($($arg)*) };
}

#[cfg(feature = "error-strings")]
macro_rules! info {
    ($($arg:tt)*) => { ::tracing::info!($($arg)*) };
}

#[cfg(feature = "error-strings")]
macro_rules! warn {
    ($($arg:tt)*) => { ::tracing::warn!($($arg)*) };
}

#[cfg(feature = "error-strings")]
macro_rules! error {
    ($($arg:tt)*) => { ::tracing::error!($($arg)*) };
}

// `let _ = &$arg` stops a value that a module reads only to log it from raising
// an unused-variable warning.
#[cfg(not(feature = "error-strings"))]
macro_rules! drop_record {
    ($fmt:literal $(,)?) => {{}};
    ($fmt:literal, $($arg:expr),+ $(,)?) => {{ $( let _ = &$arg; )+ }};
}

#[cfg(not(feature = "error-strings"))]
macro_rules! trace {
    ($($arg:tt)*) => { drop_record!($($arg)*) };
}

#[cfg(not(feature = "error-strings"))]
macro_rules! debug {
    ($($arg:tt)*) => { drop_record!($($arg)*) };
}

#[cfg(not(feature = "error-strings"))]
macro_rules! info {
    ($($arg:tt)*) => { drop_record!($($arg)*) };
}

#[cfg(not(feature = "error-strings"))]
macro_rules! warn {
    ($($arg:tt)*) => { drop_record!($($arg)*) };
}

#[cfg(not(feature = "error-strings"))]
macro_rules! error {
    ($($arg:tt)*) => { drop_record!($($arg)*) };
}

/// Build the text of an error message
///
/// Wrap every literal or `format!` that becomes error text, so that the format
/// string and the `{:?}` type names leave the binary with the feature off.
#[cfg(feature = "error-strings")]
macro_rules! err_text {
    ($($arg:tt)*) => { format!($($arg)*) };
}

#[cfg(not(feature = "error-strings"))]
macro_rules! err_text {
    ($fmt:literal $(,)?) => {{ ::std::string::String::new() }};
    ($fmt:literal, $($arg:expr),+ $(,)?) => {{
        $( let _ = &$arg; )+
        ::std::string::String::new()
    }};
}

// lib.rs takes these macros with `#[macro_use]`, not with a `pub(crate) use`
// re-export, because the re-exported name `warn` would clash with the built-in
// attribute.
