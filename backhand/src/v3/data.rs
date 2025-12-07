use deku::prelude::*;

const DATA_STORED_UNCOMPRESSED: u32 = 1 << 24;

const DATA_STORED_UNCOMPRESSED_V3: u16 = 1 << 15;

#[derive(Copy, Clone, Debug, PartialEq, Eq, DekuRead, DekuSize)]
#[deku(
    endian = "endian",
    ctx = "endian: deku::ctx::Endian, order: deku::ctx::Order",
    bit_order = "order"
)]
pub struct DataSize(u32);

impl DataSize {
    #[inline]
    pub fn new(size: u32, uncompressed: bool) -> Self {
        let mut value: u32 = size;
        if value > DATA_STORED_UNCOMPRESSED {
            panic!("value is too big");
        }
        if uncompressed {
            value |= DATA_STORED_UNCOMPRESSED;
        }
        Self(value)
    }

    #[inline]
    pub fn new_compressed(size: u32) -> Self {
        Self::new(size, false)
    }

    #[inline]
    pub fn new_uncompressed(size: u32) -> Self {
        Self::new(size, true)
    }

    #[inline]
    pub fn uncompressed(&self) -> bool {
        self.0 & DATA_STORED_UNCOMPRESSED != 0
    }

    #[inline]
    pub fn set_uncompressed(&mut self) {
        self.0 |= DATA_STORED_UNCOMPRESSED
    }

    #[inline]
    pub fn set_compressed(&mut self) {
        self.0 &= !DATA_STORED_UNCOMPRESSED
    }

    #[inline]
    pub fn size(&self) -> u32 {
        self.0 & !DATA_STORED_UNCOMPRESSED
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, DekuRead, DekuSize)]
#[deku(
    endian = "endian",
    ctx = "endian: deku::ctx::Endian, order: deku::ctx::Order",
    bit_order = "order"
)]
pub struct DataSizeV3(u16);

impl DataSizeV3 {
    #[inline]
    pub fn new(size: u16, uncompressed: bool) -> Self {
        let mut value: u16 = size;
        if value > DATA_STORED_UNCOMPRESSED_V3 {
            panic!("value is too big");
        }
        if uncompressed {
            value |= DATA_STORED_UNCOMPRESSED_V3;
        }
        Self(value)
    }

    #[inline]
    pub fn new_compressed(size: u16) -> Self {
        Self::new(size, false)
    }

    #[inline]
    pub fn new_uncompressed(size: u16) -> Self {
        Self::new(size, true)
    }

    #[inline]
    pub fn uncompressed(&self) -> bool {
        self.0 & DATA_STORED_UNCOMPRESSED_V3 != 0
    }

    #[inline]
    pub fn set_uncompressed(&mut self) {
        self.0 |= DATA_STORED_UNCOMPRESSED_V3
    }

    #[inline]
    pub fn set_compressed(&mut self) {
        self.0 &= !DATA_STORED_UNCOMPRESSED_V3
    }

    #[inline]
    pub fn size(&self) -> u32 {
        (self.0 & !DATA_STORED_UNCOMPRESSED_V3) as u32
    }
}

impl From<DataSizeV3> for DataSize {
    fn from(v3: DataSizeV3) -> Self {
        DataSize::new(v3.size(), v3.uncompressed())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Added {
    // Only Data was added
    Data { blocks_start: u32, block_sizes: Vec<DataSize> },
    // Only Fragment was added
    Fragment { frag_index: u32, block_offset: u32 },
}
