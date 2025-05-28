#![allow(unused)]

mod common;

const DISK_PATH: &str = "tests/virt_disk.img";
const DISK_BLOCKS: usize = 80;

use std::{fs::File, io::{Read, Seek, Write}, sync::{Arc, Mutex}};

use common::LruCache;
use muon::*;

pub struct VirtDisk {
    inner: Mutex<File>,
}

impl VirtDisk {
    pub fn new(path: &str) -> Result<Self> {
        let inner = Mutex::new(File::options().read(true).write(true).open(DISK_PATH).unwrap());
        Ok(VirtDisk { inner })
    }
}

impl BlockDevice for VirtDisk {
    fn num_blocks(&self) -> usize {
        DISK_BLOCKS
    }

    fn read_block(&self, block_id: u32, buf: &mut [u8; BLOCK_SIZE]) -> Result<()> {
        if block_id >= self.num_blocks() as u32 {
            return Err(Error::InvalidBlockId);
        }
        let start = block_id as usize * BLOCK_SIZE;
        let end = start + BLOCK_SIZE;
        let mut data = vec![0u8; BLOCK_SIZE];
        let mut inner = self.inner.lock().unwrap();
        inner.seek(std::io::SeekFrom::Start(start as u64)).unwrap();
        inner.read_exact(&mut data).unwrap();
        buf.copy_from_slice(&data[..BLOCK_SIZE]);
        Ok(())
    }

    fn write_block(&self, block_id: u32, buf: &[u8; BLOCK_SIZE]) -> Result<()> {
        if block_id >= self.num_blocks() as u32 {
            return Err(Error::InvalidBlockId);
        }
        let start = block_id as usize * BLOCK_SIZE;
        let mut inner = self.inner.lock().unwrap();
        inner.seek(std::io::SeekFrom::Start(start as u64)).unwrap();
        inner.write_all(buf).unwrap();
        Ok(())
    }

    fn flush(&self) -> Result<()> {
        let mut inner = self.inner.lock().unwrap();
        inner.flush().unwrap();
        Ok(())
    }
}



#[test]
fn disk_format() {
    let disk = VirtDisk::new(DISK_PATH).unwrap();
    let cache = LruCache::new(4);
    let cached = Cached::new(disk, cache);
    let mut fs = FileSystem::format(Arc::new(cached), DISK_BLOCKS as u32, 64).unwrap();
    log!("File System initialized: {}", fs.dump());
    fs.flush().unwrap();
}

// Following methods assume the file system is already formatted and ready to use.
#[test]
fn disk_mount() {
    let disk = VirtDisk::new(DISK_PATH).unwrap();
    let mut fs = FileSystem::mount(Arc::new(disk)).unwrap();

    log!("File System mounted: {}", fs.dump());
}