//! Shared traits for v3 and v4 SquashFS implementations

pub mod block_reader;
#[cfg(not(feature = "parallel"))]
pub mod block_reader_no_parallel;
#[cfg(feature = "parallel")]
pub mod block_reader_parallel;
pub mod compression;
pub mod error;
pub mod filesystem;
pub mod squashfs;
pub mod types;

pub use compression::CompressionAction;
pub use error::BackhandError;
pub use filesystem::FilesystemReaderTrait;
pub use squashfs::{GenericSquashfs, SquashfsVersion};
pub use types::Compressor;
