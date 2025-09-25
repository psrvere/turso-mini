use bitflags::bitflags;
use clock::Clock;
use error::TursoMiniError;
use core::fmt;
use std::sync::{Arc, OnceLock};
use buffer::Buffer;
use error::CompletionError;
use std::fmt::Debug;

pub mod buffer;
pub mod error;
pub mod clock;
pub mod memory;

pub type Result<T, E = TursoMiniError> = std::result::Result<T, E>;

pub trait File: Send + Sync {
    fn lock_file(&self) -> Result<()>;
    fn unlock_file(&self) -> Result<()>;
    fn pread(&self, pos: u64, c: Completion) -> Result<Completion>;
    fn pwrite(&self, pos: u64, buffer: Arc<Buffer>, c: Completion) -> Result<Completion>;
    fn sync(&self, c: Completion) -> Result<Completion>;
    fn truncate(&self, len: u64, c: Completion) -> Result<Completion>;
    fn size(&self) -> Result<u64>;
    fn pwritev(&self, pos: u64, buffers: Vec<Arc<Buffer>>, c: Completion) -> Result<Completion>;
}

pub trait IO: Clock + Send + Sync {
    fn open_file(&self, path: &str, flags: OpenFlags) -> Result<Arc<dyn File>>;
    fn remove_file(&self, path: &str) -> Result<()>;
    fn step(&self) -> Result<()>;
    fn cancel(&self, c: &[Completion]) -> Result<()>;
    fn drain(&self) -> Result<()>;
    fn wait_for_completion(&self, c: Completion) -> Result<()>;
}

#[derive(Debug, PartialEq)]
pub struct OpenFlags(i32);

bitflags! {
    impl OpenFlags: i32 {
        const None = 0b00000000;
        const Create = 0b00000001;
        const ReadOnly = 0b00000010;
    }
}

impl Default for OpenFlags {
    fn default() -> Self {
        Self::Create
    }
}

pub type ReadComplete = dyn Fn(Result<(Arc<Buffer>, i32), CompletionError>);
pub type WriteComplete = dyn Fn(Result<i32, CompletionError>);
pub type SyncComplete = dyn Fn(Result<i32, CompletionError>);
pub type TruncateComplete = dyn Fn(Result<i32, CompletionError>);

pub struct ReadCompletion {
    pub buf: Arc<Buffer>,
    pub complete: Box<ReadComplete>,
}

impl ReadCompletion {
    pub fn new(buf: Arc<Buffer>, complete: Box<ReadComplete>) -> Self {
        Self {buf, complete}
    }

    pub fn callback(&self, bytes_read: Result<i32, CompletionError>) {
        (self.complete)(bytes_read.map(|b| (self.buf.clone(), b)));
    }

    pub fn buf(&self) -> &Buffer {
        &self.buf
    }
}

pub struct WriteCompletion {
    pub complete: Box<WriteComplete>,
}

impl WriteCompletion {
    pub fn new(complete: Box<WriteComplete>) -> Self {
        Self { complete }
    }

    pub fn callback(&self, bytes_written: Result<i32, CompletionError>) {
        (self.complete)(bytes_written);
    }
}

pub struct SyncCompletion {
    pub complete: Box<SyncComplete>,
}

impl SyncCompletion {
    pub fn new(complete: Box<SyncComplete>) -> Self {
        Self { complete }
    }

    pub fn callback(&self, res: Result<i32, CompletionError>) {
        (self.complete)(res);
    }
}

pub struct TruncateCompletion {
    pub complete: Box<TruncateComplete>,
}

impl TruncateCompletion {
    pub fn new(complete: Box<TruncateComplete>) -> Self{
        Self { complete }
    }

    pub fn callback(&self, res: Result<i32, CompletionError>) {
        (self.complete)(res)
    }
}

pub enum CompletionType {
    Read(ReadCompletion),
    Write(WriteCompletion),
    Sync(SyncCompletion),
    Truncate(TruncateCompletion),
}

impl Debug for CompletionType {
     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(..) => f.debug_tuple("Read").finish(),
            Self::Write(..) => f.debug_tuple("Write").finish(),
            Self::Sync(..) => f.debug_tuple("Sync").finish(),
            Self::Truncate(..) => f.debug_tuple("Truncate").finish(),
        }
     }
}

#[derive(Debug)]
struct CompletionInner {
    completion_type: CompletionType,
    result: OnceLock<Option<CompletionError>>,
}

pub struct Completion {
    inner: Arc<CompletionInner>,
}

impl Completion {
    pub fn new(completion_type: CompletionType) -> Self{
        Self{
            inner: Arc::new(CompletionInner { 
                completion_type: completion_type, 
                result: OnceLock::new(), 
            }),
        }
    }

    // Q. Why do we use Generic Type F and not WriteComplete directly?
    // WriteComplete is a trait object (unsized type)
    // Function parameters must be sized
    // So we will have to pass it as Box<WriteComplete> which makes
    // it inconvenient for oter devs to use this function as they have
    // to mnaully box it everytime

    // Q. Why use static lifetime here?
    // It's a best practice to use static lifetime with callback function signatures
    // Callbacks are called asynchronously, hence they must not have any dangling
    // references. Also, with this callbacks can be safely moved between threads
    pub fn new_write<F>(complete: F) -> Self
    where
        F: Fn(Result<i32, CompletionError>) + 'static,
    {
        Self::new(CompletionType::Write(WriteCompletion::new(
            Box::new(complete)
        )))
    }

    pub fn new_read<F>(buf: Arc<Buffer>, complete: F) -> Self
    where
        F: Fn(Result<(Arc<Buffer>, i32), CompletionError>) + 'static,
    {
            Self::new(CompletionType::Read(ReadCompletion::new(
                buf, 
                Box::new(complete),
            )))
    }

    pub fn new_sync<F>(complete: F) -> Self
    where 
        F: Fn(Result<i32, CompletionError>) + 'static
    {
        Self::new(CompletionType::Sync(SyncCompletion::new(
            Box::new(complete),
        )))
    }

    pub fn new_trunc<F>(complete: F) -> Self
    where 
        F: Fn(Result<i32, CompletionError>) + 'static
    {
        Self::new(CompletionType::Truncate(TruncateCompletion::new(
            Box::new(complete),
        )))
    }

    pub fn complete(&self, result: i32) {
        let result = Ok(result);
        match &self.inner.completion_type {
            CompletionType::Read(r) => r.callback(result),
            CompletionType::Write(w) => w.callback(result),
            CompletionType::Sync(s) => s.callback(result),
            CompletionType::Truncate(t) => t.callback(result),
        }
        self.inner.result.set(None).expect("result must be set only once");
    }

    pub fn error(&self, err: CompletionError) {
        let result = Err(err);
        match &self.inner.completion_type {
            CompletionType::Read(r) => r.callback(result),
            CompletionType::Write(w) => w.callback(result),
            CompletionType::Sync(s) => s.callback(result),
            CompletionType::Truncate(t) => t.callback(result),
        }
        self.inner.result.set(Some(err)).expect("result must be set only once");
    }

    // Q. unreachable vs panic?
    // panic is for unexpectd by possible error
    // unreachable is for impossible code paths. Compiler can optimize based on this assumption
    pub fn as_read(&self) -> &ReadCompletion {
        match self.inner.completion_type {
            CompletionType::Read(ref r) => r,
            _ => unreachable!("this function must be called on ReadCompletion only")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::OpenFlags;

    #[test]
    fn test_individual_flags() {
        let none_flag = OpenFlags::None;
        let create_flag = OpenFlags::Create;
        let read_only_flag = OpenFlags::ReadOnly;

        assert_eq!(none_flag.bits(), 0);
        assert_eq!(create_flag.bits(), 1);
        assert_eq!(read_only_flag.bits(), 2);
    }

    #[test]
    fn test_combined_flags() {
        let combined = OpenFlags::Create | OpenFlags::ReadOnly;
        assert_eq!(combined.bits(), 3);

        assert!(combined.contains(OpenFlags::Create));
        assert!(combined.contains(OpenFlags::ReadOnly));
        assert!(combined.contains(OpenFlags::None));
    }

    #[test]
    fn test_default_flags() {
        let default_flags = OpenFlags::default();
        assert_eq!(default_flags, OpenFlags::Create);
    }
}