//! Shared traits for v3 and v4 SquashFS implementations

/// Compression trait definitions
pub mod compression;
/// Error types
pub mod error;
/// Version-independent filesystem types
pub mod filesystem;
/// Version-generic SquashFS types
pub mod squashfs;
/// Shared type definitions
pub mod types;

#[cfg(feature = "v3")]
pub use compression::CompressionActionV3;
pub use compression::{CompressionAction, CompressionActionV4};
pub use error::BackhandError;
pub use filesystem::FilesystemReaderTrait;
pub use squashfs::{GenericSquashfs, SquashfsVersion};
pub use types::Compressor;
