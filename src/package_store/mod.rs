#[cfg(all(target_os = "macos", feature = "macos"))]
pub mod macos;

#[cfg(feature = "prefix")]
pub mod tarball;

#[cfg(all(windows, feature = "windows"))]
pub mod windows;