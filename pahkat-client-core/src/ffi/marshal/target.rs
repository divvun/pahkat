use std::convert::TryFrom;
use std::error::Error;
use std::ffi::{CStr, CString};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};

use cffi::{FromForeign, InputType, ReturnType, ToForeign};
use once_cell::sync::Lazy;
use pahkat_types::payload::{
    macos::InstallTarget as MacOSInstallTarget, windows::InstallTarget as WindowsInstallTarget,
};
use serde::de::DeserializeOwned;
use serde::Serialize;
use url::Url;

use crate::config::ConfigPath;
use crate::ffi::BoxError;
use crate::repo::PayloadError;
use crate::transaction::{PackageStatus, PackageStatusError};
use crate::{Config, PackageKey};

pub struct TargetMarshaler;

impl InputType for TargetMarshaler {
    type Foreign = <cffi::StringMarshaler as InputType>::Foreign;
    type ForeignTraitObject = ();
}

impl ReturnType for TargetMarshaler {
    type Foreign = cffi::Slice<u8>;
    type ForeignTraitObject = ();

    fn foreign_default() -> Self::Foreign {
        cffi::Slice::default()
    }
}

impl ToForeign<MacOSInstallTarget, cffi::Slice<u8>> for TargetMarshaler {
    type Error = std::convert::Infallible;

    fn to_foreign(input: MacOSInstallTarget) -> Result<cffi::Slice<u8>, Self::Error> {
        let str_target = match input {
            MacOSInstallTarget::System => "system",
            MacOSInstallTarget::User => "user",
        };

        cffi::StringMarshaler::to_foreign(str_target.to_string())
    }
}

impl FromForeign<cffi::Slice<u8>, MacOSInstallTarget> for TargetMarshaler {
    type Error = Box<dyn Error>;

    unsafe fn from_foreign(ptr: cffi::Slice<u8>) -> Result<MacOSInstallTarget, Self::Error> {
        let str_target = cffi::StringMarshaler::from_foreign(ptr)?;

        Ok(match &*str_target {
            "user" => MacOSInstallTarget::User,
            _ => MacOSInstallTarget::System,
        })
    }
}

impl ToForeign<WindowsInstallTarget, cffi::Slice<u8>> for TargetMarshaler {
    type Error = std::convert::Infallible;

    fn to_foreign(input: WindowsInstallTarget) -> Result<cffi::Slice<u8>, Self::Error> {
        let str_target = match input {
            WindowsInstallTarget::System => "system",
            WindowsInstallTarget::User => "user",
        };

        cffi::StringMarshaler::to_foreign(str_target.to_string())
    }
}

impl FromForeign<cffi::Slice<u8>, WindowsInstallTarget> for TargetMarshaler {
    type Error = Box<dyn Error>;

    unsafe fn from_foreign(ptr: cffi::Slice<u8>) -> Result<WindowsInstallTarget, Self::Error> {
        let str_target = cffi::StringMarshaler::from_foreign(ptr)?;

        Ok(match &*str_target {
            "user" => WindowsInstallTarget::User,
            _ => WindowsInstallTarget::System,
        })
    }
}
