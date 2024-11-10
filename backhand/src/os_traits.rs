use std::ffi::OsStr;

pub trait OsStrExt {
    fn as_bytes(&self) -> &[u8];
    fn from_bytes(slice: &[u8]) -> &Self;
}

#[cfg(unix)]
impl OsStrExt for OsStr {
    fn as_bytes(&self) -> &[u8] {
        std::os::unix::ffi::OsStrExt::as_bytes(self)
    }

    fn from_bytes(slice: &[u8]) -> &Self {
        std::os::unix::ffi::OsStrExt::from_bytes(slice)
    }
}

#[cfg(windows)]
impl OsStrExt for OsStr {
    fn as_bytes(&self) -> &[u8] {
        self.to_string_lossy().as_bytes()
    }

    fn from_bytes(slice: &[u8]) -> &Self {
        todo!()
    }
}