#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsError {
    IoError,
    InvalidMagic,
    OutOfSpace,
    OutOfInodes,
    OutOfBounds,
    PermissionDenied,
    InvalidSuperBlock,
    InvalidBlockId,
    InvalidPath,
    InvalidArgument,
    FileTooLarge,
    InvalidFileName,
    ReadError,
    WriteError,
    NotFound,
    AlreadyExists,
    NotDirectory,
    NotRegular,
    NotReadable,
    NotWritable,
    NotEmpty,
}

pub type Result<T> = core::result::Result<T, FsError>;