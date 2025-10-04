use thiserror::Error;

#[derive(Error, Debug)]
pub enum TursoMiniError {
    #[error("File Extension error: {0}")]
    FileExtensionError(String),
    #[error("File Locking error: {0}")]
    FileLockingError(String),
    #[error("Completion error: {0}")]
    CompletionError(#[from] CompletionError),
    #[error("Corrupt databse: {0}")]
    Corrupt(String),
}

// Q. Copy vs Clone?
// Clone is explicity method call: let y = x.clone()
// It can be expensive: deep copy, heap allocation
// Copy is implicit: let y = x
// Must be cheap, stack only, no heap allocation
#[derive(Error, Debug, Clone, Copy)]
// CompletionError variablts contain simple types
// Hence this is stored on the stack
pub enum CompletionError {
    #[error("I/O error: {0}")]
    IOError(std::io::ErrorKind)
}

// creting new strings is expensive in hot path
// copying/cloning enums is cheaper
// hence we only propagate ErrorKind
impl From<std::io::Error> for TursoMiniError {
    fn from(value: std::io::Error) -> Self {
        Self::CompletionError(CompletionError::IOError(value.kind()))
    }
}

#[macro_export]
macro_rules! bail_corrupt_error {
    ($($arg:tt)*) => {
        return Err(TursoMiniError::Corrupt(format!($($arg)*)))
    }
}