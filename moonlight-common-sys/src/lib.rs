#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

#[cfg(feature = "windows-symbol-fix")]
#[allow(clippy::missing_safety_doc)]
pub mod windows_fix;
