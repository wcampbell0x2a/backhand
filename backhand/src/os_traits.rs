use std::ffi::OsStr;

pub trait OsStrExt {
    fn as_bytes(&self) -> &[u8];
}

#[cfg(unix)]
impl OsStrExt for OsStr {
    fn as_bytes(&self) -> &[u8] {
        std::os::unix::ffi::OsStrExt::as_bytes(self)
    }
}

#[cfg(windows)]
impl OsStrExt for OsStr {
    fn as_bytes(&self) -> &[u8] {
        self.to_string_lossy().as_bytes()
    }
}