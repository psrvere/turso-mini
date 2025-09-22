use std::pin::Pin;
use std::fmt;

pub enum Buffer {
    Heap(Pin<Box<[u8]>>)
}

impl Buffer {
    /// create a new buffer from a vector
    pub fn new(data: Vec<u8>) -> Self {
        Self::Heap(Pin::new(data.into_boxed_slice()))
    }

    pub fn new_zeroed(size: usize) -> Self {
        Self::Heap(Pin::new(vec![0; size].into_boxed_slice()))
    }

    pub fn len(&self) -> usize {
        match self {
            Self::Heap(buf) => buf.len()
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    // Get a slice reference to the buffer data
    pub fn as_slice(&self) -> &[u8] {
        match self {
            Self::Heap(buf) => {
                unsafe {
                    // SAFETY: The buffer is guaranteed to be valid for the lifetime of the slice
                    std::slice::from_raw_parts(buf.as_ptr(), buf.len())
                }
            }
        }
    }

    // Get a mutable slice reference to the buffer data
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        match self {
            Self::Heap(buf) => {
                unsafe {
                    std::slice::from_raw_parts_mut(buf.as_mut_ptr(), buf.len())
                }
            }
        }
    }

    pub fn as_ptr(&self) -> *const u8 {
        match self {
            Self::Heap(buf) => buf.as_ptr()
        }
    }

    pub fn as_ptr_mut(&mut self) -> *mut u8 {
        match self {
            Self::Heap(buf) => buf.as_mut_ptr()
        }
    }
}

impl fmt::Debug for Buffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Heap(buf) => write!(f, "Heap(len={})", buf.len())
        }
    }
}