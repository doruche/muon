use crate::error::FsError;


pub trait BlockDevice: Send + Sync {
    /// Returns the number of blocks in the block device.
    fn num_blocks(&self) -> usize;

    /// Reads a block of data from the block device.
    /// buf.len() must be equal to block_size().
    fn read_block(&self, block_id: usize, buf: &mut [u8]) -> Result<(), FsError>;
    
    /// Writes a block of data to the block device.
    /// buf.len() must be equal to block_size().
    fn write_block(&self, block_id: usize, buf: &[u8]) -> Result<(), FsError>;
    
    /// Flushes any cached data to the block device.
    /// This is typically used to ensure that all writes are persisted.
    fn flush(&self) -> Result<(), FsError>;
    
    /// Returns the size of each block in bytes.
    fn block_size(&self) -> usize {
        crate::config::BLOCK_SIZE
    }
}