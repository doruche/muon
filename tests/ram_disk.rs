#![allow(unused)]

use std::fs::File;
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
        indirect_ptr: None,
        direct_ptrs: [None; NUM_DIRECT_PTRS],
        size: 1024,
    };
    write_inode(&rd, &superblock, &inode).unwrap();
    let mut read_inode = get_inode(&rd, &superblock, 3).unwrap();
}

#[test]
fn test_init_fs() {
    let rd = RamDisk::new(64);
    let fs = FileSystem::format(Arc::new(rd), 64, 80).unwrap();
    println!("{}", fs.dump());
}

#[test]
fn test_root_dir() {
    let rd = RamDisk::new(64);
    let mut fs = FileSystem::format(Arc::new(rd), 64, 80).unwrap();
    let root_inode_id = fs.root_inode_id();
    let root_inode = fs.get_inode(root_inode_id).unwrap();
    assert_eq!(root_inode.ftype, FileType::Directory);
    assert_eq!(root_inode.id, 1);
    assert_eq!(root_inode.blocks, 1);
    assert_eq!(root_inode.links_cnt, 2); // Root directory has at least two links: '.' and '..'
    let entries = fs.read_dir("/").unwrap();
    println!("Root directory entries count: {}", entries.len());
    for entry in entries {
        println!("Inode {} Name {}", entry.inode_id, String::from_utf8_lossy(&entry.name));
    }
}

#[test]
fn test_create_file() {
    let rd = RamDisk::new(64);
    let mut fs = FileSystem::format(Arc::new(rd), 64, 80).unwrap();
    let file_inode_id = fs.creat("/test.txt", FileType::Regular, Mode::RW).unwrap();
    let file_inode = fs.get_inode(file_inode_id).unwrap();
    assert_eq!(file_inode.ftype, FileType::Regular);
    assert_eq!(file_inode.id, file_inode_id);
    assert_eq!(file_inode.blocks, 0);
    assert_eq!(file_inode.links_cnt, 1); // New file has one link
    let entries = fs.read_dir("/").unwrap();
    for entry in entries {
        println!("Inode {} Name {}", entry.inode_id, String::from_utf8_lossy(&entry.name));
    }
    let file2_inode_id = fs.creat("/test2.txt", FileType::Regular, Mode::RW).unwrap();
    let file2_inode = fs.get_inode(file2_inode_id).unwrap();
    assert_eq!(file2_inode.ftype, FileType::Regular);
    assert_eq!(file2_inode.id, file2_inode_id);
    assert_eq!(file2_inode.blocks, 0);
    assert_eq!(file2_inode.links_cnt, 1); // New file has one link
    let entries = fs.read_dir("/").unwrap();
    for entry in entries {
        println!("Inode {} Name {}", entry.inode_id, String::from_utf8_lossy(&entry.name));
    }
}

#[test]
fn test_lookup() {
    let rd = RamDisk::new(64);
    let mut fs = FileSystem::format(Arc::new(rd), 64, 80).unwrap();
    fs.creat("/test.txt", FileType::Regular, Mode::RW).unwrap();
    let entries = fs.read_dir("/").unwrap();
    for entry in entries {
        println!("Inode {} Name {}", entry.inode_id, String::from_utf8_lossy(&entry.name));
    }
    let (inode_id, ftype) = fs.lookup("/test.txt").unwrap();
    let inode = fs.get_inode(inode_id).unwrap();
    assert_eq!(inode.ftype, FileType::Regular);
}

#[test]
fn test_remove_file() {
    let rd = RamDisk::new(64);
    let mut fs = FileSystem::format(Arc::new(rd), 64, 80).unwrap();
    fs.creat("/test.txt", FileType::Regular, Mode::RW).unwrap();
    let entries = fs.read_dir("/").unwrap();
    fs.remove("/test.txt", FileType::Regular).unwrap();
    let entries = fs.read_dir("/").unwrap();
    for entry in entries {
        println!("Inode {} Name {}", entry.inode_id, String::from_utf8_lossy(&entry.name));
    }
    fs.creat("/test2.txt", FileType::Regular, Mode::RW).unwrap();
    let entries = fs.read_dir("/").unwrap();
    for entry in entries {
        println!("Inode {} Name {}", entry.inode_id, String::from_utf8_lossy(&entry.name));
    }
    let (inode_id, ftype) = fs.lookup("/test2.txt").unwrap();
    let inode = fs.get_inode(inode_id).unwrap();
    assert_eq!(inode.ftype, FileType::Regular);
    fs.remove("/test2.txt", FileType::Regular).unwrap();
    let entries = fs.read_dir("/").unwrap();
    for entry in entries {
        println!("Inode {} Name {}", entry.inode_id, String::from_utf8_lossy(&entry.name));
    }
}

#[test]
fn test_remove_2() {
    // Create a bunch of files and test removal.
    let rd = RamDisk::new(64);
    let mut fs = FileSystem::format(Arc::new(rd), 64, 80).unwrap();
    for i in 0..10 {
        let file_name = format!("/file_{}.txt", i);
        fs.creat(&file_name, FileType::Regular, Mode::RW).unwrap();
    }
    let entries = fs.read_dir("/").unwrap();
    for entry in entries {
        println!("Inode {} Name {}", entry.inode_id, String::from_utf8_lossy(&entry.name));
    }
    for i in 0..10 {
        let file_name = format!("/file_{}.txt", i);
        fs.remove(&file_name, FileType::Regular).unwrap();
    }
    let entries = fs.read_dir("/").unwrap();
    for entry in entries {
        println!("Inode {} Name {}", entry.inode_id, String::from_utf8_lossy(&entry.name));
    }
}