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
    InvalidFileType,
    InvalidArgument,
    FileTooLarge,
    PathTooLong,
    InvalidFileName,
    ReadError,
    WriteError,
    NotFound,
    AlreadyExists,
    DirNotEmpty,
    NotDirectory,
    NotRegular,
    NotSymlink,
    NotReadable,
    NotWritable,
    NotEmpty,
}

pub type Result<T> = core::result::Result<T, FsError>;