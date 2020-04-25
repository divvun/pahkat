use std::convert::TryFrom;
use std::error::Error;
use std::ffi::{CStr, CString};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};

use cursed::{FromForeign, InputType, ReturnType, ToForeign};
use once_cell::sync::Lazy;
use pahkat_types::payload::{
    macos::InstallTarget as MacOSInstallTarget, windows::InstallTarget as WindowsInstallTarget,
};
use serde::de::DeserializeOwned;
use serde::Serialize;
use url::Url;

use crate::config::ConfigPath;
use crate::repo::PayloadError;
use crate::transaction::{PackageStatus, PackageStatusError};
use crate::{Config, PackageKey};

#[cfg(not(target_os = "android"))]
#[no_mangle]
pub extern "C" fn pahkat_enable_logging(level: u8) {
    use std::io::Write;

    if let Some(level) = level_u8_to_str(level) {
        std::env::set_var("RUST_LOG", format!("pahkat_client={}", level));
    }

    env_logger::builder()
        .format(|buf, record| {
            writeln!(
                buf,
                "{} {} {}:{} > {}",
                record.level(),
                record.target(),
                record.file().unwrap_or("<unknown>"),
                record.line().unwrap_or(0),
                record.args()
            )
        })
        .init();
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn pahkat_enable_logging(level: u8) {
    use std::io::Write;

    if let Some(level) = level_u8_to_str(level) {
        std::env::set_var("RUST_LOG", format!("pahkat_client={}", level));
    }

    let mut builder = android_log::LogBuilder::new("PahkatClient");
    builder.format(|record| {
        format!(
            "{} {} {}:{} > {}",
            record.level(),
            record.target(),
            record.file().unwrap_or("<unknown>"),
            record.line().unwrap_or(0),
            record.args()
        )
    });
    builder.init().unwrap();
}

#[inline(always)]
fn level_u8_to_str(level: u8) -> Option<&'static str> {
    Some(match level {
        0 => return None,
        1 => "error",
        2 => "warn",
        3 => "info",
        4 => "debug",
        _ => "trace",
    })
}

pub struct ExternalLogger {
    pub callback: LoggingCallback,
}

fn make_unknown_cstr() -> CString {
    CString::new("<unknown>").unwrap()
}

impl log::Log for ExternalLogger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        true
    }

    fn log(&self, record: &log::Record<'_>) {
        use log::Level::*;

        let level = match record.level() {
            Error => 1,
            Warn => 2,
            Info => 3,
            Debug => 4,
            Trace => 5,
        };

        let msg =
            CString::new(format!("{}", record.args())).unwrap_or_else(|_| make_unknown_cstr());
        let module = CString::new(record.module_path().unwrap_or("<unknown>"))
            .unwrap_or_else(|_| make_unknown_cstr());
        let file_path = format!(
            "{}:{}",
            record.file().unwrap_or("<unknown>"),
            record.line().unwrap_or(0)
        );
        let file_path = CString::new(file_path).unwrap_or_else(|_| make_unknown_cstr());

        (self.callback)(level, msg.as_ptr(), module.as_ptr(), file_path.as_ptr());
    }

    fn flush(&self) {}
}

type LoggingCallback =
    extern "C" fn(u8, *const libc::c_char, *const libc::c_char, *const libc::c_char);
