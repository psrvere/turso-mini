use std::pin::Pin;
use std::fmt;

pub type BufferData = Pin<Box<[u8]>>;
pub enum Buffer {
    Heap(BufferData)
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
    pub fn as_mut_slice(&self) -> &mut [u8] {
        unsafe {std::slice::from_raw_parts_mut(self.as_mut_ptr(), self.len())}
    }

    pub fn as_ptr(&self) -> *const u8 {
        match self {
            Self::Heap(buf) => buf.as_ptr()
        }
    }

    pub fn as_mut_ptr(&self) -> *mut u8 {
        match self {
            Self::Heap(buf) => buf.as_ptr() as *mut u8,
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

// Rust will handle cleanup automatically
// The Arc<Buffer> will automatically deallocates when ref counts reaches 0
// So let's not implement Drop trait for now
// impl Drop for Buffer {
//     fn drop(&mut self) {
//     }
// }