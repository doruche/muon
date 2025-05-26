#![allow(unused)]

use std::sync::{Arc, Mutex};

use muon::get_inode;
use muon::write_inode;
use muon::BlockDevice;
use muon::Error;
use muon::FileSystem;
use muon::FileType;
use muon::Inode;
use muon::Mode;
use muon::SuperBlock;
use muon::BLOCK_SIZE;
use muon::NUM_DIRECT_PTRS;

#[derive(Debug)]
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

    fn read_block(&self, block_id: u32, buf: &mut [u8; BLOCK_SIZE]) -> Result<(), muon::Error> {
        if block_id >= self.num_blocks as u32 {
            return Err(Error::InvalidBlockId);
        }
        let start = block_id as usize * BLOCK_SIZE;
        let end = start as usize + BLOCK_SIZE;
        let data = self.inner.lock().unwrap();
        buf.copy_from_slice(&data[start..end]);
        Ok(())
    }

    fn write_block(&self, block_id: u32, buf: &[u8; BLOCK_SIZE]) -> Result<(), Error> {
        if block_id >= self.num_blocks as u32 {
            return Err(Error::InvalidBlockId);
        }
        let start = block_id as usize * BLOCK_SIZE;
        let end = start as usize + BLOCK_SIZE;
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
    muon::write_superblock(&rd, &superblock).unwrap();
    let read_superblock = muon::read_superblock(&rd).unwrap();
    assert_eq!(superblock.magic, read_superblock.magic);
}

#[test]
fn test_inode() {
    let rd = RamDisk::new(64);
    let superblock = SuperBlock::new(64, 80).unwrap();
    muon::write_superblock(&rd, &superblock).unwrap();
    let inode = Inode {
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
}

#[test]
fn test_init_fs() {
    let rd = RamDisk::new(64);
    let fs = FileSystem::format(Arc::new(rd), 64, 80).unwrap();
}

#[test]
fn test_create_file() {
    let rd = RamDisk::new(64);
    let mut fs = FileSystem::format(Arc::new(rd), 64, 80).unwrap();
    let parent_inode_id = fs.root_inode_id();
    let file_name = "/test_file.txt";
    
    // Create a new file
    let new_inode_id = fs.open(file_name, Mode::RWE, true).unwrap();
    println!("Created file with inode ID: {}", new_inode_id);

    // Verify the file was created
    let inode = get_inode(fs.device().as_ref(), fs.superblock(), new_inode_id).unwrap();
    assert_eq!(inode.ftype, FileType::Regular);
}