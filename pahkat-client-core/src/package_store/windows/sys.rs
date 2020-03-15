use std::ffi::{OsStr, OsString};
use std::ops::Range;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::ffi::OsStringExt;
use std::slice;
use winapi::ctypes::c_void;
use winapi::um::shellapi::CommandLineToArgvW;
use winapi::um::winbase::LocalFree;

// https://github.com/rust-lang/rust/blob/f76d9bcfc2c269452522fbbe19f66fe653325646/src/libstd/sys/windows/os.rs#L286-L289
pub struct Args {
    range: Range<isize>,
    cur: *mut *mut u16,
}

impl Iterator for Args {
    type Item = OsString;
    fn next(&mut self) -> Option<OsString> {
        self.range.next().map(|i| unsafe {
            let ptr = *self.cur.offset(i);
            let mut len = 0;
            while *ptr.offset(len) != 0 {
                len += 1;
            }

            // Push it onto the list.
            let ptr = ptr as *const u16;
            let buf = slice::from_raw_parts(ptr, len as usize);
            OsStringExt::from_wide(buf)
        })
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.range.size_hint()
    }
}

impl ExactSizeIterator for Args {
    fn len(&self) -> usize {
        self.range.len()
    }
}

impl Drop for Args {
    fn drop(&mut self) {
        unsafe {
            LocalFree(self.cur as *mut c_void);
        }
    }
}

pub fn args<S: AsRef<OsStr>>(input: S) -> Args {
    let input_vec: Vec<u16> = OsStr::new(&input)
        .encode_wide()
        .chain(Some(0).into_iter())
        .collect();
    let lp_cmd_line = input_vec.as_ptr();
    let mut args: i32 = 0;
    let arg_list: *mut *mut u16 = unsafe { CommandLineToArgvW(lp_cmd_line, &mut args) };
    Args {
        range: 0..(args as isize),
        cur: arg_list,
    }
}
