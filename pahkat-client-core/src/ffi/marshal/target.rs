
use std::convert::TryFrom;
use std::error::Error;
use std::ffi::{CStr, CString};
use std::path::PathBuf;
use std::sync::{Arc, RwLock, Mutex};

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
use crate::ffi::BoxError;

pub struct TargetMarshaler;

impl InputType for TargetMarshaler {
    type Foreign = <cursed::StringMarshaler as InputType>::Foreign;
}

impl ReturnType for TargetMarshaler {
    type Foreign = cursed::Slice<u8>;

    fn foreign_default() -> Self::Foreign {
        cursed::Slice::default()
    }
}

impl ToForeign<MacOSInstallTarget, cursed::Slice<u8>> for TargetMarshaler {
    type Error = std::convert::Infallible;

    fn to_foreign(input: MacOSInstallTarget) -> Result<cursed::Slice<u8>, Self::Error> {
        let str_target = match input {
            MacOSInstallTarget::System => "system",
            MacOSInstallTarget::User => "user",
        };

        cursed::StringMarshaler::to_foreign(str_target.to_string())
    }
}

impl FromForeign<cursed::Slice<u8>, MacOSInstallTarget> for TargetMarshaler {
    type Error = Box<dyn Error>;

    unsafe fn from_foreign(ptr: cursed::Slice<u8>) -> Result<MacOSInstallTarget, Self::Error> {
        let str_target = cursed::StringMarshaler::from_foreign(ptr)?;

        Ok(match &*str_target {
            "user" => MacOSInstallTarget::User,
            _ => MacOSInstallTarget::System,
        })
    }
}

impl ToForeign<WindowsInstallTarget, cursed::Slice<u8>> for TargetMarshaler {
    type Error = std::convert::Infallible;

    fn to_foreign(input: WindowsInstallTarget) -> Result<cursed::Slice<u8>, Self::Error> {
        let str_target = match input {
            WindowsInstallTarget::System => "system",
            WindowsInstallTarget::User => "user",
        };

        cursed::StringMarshaler::to_foreign(str_target.to_string())
    }
}

impl FromForeign<cursed::Slice<u8>, WindowsInstallTarget> for TargetMarshaler {
    type Error = Box<dyn Error>;

    unsafe fn from_foreign(ptr: cursed::Slice<u8>) -> Result<WindowsInstallTarget, Self::Error> {
        let str_target = cursed::StringMarshaler::from_foreign(ptr)?;

        Ok(match &*str_target {
            "user" => WindowsInstallTarget::User,
            _ => WindowsInstallTarget::System,
        })
    }
}