use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize};
use windows::core::{Error, HRESULT, Result};

pub(crate) const E_FAIL: HRESULT = HRESULT(0x80004005_u32 as i32);

pub(crate) fn run_sta<T: Send>(f: impl FnOnce() -> Result<T> + Send) -> Result<T> {
    std::thread::scope(|s| {
        s.spawn(|| {
            unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()? };
            let r = f();
            unsafe { CoUninitialize() };
            r
        })
        .join()
        .unwrap_or_else(|_| Err(Error::new(E_FAIL, "clipboard thread panicked")))
    })
}
