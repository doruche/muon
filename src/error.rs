#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsError {
    IoError,
    InvalidMagic,
    OutOfSpace,
    OutOfInodes,
    OutOfBounds,
    CacheMiss,
    CacheEvict(u32), // Block ID of the evicted cache entry
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
    DirNotEmpty,
    NotDirectory,
    NotRegular,
    NotReadable,
    NotWritable,
    NotEmpty,
}

pub type Result<T> = core::result::Result<T, FsError>;