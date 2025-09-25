use std::{cell::{Cell, UnsafeCell}, collections::BTreeMap};
use std::sync::Arc;
use crate::{Completion, File, Result, Buffer};

const PAGE_SIZE: usize = 4096;
type MemPage = Box<[u8; PAGE_SIZE]>;

// Q. Why wrap size in Cell<>
// To have interior mutability in the API. Most file operations will need to just
// read file size and a writing operation will set file size.

// Q. Cell vs UnsafeCell?
// Cell only works with copy types like u64
// UnsafeCell works with any type T
pub struct MemoryFile {
    path: String,
    pages: UnsafeCell<BTreeMap<usize, MemPage>>,
    size: Cell<u64>,
}

unsafe impl Sync for MemoryFile {}

impl File for MemoryFile {
    fn lock_file(&self) -> Result<()> {
        Ok(())
    }

    fn unlock_file(&self) -> crate::Result<()> {
        Ok(())
    }

    fn size(&self) -> Result<u64> {
        Ok(self.size.get())
    }

    fn pread(&self, pos: u64, c: Completion) -> Result<Completion> {
        let r = c.as_read();
        let buf_len = r.buf.len() as u64;
        if buf_len == 0 {
            c.complete(0);
            return Ok(c);
        }

        let file_size = self.size.get();
        if pos >= file_size {
            c.complete(0);
            return Ok(c);
        }

        let read_len = buf_len.min(file_size - pos);
        {
            let read_buf = r.buf();
            let mut offset = pos as usize;
            let mut remaining = read_len as usize;
            let mut buf_offset = 0;

            while remaining > 0 {
                let page_no = offset / PAGE_SIZE;
                let page_offset = offset % PAGE_SIZE;
                let bytes_to_read = remaining.min(PAGE_SIZE - page_offset);
                if let Some(page) = self.get_page(page_no) {
                    read_buf.as_mut_slice()[buf_offset..buf_offset+bytes_to_read]
                        .copy_from_slice(&page[page_offset..page_offset+bytes_to_read]);
                } else {
                    read_buf.as_mut_slice()[buf_offset..buf_offset + bytes_to_read].fill(0);
                }

                offset += bytes_to_read;
                buf_offset += bytes_to_read;
                remaining -= bytes_to_read;
            }
        }
        c.complete(read_len as i32);
        Ok(c)
    }

    fn pwrite(&self, pos: u64, buffer: Arc<Buffer>, c: Completion) -> Result<Completion> {
        let buf_len = buffer.len();
        if buf_len == 0 {
            c.complete(0);
            return Ok(c)
        }

        let data = &buffer.as_slice();
        let mut offset = pos as usize;
        let mut remaining = buf_len;
        let mut buf_offset = 0;
        
        while remaining > 0 {
            let page_no = offset / PAGE_SIZE;
            let page_offset = offset % PAGE_SIZE;
            let bytes_to_write = remaining.min(PAGE_SIZE - page_offset);
            
            {
                let page = self.get_or_allocate_page(page_no);
                page[page_offset..page_offset+bytes_to_write]
                    .copy_from_slice(&data[buf_offset..buf_offset+bytes_to_write]);
            }
            
            offset += bytes_to_write;
            buf_offset += bytes_to_write;
            remaining -= bytes_to_write;
        }

        self.size
            .set(core::cmp::max(pos + buf_len as u64, self.size.get()));
        c.complete(buf_len as i32);
        Ok(c)
    }

    fn sync(&self, c: Completion) -> Result<Completion> {
        c.complete(0);
        Ok(c)
    }

    fn truncate(&self, len: u64, c: Completion) -> Result<Completion> {
        let file_size = self.size.get();
        if len < file_size {
            unsafe {
                let pages = &mut *self.pages.get();
                pages.retain(|&k, _| k*PAGE_SIZE < len as usize);
            }
        }
        self.size.set(len);
        c.complete(0);
        Ok(c)
    }

    fn pwritev(&self, pos: u64, buffers: Vec<Arc<Buffer>>, c: Completion) -> Result<Completion> {
        if buffers.len() == 0 {
            c.complete(0);
            return Ok(c)
        }

        let mut offset = pos as usize;
        let mut total_written = 0;
        
        for buffer in buffers {
            let buf_len = buffer.len();
            if buf_len == 0 {
                continue;
            }

            let mut remaining = offset;
            let mut buf_offset = 0;
            let data = buffer.as_slice();

            while remaining > 0 {
                let page_no = offset / PAGE_SIZE;
                let page_offset = offset % PAGE_SIZE;
                let bytes_to_write = remaining.min(PAGE_SIZE - page_offset);

                {
                    let page = self.get_or_allocate_page(page_no);
                    page[page_offset..page_offset+bytes_to_write]
                        .copy_from_slice(&data[buf_offset..buf_offset+bytes_to_write]);
                }
                
                offset += bytes_to_write;
                buf_offset += bytes_to_write;
                remaining -= bytes_to_write;
            }
            total_written += buf_len;
        }
        c.complete(total_written as i32);
        self.size
            .set(core::cmp::max(pos + total_written as u64,self.size.get()));
        Ok(c)
    }
}

impl MemoryFile {
    fn get_page(&self, page_no: usize) -> Option<&MemPage> {
        unsafe {(*self.pages.get()).get(&page_no)}
    }

    fn get_or_allocate_page(&self, page_no: usize) -> &mut MemPage {
        unsafe {
            let pages = &mut *self.pages.get();
            pages
                .entry(page_no)
                .or_insert_with(|| Box::new([0; PAGE_SIZE]))
        }
    }
}
