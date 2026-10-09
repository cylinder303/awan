//! Sleep notifications. On Windows we subscribe to suspend/resume events so a
//! running session pauses the moment the machine goes to sleep. The timer
//! engine also detects sleep from clock gaps as a fallback (e.g. on systems
//! with Modern Standby, where suspend may not be announced).

use tauri::AppHandle;

#[cfg(windows)]
pub fn watch(app: AppHandle) {
    use std::ffi::c_void;
    use std::sync::OnceLock;
    use windows_sys::Win32::System::Power::{
        PowerRegisterSuspendResumeNotification, DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        DEVICE_NOTIFY_CALLBACK, PBT_APMRESUMEAUTOMATIC, PBT_APMSUSPEND,
    };

    static APP: OnceLock<AppHandle> = OnceLock::new();
    if APP.set(app).is_err() {
        return;
    }

    unsafe extern "system" fn callback(_ctx: *const c_void, kind: u32, _setting: *const c_void) -> u32 {
        if let Some(app) = APP.get() {
            match kind {
                PBT_APMSUSPEND => crate::on_suspend(app),
                PBT_APMRESUMEAUTOMATIC => crate::on_resume(app),
                _ => {}
            }
        }
        0
    }

    // The subscription lives for the whole process, so the parameters are leaked.
    let params: &'static mut DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS =
        Box::leak(Box::new(DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS {
            Callback: Some(callback),
            Context: std::ptr::null_mut(),
        }));
    let mut handle = std::ptr::null_mut();
    // SAFETY: `params` is valid for the life of the process; `handle` is an out pointer.
    unsafe {
        PowerRegisterSuspendResumeNotification(
            DEVICE_NOTIFY_CALLBACK,
            params as *mut _ as *mut c_void,
            &mut handle,
        );
    }
}

#[cfg(not(windows))]
pub fn watch(_app: AppHandle) {}
