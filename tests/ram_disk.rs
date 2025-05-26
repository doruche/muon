#![allow(unused)]

use std::sync::{Arc, Mutex};

use muon::get_inode;
use muon::write_inode;
use muon::BlockDevice;
use muon::Error;
use muon::FileType;
use muon::Inode;
use muon::Mode;
use muon::SuperBlock;
use muon::BLOCK_SIZE;
use muon::NUM_DIRECT_PTRS;

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

    fn read_block(&self, block_id: usize, buf: &mut [u8; BLOCK_SIZE]) -> Result<(), muon::Error> {
        if block_id >= self.num_blocks {
            return Err(Error::InvalidBlockId);
        }
        let start = block_id * BLOCK_SIZE;
        let end = start + BLOCK_SIZE;
        let data = self.inner.lock().unwrap();
        buf.copy_from_slice(&data[start..end]);
        Ok(())
    }

    fn write_block(&self, block_id: usize, buf: &[u8; BLOCK_SIZE]) -> Result<(), Error> {
        if block_id >= self.num_blocks {
            return Err(Error::InvalidBlockId);
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

#[test]
fn test_superblock() {
    let rd = RamDisk::new(64);
    let superblock = SuperBlock::new(64, 80).unwrap();
    println!("Created superblock: {:?}", superblock);
    muon::write_superblock(&rd, &superblock).unwrap();
    let read_superblock = muon::read_superblock(&rd).unwrap();
    assert_eq!(superblock.magic, read_superblock.magic);
    println!("Read superblock: {:?}", read_superblock);
}

#[test]
fn test_inode() {
    let rd = RamDisk::new(64);
    let superblock = SuperBlock::new(64, 80).unwrap();
    muon::write_superblock(&rd, &superblock).unwrap();
    let inode = Inode {
        mode: Mode::RW,
        ftype: FileType::Regular,
        blocks: 3,
        id: 3,
        links_cnt: 1,
        indirect_ptr: 0,
        direct_ptrs: [0; NUM_DIRECT_PTRS],
        size: 1024,
        reserved: [0; 44],
    };
    write_inode(&rd, &superblock, &inode).unwrap();
    let mut read_inode = get_inode(&rd, &superblock, 3).unwrap();
    println!("Inode struct size: {}", std::mem::size_of::<Inode>());
    println!("Read inode: {:?}", read_inode);
}
