// Compiled for every binary, as this is not a workspace

pub mod parse;

use std::sync::LazyLock;

use clap::builder::styling::*;

#[doc(hidden)]
pub static RED_BOLD: LazyLock<console::Style> =
    LazyLock::new(|| console::Style::new().red().bold());
#[doc(hidden)]
pub static BLUE_BOLD: LazyLock<console::Style> =
    LazyLock::new(|| console::Style::new().blue().bold());

#[doc(hidden)]
pub fn styles() -> clap::builder::Styles {
    Styles::styled()
        .header(AnsiColor::Green.on_default() | Effects::BOLD)
        .usage(AnsiColor::Green.on_default() | Effects::BOLD)
        .literal(AnsiColor::Cyan.on_default() | Effects::BOLD)
        .placeholder(AnsiColor::Cyan.on_default())
        .error(AnsiColor::Red.on_default() | Effects::BOLD)
        .valid(AnsiColor::Cyan.on_default() | Effects::BOLD)
        .invalid(AnsiColor::Yellow.on_default() | Effects::BOLD)
}

#[doc(hidden)]
pub fn after_help_unsquashfs(rayon_env: bool) -> String {
    let mut s = String::new();

    let header = color_print::cstr!("<green, bold>Decompressors available:</>\n");
    s.push_str(header);

    #[cfg(feature = "any-gzip")]
    s.push_str(color_print::cstr!("  <cyan, bold>gzip\n"));

    #[cfg(feature = "xz")]
    s.push_str(color_print::cstr!("  <cyan, bold>xz\n"));

    #[cfg(feature = "lzo")]
    s.push_str(color_print::cstr!("  <cyan, bold>lzo\n"));

    #[cfg(feature = "zstd")]
    s.push_str(color_print::cstr!("  <cyan, bold>zstd\n"));

    s.push_str(&after_help_common(rayon_env));

    s
}

#[doc(hidden)]
pub fn after_help_common(rayon_env: bool) -> String {
    let mut s = String::new();

    s.push_str(color_print::cstr!("<green, bold>Environment Variables:\n"));
    s.push_str(color_print::cstr!("  <cyan, bold>RUST_LOG:"));
    s.push_str("    https://docs.rs/tracing-subscriber/latest/tracing_subscriber/fmt/index.html#filtering-events-with-environment-variables");
    if rayon_env {
        s.push('\n');
        s.push_str(color_print::cstr!("  <cyan, bold>RAYON_NUM_THREADS:"));
        s.push_str(r#"  https://docs.rs/rayon/latest/rayon/struct.ThreadPoolBuilder.html#method.num_threads"#);
    }
    s
}
