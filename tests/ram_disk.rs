use std::sync::{Arc, Mutex};

use muon::BlockDevice;
use muon::Error;
use muon::BLOCK_SIZE;

pub struct RamDisk {
    inner: Arc<Mutex<Vec<u8>>>,
    num_blocks: usize,
}

impl RamDisk {
    /// Creates a new RamDisk with the specified number of blocks.
    /// Each block is BLOCK_SIZE bytes.
    pub fn new(num_blocks: usize) -> Self {
        let size = num_blocks * BLOCK_SIZE;
        let inner = Arc::new(Mutex::new(vec![0u8; size]));
        RamDisk {
            inner,
            num_blocks,
        }
    }
}


impl BlockDevice for RamDisk {
    fn num_blocks(&self) -> usize {
        self.num_blocks
    }

    fn read_block(&self, block_id: usize, buf: &mut [u8]) -> Result<(), muon::Error> {
        if block_id >= self.num_blocks {
            return Err(Error::InvalidBlockId);
        }
        if buf.len() != BLOCK_SIZE {
            return Err(Error::ReadError);
        }
        let start = block_id * BLOCK_SIZE;
        let end = start + BLOCK_SIZE;
        let data = self.inner.lock().unwrap();
        buf.copy_from_slice(&data[start..end]);
        Ok(())
    }

    fn write_block(&self, block_id: usize, buf: &[u8]) -> Result<(), Error> {
        if block_id >= self.num_blocks {
            return Err(Error::InvalidBlockId);
        }
        if buf.len() != BLOCK_SIZE {
            return Err(Error::WriteError);
        }
        let start = block_id * BLOCK_SIZE;
        let end = start + BLOCK_SIZE;
        let mut data = self.inner.lock().unwrap();
        data[start..end].copy_from_slice(buf);
        Ok(())
    }

    fn flush(&self) -> Result<(), Error> {
        // In a RAM disk, flushing is a no-op since data is already in memory.
        Ok(())
    }
}

fn test_superblock() {

}

fn main() {
    test_superblock();    
}