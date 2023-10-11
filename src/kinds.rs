//! Types of image formats

use core::fmt;
use std::sync::Arc;

use crate::{
    compressor::{CompressionAction, Compressor},
    BackhandError,
};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Version {
    V3_0,
    V4_0,
}

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

pub struct InnerKind<C: CompressionAction + ?Sized + 'static + Send + Sync> {
    /// Magic at the beginning of the image
    pub(crate) magic: [u8; 4],
    /// Endian used for all data types
    pub(crate) type_endian: deku::ctx::Endian,
    /// Endian used for Metadata Lengths
    pub(crate) data_endian: deku::ctx::Endian,
    pub(crate) version: Version,
    /// Major version
    pub(crate) version_major: u16,
    /// Minor version
    pub(crate) version_minor: u16,
    /// Compression impl
    pub(crate) compressor: &'static C,
    /// v3 needs the bit-order for reading with little endian
    /// v4 does not need this field
    pub(crate) bit_order: Option<deku::ctx::Order>,
}

/// Version of SquashFS, also supporting custom changes to SquashFS seen in 3rd-party firmware
///
/// See [Kind Constants](`crate::kind#constants`) for a list of custom Kinds
pub struct Kind {
    /// "Easier for the eyes" type for the real Kind
    pub(crate) inner: Arc<InnerKind<dyn CompressionAction + Send + Sync>>,
}

impl fmt::Debug for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FilesystemWriter")
            .field("magic", &self.inner.magic)
            .field("type_endian", &self.inner.type_endian)
            .field("data_endian", &self.inner.data_endian)
            .field("version", &self.inner.version)
            .field("version_major", &self.inner.version_major)
            .field("version_minor", &self.inner.version_minor)
            .finish()
    }
}

impl Kind {
    /// Create [`LE_V4_0`] with custom `compressor`.
    ///
    /// Use other [`Kind`] functions such as [`Kind::with_magic`] to change other settings other than
    /// `compressor`.
    ///
    /// Use [`Kind::new_with_const`] when using a custom compressor with something other than
    /// [`LE_V4_0`].
    ///
    /// # Example
    /// ```rust
    /// # use backhand::{compression::Compressor, kind, FilesystemCompressor, kind::Kind, compression::CompressionAction, compression::DefaultCompressor, BackhandError};
    /// # use std::io::Write;
    /// #[derive(Copy, Clone)]
    /// pub struct CustomCompressor;
    ///
    /// // Special decompress that only has support for the Rust version of gzip: zune-inflate for
    /// // decompression.
    /// impl CompressionAction for CustomCompressor {
    ///     fn decompress(
    ///         &self,
    ///         bytes: &[u8],
    ///         out: &mut Vec<u8>,
    ///         compressor: Compressor,
    ///     ) -> Result<(), BackhandError> {
    ///         if let Compressor::Gzip = compressor {
    ///             let mut decoder = zune_inflate::DeflateDecoder::new(bytes);
    ///             let decompressed_data = decoder.decode_zlib().unwrap();
    ///             out.write_all(&decompressed_data)?;
    ///         } else {
    ///             unimplemented!();
    ///         }
    ///
    ///         Ok(())
    ///     }
    ///
    /// let kind = Kind::new(&CustomCompressor);
    /// ```
    pub fn new<C: CompressionAction + Send + Sync>(compressor: &'static C) -> Self {
        Self {
            inner: Arc::new(InnerKind {
                compressor,
                ..LE_V4_0
            }),
        }
    }

    pub fn new_with_const<C: CompressionAction + Send + Sync>(
        compressor: &'static C,
        c: InnerKind<dyn CompressionAction + Send + Sync>,
    ) -> Self {
        Self {
            inner: Arc::new(InnerKind { compressor, ..c }),
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
    /// # Returns
    /// - `"le_v4_0"`: [`LE_V4_0`]
    /// - `"be_v4_0"`: [`BE_V4_0`]
    /// - `"avm_be_v4_0"`: [`AVM_BE_V4_0`]
    pub fn from_target(s: &str) -> Result<Kind, String> {
        let kind = match s {
            "avm_be_v4_0" => AVM_BE_V4_0,
            "be_v4_0" => BE_V4_0,
            "le_v4_0" => LE_V4_0,
            "le_v3_0" => LE_V3_0,
            "be_v3_0" => BE_V3_0,
            _ => return Err("not a valid kind".to_string()),
        };

        Ok(Kind {
            inner: Arc::new(kind),
        })
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
    pub fn from_const(
        inner: InnerKind<dyn CompressionAction + Send + Sync>,
    ) -> Result<Kind, String> {
        Ok(Kind {
            inner: Arc::new(inner),
        })
    }

    // TODO: example
    pub fn from_kind(kind: &Kind) -> Kind {
        Self {
            inner: kind.inner.clone(),
        }
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
pub const LE_V4_0: InnerKind<dyn CompressionAction + Send + Sync> = InnerKind {
    magic: *b"hsqs",
    type_endian: deku::ctx::Endian::Little,
    data_endian: deku::ctx::Endian::Little,
    version: Version::V4_0,
    version_major: 4,
    version_minor: 0,
    compressor: &crate::compressor::DefaultCompressor,
    bit_order: None,
};

/// Big-Endian Superblock v4.0
pub const BE_V4_0: InnerKind<dyn CompressionAction + Send + Sync> = InnerKind {
    magic: *b"sqsh",
    type_endian: deku::ctx::Endian::Big,
    data_endian: deku::ctx::Endian::Big,
    version: Version::V4_0,
    version_major: 4,
    version_minor: 0,
    compressor: &crate::compressor::DefaultCompressor,
    bit_order: None,
};

/// AVM Fritz!OS firmware support. Tested with: <https://github.com/dnicolodi/squashfs-avm-tools>
pub const AVM_BE_V4_0: InnerKind<dyn CompressionAction + Send + Sync> = InnerKind {
    magic: *b"sqsh",
    type_endian: deku::ctx::Endian::Big,
    data_endian: deku::ctx::Endian::Little,
    version: Version::V4_0,
    version_major: 4,
    version_minor: 0,
    compressor: &crate::compressor::DefaultCompressor,
    bit_order: None,
};

/// Little-Endian v3.0
pub const LE_V3_0: InnerKind<dyn CompressionAction + Send + Sync> = InnerKind {
    magic: *b"hsqs",
    type_endian: deku::ctx::Endian::Little,
    data_endian: deku::ctx::Endian::Little,
    version: Version::V3_0,
    version_major: 3,
    version_minor: 0,
    compressor: &crate::compressor::DefaultCompressor, // Only Gzip
    bit_order: Some(deku::ctx::Order::Lsb0),
};

/// Big-Endian v3.0
pub const BE_V3_0: InnerKind<dyn CompressionAction + Send + Sync> = InnerKind {
    magic: *b"sqsh",
    type_endian: deku::ctx::Endian::Big,
    data_endian: deku::ctx::Endian::Big,
    version: Version::V3_0,
    version_major: 3,
    version_minor: 0,
    compressor: &crate::compressor::DefaultCompressor, // Only Gzip
    bit_order: Some(deku::ctx::Order::Msb0),
};
