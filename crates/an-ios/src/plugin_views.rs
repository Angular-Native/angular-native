//! Views a plugin brings.
//!
//! Every other kind in `NodeKind` is a control this crate knows how to build.
//! `Custom` is the one it does not: the view belongs to a plugin, is written in
//! Swift, and this crate has never heard of it. So the shell installs a factory
//! and this asks it by name.
//!
//! The pointer that comes back is a `+1` reference — Swift hands it over with
//! `Unmanaged.passRetained` — and this takes ownership of it. Anything else
//! would either leak one view per node or release one Swift still holds.

use std::ffi::{c_char, c_void, CString};
use std::sync::{Mutex, OnceLock};

use objc2::rc::Retained;
use objc2_ui_kit::UIView;

/// What the shell installs: a name in, a retained `UIView` out, or null for a
/// name nobody registered.
pub type PluginViewFn = unsafe extern "C" fn(name: *const c_char) -> *mut c_void;

fn factory() -> &'static Mutex<Option<PluginViewFn>> {
    static FACTORY: OnceLock<Mutex<Option<PluginViewFn>>> = OnceLock::new();
    FACTORY.get_or_init(|| Mutex::new(None))
}

/// Installs the factory. Passing `None` takes it away.
///
/// # Safety
/// The callback has to stay valid for the life of the process, which it does:
/// the shell installs a Swift function once, at startup.
#[unsafe(no_mangle)]
pub extern "C" fn an_plugin_view_set_factory(callback: Option<PluginViewFn>) {
    *factory().lock().expect("poisoned plugin view factory") = callback;
}

/// The view a plugin registered under that name, if any.
///
/// `None` covers both "no plugin brings views at all" and "no plugin brings
/// this one". The caller says so once in the log rather than mounting an empty
/// box nobody can explain.
pub fn make(name: &str) -> Option<Retained<UIView>> {
    let callback = (*factory().lock().expect("poisoned plugin view factory"))?;
    let name = CString::new(name).ok()?;
    let raw = unsafe { callback(name.as_ptr()) };
    if raw.is_null() {
        return None;
    }
    // `+1` from Swift, owned from here. `from_raw` takes that reference rather
    // than adding another.
    unsafe { Retained::from_raw(raw.cast()) }
}
