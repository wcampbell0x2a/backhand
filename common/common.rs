// Compiled for every binary, as this is not a workspce. Don't put many functinos in this file

pub fn after_help() -> String {
    let mut s = "Decompressors available:\n".to_string();

    #[cfg(feature = "gzip")]
    s.push_str("\tgzip\n");

    #[cfg(feature = "xz")]
    s.push_str("\txz\n");

    #[cfg(feature = "lzo")]
    s.push_str("\tlzo\n");

    #[cfg(feature = "zstd")]
    s.push_str("\tzstd\n");

    s.push_str("\nEnvironment Variables:\n\t");
    s.push_str(r#"RUST_LOG: See "https://docs.rs/tracing-subscriber/latest/tracing_subscriber/fmt/index.html#filtering-events-with-environment-variables""#);
    s
}
