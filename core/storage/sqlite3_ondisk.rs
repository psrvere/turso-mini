use std::{pin::Pin, sync::Arc};

use crate::{bail_corrupt_error, error::TursoMiniError, io::Buffer, storage::btree::offset::{BTREE_CELL_CONTENT_AREA, BTREE_CELL_COUNT, BTREE_FIRST_FREEBLOCK, BTREE_FRAGMENTED_BYTES_COUNT, BTREE_PAGE_TYPE, BTREE_RIGHTMOST_PTR}, Result};
use pack1::{U16BE};

pub const CELL_PTR_SIZE_BYTES: usize = 2;
pub const INTERIOR_PAGE_HEADER_SIZE_BYTES: usize = 12;
pub const LEAF_PAGE_HEADER_SIZE_BYTES: usize = 8;

pub struct PageSize(U16BE);

impl PageSize {
    pub const MIN: u32 = 512;
    pub const MAX: u32 = 65536;
    pub const DEFAULT: u16 = 4096;

    // const functions are evaluated at compile time
    // This fn has no heap allocation, no side effects, simple bit operations
    // And hence is a suitable candidate for const fn
    pub const fn new(size: u32) -> Option<Self> {
        if size < PageSize::MIN || size > PageSize::MAX {
            return None;
        }

        // Page size must be power of 2
        if size.count_ones() != 1 {
            return None;
        }

        if size == PageSize::MAX {
            // Internally, the value of 1 represents 65536
            // page size space is 2 bytes (u16) in DB header, which have max value of 65535
            return Some(Self(U16BE::new(1)));
        }

        Some(Self(U16BE::new(size as u16)))
    }

    pub fn new_from_header_u16(value: u16) -> Result<Self> {
        match value {
            1 => Ok(Self(U16BE::new(1))),
            n => {
                let Some(size) = Self::new(n as u32) else {
                    bail_corrupt_error!("invalid page size in database header: {n}")
                };
                Ok(size)
            }
        }
    }

    pub const fn get(self) -> u32 {
        match self.0.get() {
            1 => Self::MAX,
            n => n as u32,
        }
    }

    pub const fn get_raw(self) -> u16 {
        self.0.get()
    }
}

impl Default for PageSize {
    fn default() -> Self {
        Self(U16BE::new(Self::DEFAULT))
    }
}

pub enum PageType {
    IndexInterior = 2,
    TableInterior = 5,
    IndexLeaf = 10,
    TableLeaf = 13,
}

impl PageType {
    pub fn is_table(&self) -> bool {
        match  self {
            PageType::IndexInterior | PageType::IndexLeaf => false,
            PageType::TableInterior | PageType::TableLeaf => true,
        }
    }
}

impl TryFrom<u8> for PageType {
    type Error = TursoMiniError;

    fn try_from(value: u8) -> Result<Self> {
        match value {
            2 => Ok(Self::IndexInterior),
            5 => Ok(Self::TableInterior),
            10 => Ok(Self::IndexLeaf),
            13 => Ok(Self::TableLeaf),
            _ => Err(TursoMiniError::Corrupt(format!("Invalid page type: {value}"))),
        }
    }
}

pub struct OverflowCell {
    pub index: usize,
    pub payload: Pin<Vec<u8>>,
}

/* PageContent represents a page in sqlite File
The first page has header of 100bytes (database file header)
All other pages have header of 0 bytes.
This header space is adjusted by offset
*/
pub struct PageContent {
    pub offset: usize,
    pub buffer: Arc<Buffer>,
    pub overflow_cells: Vec<OverflowCell>,
}

impl PageContent {
    pub fn new(offset: usize, buffer: Arc<Buffer>) -> Self {
        Self {
            offset,
            buffer,
            overflow_cells: Vec::new(),
        }
    }

    pub fn page_type(&self) -> PageType {
        // PageType is present just after header
        self.read_u8(BTREE_PAGE_TYPE).try_into().unwrap()
    }

    pub fn maybe_page_type(&self) -> Option<PageType> {
        self.read_u8(0).try_into().ok()
    }

    // Reads any byte at pos bytes after page header
    fn read_u8(&self, pos: usize) -> u8 {
        let buf = self.as_ptr();
        buf[self.offset + pos]
    }

    pub fn as_ptr(&self) -> &mut [u8] {
        self.buffer.as_mut_slice()
    }

    // Read two bytes from the page content at the given offset (pos), accounting for page header (self.offset)
    fn read_u16(&self, pos: usize) -> u16 {
        let buf = self.as_ptr();
        u16::from_be_bytes([buf[self.offset + pos],buf[self.offset + pos + 1]])
    }

    fn read_u32(&self, pos: usize) -> u32 {
        let buf = self.as_ptr();
        read_u32(buf, self.offset + pos)
    }

    fn write_u8(&self, pos: usize, value: u8) {
        let buf = self.as_ptr();
        buf[self.offset + pos] = value;
    }

    fn write_u16(&self, pos: usize, value: u16) {
        let buf = self.as_ptr();
        buf[self.offset + pos..self.offset + pos + 2].copy_from_slice(&value.to_be_bytes());
    }

    fn write_u32(&self, pos: usize, value: u32) {
        let buf = self.as_ptr();
        buf[self.offset + pos..self.offset + pos + 4].copy_from_slice(&value.to_be_bytes());
    }

    pub fn read_u16_no_offset(&self, pos: usize) -> u16 {
        let buf = self.as_ptr();
        u16::from_be_bytes([buf[pos], buf[pos+1]])
    }

    pub fn read_u32_no_offset(&self, pos: usize) -> u32 {
        let buf = self.as_ptr();
        read_u32(buf, pos)
    }

    pub fn write_u16_no_offset(&self, pos: usize, value: u16) {
        let buf = self.as_ptr();
        buf[pos..pos+2].copy_from_slice(&value.to_be_bytes());
    }

    pub fn write_u32_no_offset(&self, pos: usize, value: u32) {
        let buf = self.as_ptr();
        buf[pos..pos+4].copy_from_slice(&value.to_be_bytes());
    }

    pub fn write_page_type(&self, value: u8) {
        self.write_u8(BTREE_PAGE_TYPE, value);
    }

    pub fn write_rightmost_ptr(&self, value: u32) {
        self.write_u32(BTREE_RIGHTMOST_PTR, value);
    }

    pub fn write_first_freeblock(&self, value: u16) {
        self.write_u16(BTREE_FIRST_FREEBLOCK, value);
    }

    pub fn read_first_freeblock(&self) -> u16 {
        self.read_u16(BTREE_FIRST_FREEBLOCK)
    }

    /*
        Freeblocks store location of free blocks on the page and size of each free block
        Freeblocks information is stored in a 4 bytes
        The first two bytes store the absolute offset of the next free block
        The last two bytes store the size of the current free block
        If next_block is zero, there is no next block
    */
    pub fn write_freeblock(&self, offset: u16, next_block: Option<u16>, size: u16) {
        self.write_freeblock_next_ptr(offset, next_block.unwrap_or(0));
        self.write_freeblock_size(offset + 2, size);
    }

    fn write_freeblock_next_ptr(&self, offset: u16, next_block: u16) {
        self.write_u16(offset as usize, next_block);
    }

    fn write_freeblock_size(&self, offset: u16, size: u16) {
        self.write_u16(offset as usize + 2, size);
    }

    pub fn read_freeblock(&self, offset: u16) -> (u16, u16) {
        (
            self.read_u16_no_offset(offset as usize),
            self.read_u16_no_offset(offset as usize + 2)
        )
    }

    pub fn write_cell_count(&self, count: u16) {
        self.write_u16(BTREE_CELL_COUNT, count);
    }

    pub fn read_cell_count(&self) -> u16 {
        self.read_u16(BTREE_CELL_COUNT)
    }

    // zero value for this area is interpreted as 65,536
    pub fn write_cell_content_area(&self, value: u16) {
        self.write_u16(BTREE_CELL_CONTENT_AREA, value);
    }
 
    pub fn write_fragmented_bytes_count(&self, count: u8) {
        self.write_u8(BTREE_FRAGMENTED_BYTES_COUNT, count);
    }

    pub fn header_size(&self) -> usize {
        let is_interior = self.read_u8(BTREE_PAGE_TYPE) <= PageType::TableInterior as u8;
        (is_interior as usize) * INTERIOR_PAGE_HEADER_SIZE_BYTES
            + (!is_interior as usize) * LEAF_PAGE_HEADER_SIZE_BYTES
    }

    pub fn cell_pointer_array_offset_and_size(&self) -> (usize, usize) {
        (
            self.cell_pointer_array_offset(),
            self.cell_pointer_array_size(),
        )
    }

    pub fn cell_pointer_array_offset(&self) -> usize {
        self.offset + self.header_size()
    }

    pub fn cell_pointer_array_size(&self) -> usize {
        self.read_cell_count() as usize * CELL_PTR_SIZE_BYTES
    }

    pub fn unallocated_region_start(&self) -> usize {
        let (cell_ptr_array_start, cell_ptr_array_size) = self.cell_pointer_array_offset_and_size();
        cell_ptr_array_start + cell_ptr_array_size
    }

    pub fn unallocated_region_size(&self) -> usize {
        self.cell_content_area() as usize - self.unallocated_region_start()
    }

    pub fn cell_content_area(&self) -> u32 {
        let offset = self.read_u16(BTREE_CELL_CONTENT_AREA);
        if offset == 0 {
            PageSize::MAX
        } else {
            offset as u32
        }
    }

    /// Total number of fragmented bytes in all the fragments
    pub fn num_frag_free_bytes(&self) -> u8 {
        self.read_u8(BTREE_FRAGMENTED_BYTES_COUNT)
    }

    /// Returns value of rightmost pointer i.e. page number (value) of right most key
    pub fn rightmost_pointer(&self) -> Option<u32> {
        match self.page_type() {
            PageType::IndexInterior | PageType::TableInterior => Some(self.read_u32(BTREE_RIGHTMOST_PTR)),
            PageType::IndexLeaf | PageType::TableLeaf => None,
        }
    }

    /// Returns a pointer to the right most key
    /// Since buffer allocation guarantees page is stored contiguously in physical memory
    /// we can do valid pointer arithmetic
    pub fn rightmost_pointer_raw(&self) -> Option<*mut u8> {
        match self.page_type() {
            PageType::IndexInterior | PageType::TableInterior => Some(unsafe{
                self
                    .as_ptr()
                    .as_mut_ptr()
                    .add(self.offset + BTREE_RIGHTMOST_PTR)
            }),
            PageType::IndexLeaf | PageType::TableLeaf => None,
        }
    }
}

pub fn read_u32(buf: &[u8], pos: usize) -> u32 {
    u32::from_be_bytes([buf[pos], buf[pos+1], buf[pos+2], buf[pos+3]])
}

/*
SQLite uses varint for rowids and keys in B-Trees
varints use between 1 to 9 bytes to be more space efficient
in each byte only lower 7 bits are used and the left most bit is used as a continuation flag
if it's 0, this is the last byte and if it's 1, then there is at least one more lower byte to read
varints are big endian i.e. in buffer the most significant byte is stored first

Q. Why is Big Engian preferred in certain cases?
A. It allows byte by byte coomparision, while comparing two varints, and the comparision can stop at 
first different byte found point i.e. there is no need to fully decode to compare

Q. Why do we need 9 bytes to store 64 bit values?
A. Since, highest bit of every byte is reserved for continuation flag, in 8 bytes we can store only 8*7 = 56 bits.
Hence we need 8 more bits (56 + 8 = 64) to store 64 bits values. Needless to say, 9th byte doesn't have a bit reserved
for continuation flag.
*/

/* From SQLite codebase comments:
** The variable-length integer encoding is as follows:
**
** KEY:
**         A = 0xxxxxxx    7 bits of data and one flag bit
**         B = 1xxxxxxx    7 bits of data and one flag bit
**         C = xxxxxxxx    8 bits of data
**
**  7 bits - A
** 14 bits - BA
** 21 bits - BBA
** 28 bits - BBBA
** 35 bits - BBBBA
** 42 bits - BBBBBA
** 49 bits - BBBBBBA
** 56 bits - BBBBBBBA
** 64 bits - BBBBBBBBC
*/

// write_variant returns size of variant in bytes
pub fn write_varint(buf: &mut [u8], value: u64) -> usize {
    // Fast Paths for handling 1 and 2 bytes values were added as optimisations in SQLite codebase

    // Fast Path to handle 1 byte values: 0 to 127 (2^8-1)
    if value <= 0x7f {
        buf[0] = (value & 0x7f) as u8; // extract lower 7 bits, continuation flag (highest bit) is zero by default
        return 1;
    }

    // Fast path to handle 2 bytes values: 128 (2^8) to 16383 (2^14 - 1)
    if value <= 0x3fff {
        // value >> 7 returns a new value and doesn't modify the original variable
        // value >>=7 using an assignment operator will modify the original variable
        // u64 is copy type, so here these type of variables are copied, not moved
        buf[0] = (((value >> 7) & 0x7f) | 0x80) as u8; // extract higher 7 bits and mark continuation flag as 1 for this byte
        buf[1] = (value & 0x7f) as u8; // extract lower 7 bits, continuation flag (highest bit) is zero by default
        return 2;
    }

    // Handle values which require all the 9 bytes
    let mut value = value;
    if (value & (0xff000000 << 32)) > 0 {
        buf[8] = value as u8; // this will store least significant 8th bits a a full byte
        value >>= 8; // shift out these 8 bits
        for i in (0..8).rev() {
            buf[i] = ((value & 0x7f) | 0x80) as u8; // 
            value >>= 7 // shift out these 7 bits
        }
        return 9;
    }

    // General path for rest of the cases
    let mut encoded: [u8; 10] = [0; 10];
    let mut bytes: u64 = value;
    let mut n = 0;
    while bytes != 0 {
        let v = 0x80 | (bytes & 0x7f); // extract 7 lower bits, and set highest bit to 1
        encoded[n] = v as u8; // Note: byte are stored in little endian order in encoded
        bytes >>= 7; // shift out these 7 bits
        n += 1;
    }
    encoded[0] &= 0x7f; // clear highest bit to 0 i.e. no more bytes after this
    for i in 0..n {
        buf[i] = encoded[n - 1 -i]; // copy values in buffer in big endian order
    }
    n
}

pub fn varint_len(value: u64) -> usize {
    if value <= 0x7f {
        return 1;
    }

    if value <= 0x3fff {
        return 2;
    }

    if (value & (0xFF000000 << 32)) > 0 {
        return 9;
    }

    let mut bytes = value;
    let mut n = 0;
    while bytes != 0 {
        bytes >>= 7;
        n += 1;
    }
    n
}

// read_varint is provided with a slice (buffer) starting at the variant
// the length of buffer may be longer than varint length
// hence using buf.len() is not reliable
// Moreover, the buffer can have additional data not related to varint
// The most reliable way is to check for High Bit
pub fn read_varint(buf: &[u8]) -> Result<(u64, usize)> {
    let mut v: u64 = 0;
    for i in 0..8 {
        match buf.get(i) {
            Some(c) => {
                v = (v << 7) + (c & 0x7f) as u64;
                if (c & 0x80) == 0 {
                    return Ok((v, i + 1));
                }
            }
            None => bail_corrupt_error!("Invalid varint")
        }
    }
    match buf.get(8) {
        Some(&c) => {
            if (v >> 48) == 0 {
                bail_corrupt_error!("Invalid varint");
            }
            v = (v << 8) + c as u64;
            Ok((v, 9))
        }
        None => bail_corrupt_error!("invalid varint")
    }
}