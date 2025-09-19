use thiserror::Error;

#[derive(Error, Debug)]
pub enum TursoMiniError {
    #[error("File Extension error: {0}")]
    FileExtensionError(String),
    #[error("File Locking error: {0}")]
    FileLockingError(String),
    #[error("Completion error: {0}")]
    CompletionError(#[from] CompletionError),
}

#[derive(Error, Debug)]
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