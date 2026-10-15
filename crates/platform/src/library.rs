use std::ffi::CString;

pub struct Library {
    #[cfg(windows)]
    handle: *mut winapi::shared::minwindef::HINSTANCE__,
    #[cfg(unix)]
    handle: *mut std::ffi::c_void,
}

impl Library {
    pub fn new(path: impl AsRef<std::ffi::OsStr>) -> Result<Self, String> {
        let path_str = path.as_ref().to_string_lossy();
        let c_path = CString::new(path_str.as_bytes()).map_err(|e| e.to_string())?;

        #[cfg(windows)]
        let handle = unsafe {
            winapi::um::libloaderapi::LoadLibraryA(c_path.as_ptr())
        };

        #[cfg(unix)]
        let handle = unsafe {
            libc::dlopen(c_path.as_ptr(), libc::RTLD_NOW)
        };

        if handle.is_null() {
            return Err(format!("Failed to load library: {}", path_str));
        }

        Ok(Self { handle })
    }

    pub fn get_symbol<T>(&self, name: impl AsRef<std::ffi::OsStr>) -> Result<T, String> {
        let name_str = name.as_ref().to_string_lossy();
        let c_name = CString::new(name_str.as_bytes()).map_err(|e| e.to_string())?;

        #[cfg(windows)]
        let symbol = unsafe {
            winapi::um::libloaderapi::GetProcAddress(self.handle, c_name.as_ptr())
        };

        #[cfg(unix)]
        let symbol = unsafe {
            libc::dlsym(self.handle, c_name.as_ptr())
        };

        if symbol.is_null() {
            return Err(format!("Failed to find symbol: {}", name_str));
        }

        Ok(unsafe { std::mem::transmute_copy(&symbol) })
    }
}

impl Drop for Library {
    fn drop(&mut self) {
        #[cfg(windows)]
        {
            unsafe { winapi::um::libloaderapi::FreeLibrary(self.handle); }
        }

        #[cfg(unix)]
        {
            unsafe { libc::dlclose(self.handle); }
        }
    }
}

unsafe impl Send for Library {}
unsafe impl Sync for Library {}
