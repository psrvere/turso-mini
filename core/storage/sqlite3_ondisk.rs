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

    pub fn uallocated_region_start(&self) -> usize {
        let (cell_ptr_array_start, cell_ptr_array_size) = self.cell_pointer_array_offset_and_size();
        cell_ptr_array_start + cell_ptr_array_size
    }
}

pub fn read_u32(buf: &[u8], pos: usize) -> u32 {
    u32::from_be_bytes([buf[pos], buf[pos+1], buf[pos+2], buf[pos+3]])
}