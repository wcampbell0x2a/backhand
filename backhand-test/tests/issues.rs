/// https://github.com/wcampbell0x2a/backhand/issues/275
#[test]
#[cfg(feature = "xz")]
fn issue_275() {
    let mut writer = std::io::Cursor::new(vec![]);
    let mut fs = backhand::FilesystemWriter::default();
    fs.write(&mut writer).unwrap();
}
