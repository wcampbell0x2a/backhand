//! Types of image formats

use core::fmt;
use std::sync::Arc;

use crate::traits::CompressionAction;
#[cfg(feature = "v3")]
use crate::v3::compressor::DefaultCompressor as V3DefaultCompressor;
#[cfg(feature = "v3_lzma")]
use crate::v3_lzma::compressor::LzmaAdaptiveCompressor as V3LzmaCompressor;
#[cfg(feature = "v3_lzma")]
use crate::v3_lzma::standard_compressor::LzmaStandardCompressor as V3LzmaStandardCompressor;
use crate::v4::compressor::DefaultCompressor as V4DefaultCompressor;

// Static instances of compressors
#[cfg(feature = "v3_lzma")]
static V3_LZMA_STANDARD_COMPRESSOR: V3LzmaStandardCompressor = V3LzmaStandardCompressor;

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

/// Version-specific compressor types
#[derive(Clone)]
pub enum VersionedCompressor {
    #[cfg(feature = "v3")]
    V3(&'static V3DefaultCompressor),
    #[cfg(feature = "v3_lzma")]
    V3Lzma(&'static V3LzmaCompressor),
    #[cfg(feature = "v3_lzma")]
    V3LzmaStandard(&'static V3LzmaStandardCompressor),
    V4(&'static V4DefaultCompressor),
    /// Custom v4 compressor
    CustomV4(
        &'static (
                     dyn crate::traits::CompressionAction<
            Error = crate::BackhandError,
            Compressor = crate::v4::compressor::Compressor,
            FilesystemCompressor = crate::v4::filesystem::writer::FilesystemCompressor,
            SuperBlock = crate::v4::squashfs::SuperBlock,
        > + Send
                         + Sync
                 ),
    ),
}

impl VersionedCompressor {
    /// Decompress data using the version-specific compressor
    pub fn decompress(
        &self,
        bytes: &[u8],
        out: &mut Vec<u8>,
        compressor: Option<crate::traits::types::Compressor>,
    ) -> Result<(), crate::BackhandError> {
        match self {
            #[cfg(feature = "v3")]
            VersionedCompressor::V3(comp) => comp.decompress(bytes, out, None),
            #[cfg(feature = "v3_lzma")]
            VersionedCompressor::V3Lzma(comp) => comp.decompress(bytes, out, None),
            #[cfg(feature = "v3_lzma")]
            VersionedCompressor::V3LzmaStandard(comp) => comp.decompress(bytes, out, None),
            VersionedCompressor::V4(comp) => {
                let v4_compressor =
                    compressor.ok_or(crate::BackhandError::MissingCompressor)?.into();
                comp.decompress(bytes, out, v4_compressor)
            }
            VersionedCompressor::CustomV4(comp) => {
                let v4_compressor =
                    compressor.ok_or(crate::BackhandError::MissingCompressor)?.into();
                comp.decompress(bytes, out, v4_compressor)
            }
        }
    }
}

pub struct InnerKind {
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
    /// Version-specific compression impl
    pub(crate) compressor: VersionedCompressor,
    /// v3 needs the bit-order for reading with little endian
    /// v4 does not need this field
    #[allow(dead_code)]
    pub(crate) bit_order: Option<deku::ctx::Order>,
}

/// Version of SquashFS, also supporting custom changes to SquashFS seen in 3rd-party firmware
///
/// See [Kind Constants](`crate::kind#constants`) for a list of custom Kinds
#[derive(Clone)]
pub struct Kind {
    /// "Easier for the eyes" type for the real Kind
    pub(crate) inner: Arc<InnerKind>,
}

impl fmt::Debug for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FilesystemWriter")
            .field("magic", &self.inner.magic)
            .field("type_endian", &self.inner.type_endian)
            .field("data_endian", &self.inner.data_endian)
            .field("version_major", &self.inner.version_major)
            .field("version_minor", &self.inner.version_minor)
            .finish()
    }
}

impl Kind {
    /// Create a new Kind with a custom v4 compressor (defaults to LE_V4_0)
    ///
    /// # Example
    /// ```rust,ignore
    /// # use backhand::{kind::Kind, compression::CompressionAction};
    /// struct MyCompressor;
    /// impl CompressionAction for MyCompressor {
    ///     // ... implementation
    /// }
    /// static MY_COMPRESSOR: MyCompressor = MyCompressor;
    /// let kind = Kind::new_v4(&MY_COMPRESSOR);
    /// ```
    pub fn new_v4<C>(compression: &'static C) -> Self
    where
        C: crate::traits::CompressionAction<
                Error = crate::BackhandError,
                Compressor = crate::v4::compressor::Compressor,
                FilesystemCompressor = crate::v4::filesystem::writer::FilesystemCompressor,
                SuperBlock = crate::v4::squashfs::SuperBlock,
            > + Send
            + Sync,
    {
        Kind {
            inner: Arc::new(InnerKind {
                magic: LE_V4_0.magic,
                type_endian: LE_V4_0.type_endian,
                data_endian: LE_V4_0.data_endian,
                version_major: LE_V4_0.version_major,
                version_minor: LE_V4_0.version_minor,
                compressor: VersionedCompressor::CustomV4(compression),
                bit_order: LE_V4_0.bit_order,
            }),
        }
    }

    /// Create a Kind from a const with a custom v4 compressor
    ///
    /// # Example
    /// ```rust,ignore
    /// # use backhand::{kind::{self, Kind}, compression::CompressionAction};
    /// struct MyCompressor;
    /// impl CompressionAction for MyCompressor {
    ///     // ... implementation
    /// }
    /// static MY_COMPRESSOR: MyCompressor = MyCompressor;
    /// let kind = Kind::new_v4_with_const(&MY_COMPRESSOR, kind::BE_V4_0);
    /// ```
    pub fn new_v4_with_const<C>(compression: &'static C, inner: InnerKind) -> Self
    where
        C: crate::traits::CompressionAction<
                Error = crate::BackhandError,
                Compressor = crate::v4::compressor::Compressor,
                FilesystemCompressor = crate::v4::filesystem::writer::FilesystemCompressor,
                SuperBlock = crate::v4::squashfs::SuperBlock,
            > + Send
            + Sync,
    {
        Kind {
            inner: Arc::new(InnerKind {
                magic: inner.magic,
                type_endian: inner.type_endian,
                data_endian: inner.data_endian,
                version_major: inner.version_major,
                version_minor: inner.version_minor,
                compressor: VersionedCompressor::CustomV4(compression),
                bit_order: inner.bit_order,
            }),
        }
    }

    /// From a string, return a kind
    ///
    /// # Example
    /// Get a default [`Kind`]
    /// ```rust
    /// # use backhand::{kind, kind::Kind};
    /// let kind = Kind::from_target("le_v4_0").unwrap();
    /// ```
    pub fn from_target(s: &str) -> Result<Kind, String> {
        let kind = match s {
            "be_v4_0" => BE_V4_0,
            "le_v4_0" => LE_V4_0,
            "avm_be_v4_0" => AVM_BE_V4_0,
            #[cfg(feature = "v3")]
            "be_v3_0" => BE_V3_0,
            #[cfg(feature = "v3")]
            "le_v3_0" => LE_V3_0,
            #[cfg(feature = "v3_lzma")]
            "le_v3_0_lzma" => LE_V3_0_LZMA,
            #[cfg(feature = "v3_lzma")]
            "be_v3_0_lzma" => BE_V3_0_LZMA,
            #[cfg(feature = "v3_lzma")]
            "netgear_be_v3_0_lzma" => NETGEAR_BE_V3_0_LZMA,
            #[cfg(feature = "v3_lzma")]
            "netgear_be_v3_0_lzma_standard" => NETGEAR_BE_V3_0_LZMA_STANDARD,
            _ => return Err("not a valid kind".to_string()),
        };

        Ok(Kind { inner: Arc::new(kind) })
    }

    /// From a known Squashfs image Kind, return a [`Kind`]
    ///
    /// # Example
    /// Get a default [`Kind`]
    ///
    /// ```rust
    /// # use backhand::{kind, kind::Kind};
    /// let kind = Kind::from_const(kind::LE_V4_0).unwrap();
    /// ```
    pub fn from_const(inner: InnerKind) -> Result<Kind, String> {
        Ok(Kind { inner: Arc::new(inner) })
    }

    // TODO: example
    pub fn from_kind(kind: &Kind) -> Kind {
        Self { inner: kind.inner.clone() }
    }

    /// Set magic type at the beginning of the image
    // TODO: example
    pub fn with_magic(mut self, magic: Magic) -> Self {
        Arc::get_mut(&mut self.inner).unwrap().magic = magic.magic();
        self
    }

    pub fn magic(&self) -> [u8; 4] {
        self.inner.magic
    }

    /// Get major version
    pub fn version_major(&self) -> u16 {
        self.inner.version_major
    }

    /// Get minor version
    pub fn version_minor(&self) -> u16 {
        self.inner.version_minor
    }

    /// Set endian used for data types
    // TODO: example
    pub fn with_type_endian(mut self, endian: Endian) -> Self {
        match endian {
            Endian::Little => {
                Arc::get_mut(&mut self.inner).unwrap().type_endian = deku::ctx::Endian::Little;
            }
            Endian::Big => {
                Arc::get_mut(&mut self.inner).unwrap().type_endian = deku::ctx::Endian::Big;
            }
        }
        self
    }

    /// Set endian used for Metadata lengths
    // TODO: example
    pub fn with_data_endian(mut self, endian: Endian) -> Self {
        match endian {
            Endian::Little => {
                Arc::get_mut(&mut self.inner).unwrap().data_endian = deku::ctx::Endian::Little;
            }
            Endian::Big => {
                Arc::get_mut(&mut self.inner).unwrap().data_endian = deku::ctx::Endian::Big;
            }
        }
        self
    }

    /// Set both type and data endian
    // TODO: example
    pub fn with_all_endian(mut self, endian: Endian) -> Self {
        match endian {
            Endian::Little => {
                Arc::get_mut(&mut self.inner).unwrap().type_endian = deku::ctx::Endian::Little;
                Arc::get_mut(&mut self.inner).unwrap().data_endian = deku::ctx::Endian::Little;
            }
            Endian::Big => {
                Arc::get_mut(&mut self.inner).unwrap().type_endian = deku::ctx::Endian::Big;
                Arc::get_mut(&mut self.inner).unwrap().data_endian = deku::ctx::Endian::Big;
            }
        }
        self
    }

    /// Set major and minor version
    // TODO: example
    pub fn with_version(mut self, major: u16, minor: u16) -> Self {
        Arc::get_mut(&mut self.inner).unwrap().version_major = major;
        Arc::get_mut(&mut self.inner).unwrap().version_minor = minor;
        self
    }
}

/// Default `Kind` for linux kernel and squashfs-tools/mksquashfs. Little-Endian v4.0
pub const LE_V4_0: InnerKind = InnerKind {
    magic: *b"hsqs",
    type_endian: deku::ctx::Endian::Little,
    data_endian: deku::ctx::Endian::Little,
    version_major: 4,
    version_minor: 0,
    compressor: VersionedCompressor::V4(&V4DefaultCompressor),
    bit_order: None,
};

/// Big-Endian Superblock v4.0
pub const BE_V4_0: InnerKind = InnerKind {
    magic: *b"sqsh",
    type_endian: deku::ctx::Endian::Big,
    data_endian: deku::ctx::Endian::Big,
    version_major: 4,
    version_minor: 0,
    compressor: VersionedCompressor::V4(&V4DefaultCompressor),
    bit_order: None,
};

/// AVM Fritz!OS firmware support. Tested with: <https://github.com/dnicolodi/squashfs-avm-tools>
pub const AVM_BE_V4_0: InnerKind = InnerKind {
    magic: *b"sqsh",
    type_endian: deku::ctx::Endian::Big,
    data_endian: deku::ctx::Endian::Little,
    version_major: 4,
    version_minor: 0,
    compressor: VersionedCompressor::V4(&V4DefaultCompressor),
    bit_order: None,
};

/// Default `Kind` for SquashFS v3.0 Little-Endian
#[cfg(feature = "v3")]
pub const LE_V3_0: InnerKind = InnerKind {
    magic: *b"hsqs",
    type_endian: deku::ctx::Endian::Little,
    data_endian: deku::ctx::Endian::Little,
    version_major: 3,
    version_minor: 0,
    compressor: VersionedCompressor::V3(&V3DefaultCompressor),
    bit_order: Some(deku::ctx::Order::Lsb0),
};

/// Big-Endian SquashFS v3.0
#[cfg(feature = "v3")]
pub const BE_V3_0: InnerKind = InnerKind {
    magic: *b"sqsh",
    type_endian: deku::ctx::Endian::Big,
    data_endian: deku::ctx::Endian::Big,
    version_major: 3,
    version_minor: 0,
    compressor: VersionedCompressor::V3(&V3DefaultCompressor),
    bit_order: Some(deku::ctx::Order::Msb0),
};

/// Little-Endian SquashFS v3.0 with LZMA compression
#[cfg(feature = "v3_lzma")]
pub const LE_V3_0_LZMA: InnerKind = InnerKind {
    magic: *b"hsqs",
    type_endian: deku::ctx::Endian::Little,
    data_endian: deku::ctx::Endian::Little,
    version_major: 3,
    version_minor: 0,
    compressor: VersionedCompressor::V3Lzma(&V3LzmaCompressor),
    bit_order: Some(deku::ctx::Order::Lsb0),
};

/// Big-Endian SquashFS v3.0 with LZMA compression
#[cfg(feature = "v3_lzma")]
pub const BE_V3_0_LZMA: InnerKind = InnerKind {
    magic: *b"sqsh",
    type_endian: deku::ctx::Endian::Big,
    data_endian: deku::ctx::Endian::Big,
    version_major: 3,
    version_minor: 0,
    compressor: VersionedCompressor::V3Lzma(&V3LzmaCompressor),
    bit_order: Some(deku::ctx::Order::Msb0),
};

/// Big-Endian SquashFS v3.0 with LZMA compression for Netgear
#[cfg(feature = "v3_lzma")]
pub const NETGEAR_BE_V3_0_LZMA: InnerKind = InnerKind {
    magic: *b"qshs",
    type_endian: deku::ctx::Endian::Big,
    data_endian: deku::ctx::Endian::Big,
    version_major: 3,
    version_minor: 0,
    compressor: VersionedCompressor::V3Lzma(&V3LzmaCompressor),
    bit_order: Some(deku::ctx::Order::Msb0),
};

/// Big-Endian SquashFS v3.0 with LZMA standard compression for Netgear
#[cfg(feature = "v3_lzma")]
pub const NETGEAR_BE_V3_0_LZMA_STANDARD: InnerKind = InnerKind {
    magic: *b"qshs",
    type_endian: deku::ctx::Endian::Big,
    data_endian: deku::ctx::Endian::Big,
    version_major: 3,
    version_minor: 0,
    compressor: VersionedCompressor::V3LzmaStandard(&V3_LZMA_STANDARD_COMPRESSOR),
    bit_order: Some(deku::ctx::Order::Msb0),
};
