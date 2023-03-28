//! Types of image formats

use std::str::FromStr;

/// Kind Magic - First 4 bytes of image
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Magic {
    /// Little Endian `b"hsqs"`
    Little,
    /// Big Endian `b"sqsh"`
    Big,
}

impl Magic {
    fn magic(self) -> [u8; 4] {
        match self {
            Self::Little => *b"hsqs",
            Self::Big => *b"sqsh",
        }
    }
}

/// Kind Endian
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Endian {
    Little,
    Big,
}

/// Version of SquashFS, also supporting custom changes to SquashFS seen in 3rd-party firmware
///
/// See [Kind Constants](`crate::kind#constants`) for a list of custom Kinds
// TODO: we probably want a `from_reader` for this, so they can get a `Kind` from the magic bytes.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Kind {
    /// Magic at the beginning of the image
    pub(crate) magic: [u8; 4],
    /// Endian used for all data types
    pub(crate) type_endian: deku::ctx::Endian,
    /// Endian used for Metadata Lengths
    pub(crate) data_endian: deku::ctx::Endian,
    /// Major version
    pub(crate) version_major: u16,
    /// Minor version
    pub(crate) version_minor: u16,
}

impl FromStr for Kind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "avm_be_v4_0" => Ok(AVM_BE_V4_0),
            "be_v4_0" => Ok(BE_V4_0),
            "le_v4_0" => Ok(LE_V4_0),
            _ => Err("not a valid kind".to_string()),
        }
    }
}

impl Kind {
    /// Create with default Kind: [`LE_V4_0`]
    pub fn new() -> Self {
        LE_V4_0
    }

    /// Set magic type at the beginning of the image
    pub fn with_magic(mut self, magic: Magic) -> Self {
        self.magic = magic.magic();
        self
    }

    /// Set endian used for data types
    pub fn with_type_endian(mut self, endian: Endian) -> Self {
        match endian {
            Endian::Little => {
                self.type_endian = deku::ctx::Endian::Little;
            },
            Endian::Big => {
                self.type_endian = deku::ctx::Endian::Big;
            },
        }
        self
    }

    /// Set endian used for Metadata lengths
    pub fn with_data_endian(mut self, endian: Endian) -> Self {
        match endian {
            Endian::Little => {
                self.data_endian = deku::ctx::Endian::Little;
            },
            Endian::Big => {
                self.data_endian = deku::ctx::Endian::Big;
            },
        }
        self
    }

    /// Set both type and data endian
    pub fn with_all_endian(mut self, endian: Endian) -> Self {
        match endian {
            Endian::Little => {
                self.type_endian = deku::ctx::Endian::Little;
                self.data_endian = deku::ctx::Endian::Little;
            },
            Endian::Big => {
                self.type_endian = deku::ctx::Endian::Big;
                self.data_endian = deku::ctx::Endian::Big;
            },
        }
        self
    }

    /// Set major and minor version
    pub fn with_version(mut self, major: u16, minor: u16) -> Self {
        self.version_major = major;
        self.version_minor = minor;
        self
    }
}

impl Default for Kind {
    /// Same as [`Self::new`]
    fn default() -> Self {
        Self::new()
    }
}

/// Default `Kind` for linux kernel and squashfs-tools/mksquashfs. Little-Endian v4.0
pub const LE_V4_0: Kind = Kind {
    magic: *b"hsqs",
    type_endian: deku::ctx::Endian::Little,
    data_endian: deku::ctx::Endian::Little,
    version_major: 4,
    version_minor: 0,
};

/// Big-Endian Superblock v4.0
pub const BE_V4_0: Kind = Kind {
    magic: *b"sqsh",
    type_endian: deku::ctx::Endian::Big,
    data_endian: deku::ctx::Endian::Big,
    version_major: 4,
    version_minor: 0,
};

/// AVM Fritz!OS firmware support. Tested with: <https://github.com/dnicolodi/squashfs-avm-tools>
pub const AVM_BE_V4_0: Kind = Kind {
    magic: *b"sqsh",
    type_endian: deku::ctx::Endian::Big,
    data_endian: deku::ctx::Endian::Little,
    version_major: 4,
    version_minor: 0,
};
