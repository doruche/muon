
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsError {
    InvalidMagic,
    InvalidSuperBlock,
    InvalidBlockId,
    ReadError,
    WriteError,
}

