use bitflags::bitflags;
use clock::Clock;
use error::TursoMiniError;
use std::sync::Arc;
use buffer::Buffer;
use error::CompletionError;

pub mod buffer;
pub mod error;
pub mod clock;

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

pub type Result<T, E = TursoMiniError> = std::result::Result<T, E>;

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