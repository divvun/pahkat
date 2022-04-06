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

pub struct JsonMarshaler;

impl InputType for JsonMarshaler {
    type Foreign = <cffi::StringMarshaler as InputType>::Foreign;
    type ForeignTraitObject = ();
}

impl ReturnType for JsonMarshaler {
    type Foreign = cffi::Slice<u8>;
    type ForeignTraitObject = ();

    fn foreign_default() -> Self::Foreign {
        cffi::Slice::default()
    }
}

impl<T> ToForeign<T, cffi::Slice<u8>> for JsonMarshaler
where
    T: Serialize,
{
    type Error = Box<dyn Error>;

    fn to_foreign(input: T) -> Result<cffi::Slice<u8>, Self::Error> {
        let json_str = serde_json::to_string(&input)?;
        Ok(cffi::StringMarshaler::to_foreign(json_str).unwrap())
    }
}

pub struct JsonRefMarshaler<'a>(&'a std::marker::PhantomData<()>);

impl<'a> InputType for JsonRefMarshaler<'a> {
    type Foreign = <cffi::StrMarshaler<'a> as InputType>::Foreign;
    type ForeignTraitObject = ();
}

impl<'a, T> FromForeign<cffi::Slice<u8>, T> for JsonRefMarshaler<'a>
where
    T: DeserializeOwned,
{
    type Error = Box<dyn Error>;

    unsafe fn from_foreign(ptr: cffi::Slice<u8>) -> Result<T, Self::Error> {
        let json_str =
            <cffi::StrMarshaler<'a> as FromForeign<cffi::Slice<u8>, &'a str>>::from_foreign(ptr)?;
        log::debug!("JSON: {}, type: {}", &json_str, std::any::type_name::<T>());

        let v: Result<T, _> = serde_json::from_str(&json_str);
        v.map_err(|e| {
            log::error!("Json error: {}", &e);
            log::debug!("{:?}", &e);
            Box::new(e) as _
        })
    }
}
