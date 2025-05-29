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
    let mut fs = FileSystem::format(Arc::new(cached), DISK_BLOCKS as u32, 80).unwrap();
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

#[test]
fn test_repeated_create() {
    let disk = VirtDisk::new(DISK_PATH).unwrap();
    let mut fs = FileSystem::mount(Arc::new(disk)).unwrap();
    log!("File System mounted: {}", fs.dump());
    fs.creat("/dir", FileType::Directory, Mode::RW).unwrap();
    log!("Directory created: /dir",);
    let res = fs.creat("/dir", FileType::Directory, Mode::RW);
    assert!(res.is_err(), "Creating a directory that already exists should fail");
    log!("Attempted to create existing directory: {:?}", res.unwrap_err());
    fs.flush().unwrap();
    log!("File System after repeated create: {}", fs.dump());
}

#[test]
fn test_hard_link() {
    let disk = VirtDisk::new(DISK_PATH).unwrap();
    let mut fs = FileSystem::mount(Arc::new(disk)).unwrap();
    log!("File System mounted: {}", fs.dump());

    // Create a file and write some data to it.
    let inode_id = fs.creat("/test_file.txt", FileType::Regular, Mode::RW).unwrap();
    let mut inode = fs.get_inode(inode_id).unwrap();
    let data = b"Hello, World!";
    fs.fwrite("/test_file.txt", 0, data).unwrap();
    log!("Data written to /test_file.txt",);

    // Create a hard link to the file.
    let link_inode_id = fs.link("/test_file.txt", "/dir/test_link.txt").unwrap();
    log!("Hard link created: /test_link.txt -> /dir/test_file.txt",);

    // Read the data from the hard link.
    let mut buf = vec![0u8; data.len()];
    fs.fread("/dir/test_link.txt", 0, &mut buf).unwrap();
    log!("Data read from hard link: {:?}", String::from_utf8_lossy(&buf));

    // Now remove the original file.
    fs.remove("/test_file.txt", FileType::Regular).unwrap();
    log!("File System after removing hard link: {}", fs.dump());

    // Remove the hard link.
    fs.remove("/dir/test_link.txt", FileType::Regular).unwrap();
    log!("File System after removing hard link: {}", fs.dump());
    fs.flush().unwrap();
}

#[test]
fn test_multiple_hard_links() {
    let disk = VirtDisk::new(DISK_PATH).unwrap();
    let mut fs = FileSystem::mount(Arc::new(disk)).unwrap();
    log!("File System mounted: {}", fs.dump());

    // Create a file and write some data to it.
    let inode_id = fs.creat("/test_file.txt", FileType::Regular, Mode::RW).unwrap();
    let mut inode = fs.get_inode(inode_id).unwrap();
    let data = b"Hello, World!";
    fs.fwrite("/test_file.txt", 0, data).unwrap();
    log!("Data written to /test_file.txt",);

    // Create multiple hard links to the file.
    let link1_inode_id = fs.link("/test_file.txt", "/dir/test_link1.txt").unwrap();
    let link2_inode_id = fs.link("/test_file.txt", "/dir/test_link2.txt").unwrap();
    log!("Hard links created: /dir/test_link1.txt and /dir/test_link2.txt",);

    // Read the data from the first hard link.
    let mut buf1 = vec![0u8; data.len()];
    fs.fread("/dir/test_link1.txt", 0, &mut buf1).unwrap();
    log!("Data read from first hard link: {:?}", String::from_utf8_lossy(&buf1));

    // Write data to the first hard link.
    let new_data = b"Hello, Hard Links!";
    fs.fwrite("/dir/test_link1.txt", BLOCK_SIZE * 3, new_data).unwrap();
    log!("Data written to first hard link: {:?}", String::from_utf8_lossy(new_data));
    
    // Read the data from the second hard link.
    let mut buf2 = vec![0u8; new_data.len()];
    fs.fread("/dir/test_link2.txt", 0, &mut buf2).unwrap();
    log!("Data read from second hard link: {:?}", String::from_utf8_lossy(&buf2));

    // Now remove the original file.
    fs.remove("/test_file.txt", FileType::Regular).unwrap();
    log!("File System after removing original file: {}", fs.dump());

    // Remove the hard links.
    fs.remove("/dir/test_link1.txt", FileType::Regular).unwrap();
    fs.remove("/dir/test_link2.txt", FileType::Regular).unwrap();
    log!("File System after removing hard links: {}", fs.dump());
    
    fs.flush().unwrap();
}
