#![cfg(feature = "ffi")]

#[cfg(all(target_os = "macos", feature = "macos"))]
pub mod macos;
#[cfg(windows)]
pub mod windows;

#[cfg(feature = "prefix")]
pub mod prefix;
