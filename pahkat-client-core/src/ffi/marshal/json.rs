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
use crate::ffi::BoxError;
use crate::repo::PayloadError;
use crate::transaction::{PackageStatus, PackageStatusError};
use crate::{Config, PackageKey};

pub struct JsonRefMarshaler<'a>(&'a std::marker::PhantomData<()>);
pub struct JsonMarshaler;

impl InputType for JsonMarshaler {
    type Foreign = <cursed::StringMarshaler as InputType>::Foreign;
}

impl ReturnType for JsonMarshaler {
    type Foreign = cursed::Slice<u8>;

    fn foreign_default() -> Self::Foreign {
        cursed::Slice::default()
    }
}

impl<'a> InputType for JsonRefMarshaler<'a> {
    type Foreign = <cursed::StrMarshaler<'a> as InputType>::Foreign;
}

impl<T> ToForeign<T, cursed::Slice<u8>> for JsonMarshaler
where
    T: Serialize,
{
    type Error = Box<dyn Error>;

    fn to_foreign(input: T) -> Result<cursed::Slice<u8>, Self::Error> {
        let json_str = serde_json::to_string(&input)?;
        Ok(cursed::StringMarshaler::to_foreign(json_str).unwrap())
    }
}

impl<'a, T> FromForeign<cursed::Slice<u8>, T> for JsonRefMarshaler<'a>
where
    T: DeserializeOwned,
{
    type Error = Box<dyn Error>;

    unsafe fn from_foreign(ptr: cursed::Slice<u8>) -> Result<T, Self::Error> {
        let json_str =
            <cursed::StrMarshaler<'a> as FromForeign<cursed::Slice<u8>, &'a str>>::from_foreign(
                ptr,
            )?;
        log::debug!("JSON: {}, type: {}", &json_str, std::any::type_name::<T>());

        let v: Result<T, _> = serde_json::from_str(&json_str);
        v.map_err(|e| {
            log::error!("Json error: {}", &e);
            log::debug!("{:?}", &e);
            Box::new(e) as _
        })
    }
}
