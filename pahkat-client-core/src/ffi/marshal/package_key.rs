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
pub struct PackageKeyMarshaler<'a>(&'a std::marker::PhantomData<()>);

impl<'a> InputType for PackageKeyMarshaler<'a> {
    type Foreign = <cffi::StrMarshaler<'a> as InputType>::Foreign;
}

impl<'a> ReturnType for PackageKeyMarshaler<'a> {
    type Foreign = <cffi::StringMarshaler as ReturnType>::Foreign;

    fn foreign_default() -> Self::Foreign {
        Default::default()
    }
}

impl<'a> ToForeign<&'a PackageKey, cffi::Slice<u8>> for PackageKeyMarshaler<'a> {
    type Error = std::convert::Infallible;

    fn to_foreign(key: &'a PackageKey) -> Result<cffi::Slice<u8>, Self::Error> {
        cffi::StringMarshaler::to_foreign(key.to_string())
    }
}

impl<'a> FromForeign<cffi::Slice<u8>, PackageKey> for PackageKeyMarshaler<'a> {
    type Error = Box<dyn Error>;

    unsafe fn from_foreign(string: cffi::Slice<u8>) -> Result<PackageKey, Self::Error> {
        let s = <cffi::StrMarshaler<'a> as FromForeign<cffi::Slice<u8>, &'a str>>::from_foreign(
            string,
        )?;
        PackageKey::try_from(s).box_err()
    }
}
