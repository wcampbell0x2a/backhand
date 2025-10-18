//! Shared traits for v3 and v4 SquashFS implementations

pub mod compression;
pub mod error;
pub mod filesystem;
pub mod squashfs;
pub mod types;

#[cfg(feature = "v3")]
pub use compression::CompressionActionV3;
pub use compression::{CompressionAction, CompressionActionV4};
pub use error::BackhandError;
pub use filesystem::FilesystemReaderTrait;
pub use squashfs::{GenericSquashfs, SquashfsVersion};
pub use types::Compressor;
