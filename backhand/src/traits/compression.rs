//! Shared compression trait for v3 and v4 implementations

/// Custom Compression support trait that both v3 and v4 can implement
///
/// For most instances, one should just use the [`DefaultCompressor`]. This will correctly
/// implement the Squashfs found within `squashfs-tools` and the Linux kernel.
///
/// However, the "wonderful world of vendor formats" has other ideas and has implemented their own
/// ideas of compression with custom tables and such! Thus, if the need arises you can implement
/// your own [`CompressionAction`] to override the compression and de-compression used in this
/// library by default.
pub trait CompressionAction {
    /// The error type for compression operations
    type Error;
    /// The compressor type for this version
    type Compressor;
    /// The filesystem compressor type for this version
    type FilesystemCompressor;
    /// The superblock type for this version
    type SuperBlock;

    /// Decompress function used for all decompression actions
    ///
    /// # Arguments
    ///
    /// * `bytes` - Input compressed bytes
    /// * `out` - Output uncompressed bytes. You will need to call `out.resize(out.capacity(), 0)`
    ///   if your compressor relies on having a max sized buffer to write into.
    /// * `compressor` - Compressor id from SuperBlock. This can be ignored if your custom
    ///   compressor doesn't follow the normal values of the Compressor Id.
    fn decompress(
        &self,
        bytes: &[u8],
        out: &mut Vec<u8>,
        compressor: Self::Compressor,
    ) -> Result<(), Self::Error>;

    /// Compression function used for all compression actions
    ///
    /// # Arguments
    /// * `bytes` - Input uncompressed bytes
    /// * `fc` - Information from both the derived image and options added during compression
    /// * `block_size` - Block size from SuperBlock
    fn compress(
        &self,
        bytes: &[u8],
        fc: Self::FilesystemCompressor,
        block_size: u32,
    ) -> Result<Vec<u8>, Self::Error>;

    /// Compression Options for non-default compression specific options
    ///
    /// This function is called when calling FilesystemWriter::write, and the returned bytes are the
    ///  section right after the SuperBlock.
    ///
    /// # Arguments
    /// * `superblock` - Mutatable squashfs superblock info that will be written to disk after
    ///   this function is called. The fields `inode_count`, `block_size`,
    ///   `block_log` and `mod_time` *will* be set to `FilesystemWriter` options and can be trusted
    ///   in this function.
    /// * `kind` - Kind information
    /// * `fs_compressor` - Compression Options
    fn compression_options(
        &self,
        _superblock: &mut Self::SuperBlock,
        _kind: &crate::kinds::Kind,
        _fs_compressor: Self::FilesystemCompressor,
    ) -> Result<Option<Vec<u8>>, Self::Error> {
        // Default implementation returns None (no compression options)
        Ok(None)
    }
}

/// Simple compression trait for use in the Kind system
/// This avoids version-specific associated types
pub trait SimpleCompression {
    type Error;

    /// Decompress using unified compressor type
    fn decompress(
        &self,
        bytes: &[u8],
        out: &mut Vec<u8>,
        compressor: super::types::Compressor,
    ) -> Result<(), Self::Error>;

    /// Compress using unified compressor type (simplified version)
    fn compress(
        &self,
        bytes: &[u8],
        compressor: super::types::Compressor,
        block_size: u32,
    ) -> Result<Vec<u8>, Self::Error>;

    /// Compression options (simplified version)
    fn compression_options(
        &self,
        compressor: super::types::Compressor,
        kind: &crate::kinds::Kind,
    ) -> Result<Vec<u8>, Self::Error>;
}

/// Type alias for v4 CompressionAction trait objects  
///
/// This allows using CompressionAction as a trait object with concrete v4 types
pub type CompressionActionV4 = dyn CompressionAction<
    Error = crate::error::BackhandError,
    Compressor = crate::v4::compressor::Compressor,
    FilesystemCompressor = crate::v4::filesystem::writer::FilesystemCompressor,
    SuperBlock = crate::v4::squashfs::SuperBlock,
>;
