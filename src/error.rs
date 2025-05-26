#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsError {
    IoError,
    InvalidMagic,
    OutOfSpace,
    OutOfInodes,
    OutOfBounds,
    InvalidSuperBlock,
    InvalidBlockId,
    FileTooLarge,
    ReadError,
    WriteError,
    NotFound,
    AlreadyExists,
    NotDirectory,
    NotFile,
    NotEmpty,
}

pub type Result<T> = core::result::Result<T, FsError>;