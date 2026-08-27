//! Rust kernel32 loader **oracle** — CI verification only.
//!
//! Per D21 this is NOT the D14 done-gate. A green test here means the C-ABI module
//! loads and calls correctly from *a* host; it does **not** mean the flagship Delphi
//! host (or a C host) works — that gate stays BLOCKED until `dcc64`/`cl`/`gcc` exist.
//!
//! Dependency-free: uses Win32 `LoadLibraryW`/`GetProcAddress` directly.
#![cfg(windows)]

use std::ffi::c_void;

#[link(name = "kernel32")]
extern "system" {
    fn LoadLibraryW(name: *const u16) -> *mut c_void;
    fn GetProcAddress(module: *mut c_void, name: *const u8) -> *mut c_void;
    fn FreeLibrary(module: *mut c_void) -> i32;
}

/// A loaded native module. Unloads on drop.
pub struct Module(*mut c_void);

impl Module {
    /// Load a DLL by path.
    pub fn load(path: &str) -> Result<Module, String> {
        let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
        let h = unsafe { LoadLibraryW(wide.as_ptr()) };
        if h.is_null() {
            Err(format!("LoadLibrary failed: {path}"))
        } else {
            Ok(Module(h))
        }
    }

    /// Resolve an exported symbol. `name` must be nul-terminated (e.g. `b"mlx_f\0"`).
    pub fn symbol(&self, name: &[u8]) -> Result<*mut c_void, String> {
        assert_eq!(
            name.last(),
            Some(&0u8),
            "symbol name must be nul-terminated"
        );
        let p = unsafe { GetProcAddress(self.0, name.as_ptr()) };
        if p.is_null() {
            Err(format!(
                "symbol not found: {}",
                String::from_utf8_lossy(name)
            ))
        } else {
            Ok(p)
        }
    }
}

impl Drop for Module {
    fn drop(&mut self) {
        unsafe {
            FreeLibrary(self.0);
        }
    }
}
