use windows_sys::Win32::Media::{timeBeginPeriod, timeEndPeriod};

#[unsafe(no_mangle)]
pub unsafe extern "C" fn __imp_timeBeginPeriod(uperiod: u32) -> u32 {
    unsafe { timeBeginPeriod(uperiod) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn __imp_timeEndPeriod(uperiod: u32) -> u32 {
    unsafe { timeEndPeriod(uperiod) }
}
