//! Parse helpers for mksquashfs-backhand

use std::{ffi::CString, num::ParseIntError, str::FromStr};

use backhand::compression::{Compressor, XzFilter};

pub fn parse_block_size(arg: &str) -> Result<u32, <u32 as FromStr>::Err> {
    let multiplier = if arg.ends_with("K") {
        bytesize::KIB as u32
    } else if arg.ends_with("M") {
        bytesize::MIB as u32
    } else {
        1
    };
    arg.trim_end_matches(['K', 'M']).parse().map(|out: u32| out * multiplier)
}

pub fn parse_compressor(arg: &str) -> Result<Compressor, &'static str> {
    match arg {
        "gzip" => Ok(Compressor::Gzip),
        "lzo" => Ok(Compressor::Lzo),
        "lz4" => Ok(Compressor::Lz4),
        "xz" => Ok(Compressor::Xz),
        "zstd" => Ok(Compressor::Zstd),
        _ => Err("Invalid compressor! Possible values are: gzip, lzo, lz4, xz, zstd"),
    }
}

pub fn parse_octal(arg: &str) -> Result<u16, ParseIntError> {
    u16::from_str_radix(arg, 8)
}

pub fn parse_uid(arg: &str) -> Result<u32, String> {
    let uid = match arg.parse::<u32>() {
        Ok(uid) => uid,
        #[cfg(target_family = "unix")]
        Err(_e) => {
            let passwd = unsafe { libc::getpwnam(CString::new(arg).unwrap().as_ptr()) };
            if passwd.is_null() {
                return Err(format!("Invalid uid or username {arg}"));
            }
            unsafe { (*passwd).pw_uid }
        }
        #[cfg(not(target_family = "unix"))]
        Err(_e) => return Err(format!("Invalid uid {arg}: e")),
    };
    Ok(uid)
}

pub fn parse_gid(arg: &str) -> Result<u32, String> {
    let gid = match arg.parse::<u32>() {
        Ok(gid) => gid,
        #[cfg(target_family = "unix")]
        Err(_e) => {
            let passwd = unsafe { libc::getgrnam(CString::new(arg).unwrap().as_ptr()) };
            if passwd.is_null() {
                return Err(format!("Invalid gid or group name {arg}"));
            }
            unsafe { (*passwd).gr_gid }
        }
        #[cfg(not(target_family = "unix"))]
        Err(_e) => return Err(format!("Invalid gid {arg}: e")),
    };
    Ok(gid)
}

pub fn parse_xz_filter(arg: &str) -> Result<XzFilter, String> {
    let mut filter = XzFilter::default();
    let filters = arg.split(',');
    for filter_str in filters.map(str::trim) {
        match filter_str {
            "x86" => filter.with_x86(),
            "arm" => filter.with_arm(),
            "armthumb" => filter.with_armthumb(),
            "powerpc" => filter.with_powerpc(),
            "sparc" => filter.with_sparc(),
            "ia64" => filter.with_ia64(),
            _ => Err(format!("Invalid branch/call/jump filter {filter_str}"))?,
        }
    }

    Ok(filter)
}

pub fn parse_xz_dict_size(arg: &str) -> Result<u32, String> {
    let multiplier = if arg.ends_with("K") {
        bytesize::KIB as u32
    } else if arg.ends_with("M") {
        bytesize::MIB as u32
    } else {
        1
    };
    let number = arg
        .trim_end_matches(['K', 'M'])
        .parse()
        .map(|out: u32| out * multiplier)
        .map_err(|e| format!("Invalid dict size {arg}: {e}"))?;

    if (number as u64) < bytesize::KIB * 8 {
        Err(format!("Invalid dict size {arg}. The dict size must be at least 8KiB"))
    } else {
        // Dict size must be either 2^n or 3*2^n
        let is_power_2 = number & (number - 1) == 0;
        let is_power_2_times_3 = (number / 3) & (number / 3 - 1) == 0;
        if is_power_2 && is_power_2_times_3 {
            Ok(number)
        } else {
            Err(format!("Invalid dict size {arg}. The dict size must be a power of two or a power of two multiplied by 3"))
        }
    }
}

pub fn parse_window_size(arg: &str) -> Result<u16, String> {
    let value = arg.parse::<u16>().map_err(|e| format!("Invalid window size {arg}: {e}"))?;
    if !(1..=15).contains(&value) {
        Err(format!("Invalid window size {value}. Window size must be in the range [1, 15]"))
    } else {
        Ok(value)
    }
}
