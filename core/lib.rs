use crate::io::error::TursoMiniError;

pub mod storage;
pub mod io;

pub type Result<T, E = TursoMiniError> = std::result::Result<T, E>;