#![allow(unused)]
use std::{collections::VecDeque, sync::{Arc, Mutex}};

use common::{LruCache, RamDisk};
use muon::{BlockDevice, Cache, Cached, FileSystem, FileType, Mode, Result, BLOCK_SIZE};

mod common;

#[test]
fn test_cached() {
    let rd = RamDisk::new(64);
    let cache = LruCache::new(4);
    let cached = Cached::new(rd, cache);
    let mut fs = FileSystem::format(Arc::new(cached), 64, 80).unwrap();

    log!("fs initialized {}", fs.dump());
    fs.flush().unwrap();
    log!("fs flushed {}", fs.dump());
}

#[test]
fn test_hard_link() {
    let rd = RamDisk::new(64);
    let cache = LruCache::new(4);
    let cached = Cached::new(rd, cache);
    let mut fs = FileSystem::format(Arc::new(cached), 64, 80).unwrap();

    log!("File System initialized: {}", fs.dump());

    // Create a file.
    let file_inode_id = fs.creat("/test.txt", FileType::Regular, Mode::RW).unwrap();
    let file_inode = fs.get_inode(file_inode_id).unwrap();
    log!("File inode created: {:?}", file_inode);
    
    let dir_inode_id = fs.creat("/test_dir", FileType::Directory, Mode::RW).unwrap();
    let dir_inode = fs.get_inode(dir_inode_id).unwrap();
    log!("Directory inode created: {:?}", dir_inode);
    log!("File System after creating file and directory: {}", fs.dump());

    // Create a hard link to the file.
    let link_inode_id = fs.link("/test.txt", "/test_dir/test_link.txt").unwrap();
    let link_inode = fs.get_inode(link_inode_id).unwrap();
    log!("Hard link inode created: {:?}", link_inode);
    
    log!("File System after creating hard link: {}", fs.dump());

    // Check if both inodes point to the same data.
    assert_eq!(file_inode_id, link_inode_id, "Hard link should have the same inode ID as the original file");
    
    // Check directory entries.
    let entries = fs.read_dir("/").unwrap();
    for entry in entries {
        log!("Inode {} Name {}", entry.inode_id, String::from_utf8_lossy(&entry.name));
    }

    // Write some data to the original file.
    let data = b"Hello, hard link!";
    let bytes_written = fs.fwrite("/test.txt", 0, data).unwrap();
    log!("Bytes written to original file: {}", bytes_written);

    // Remove the original file.
    fs.remove("/test.txt", FileType::Regular).unwrap();
    log!("File System after removing original file: {}", fs.dump());
    // Check if the hard link still exists.
    let (link_inode_id, ftype) = fs.lookup("/test_dir/test_link.txt").unwrap();
    assert_eq!(ftype, FileType::Regular, "Hard link should still exist as a regular file");
    let link_inode = fs.get_inode(link_inode_id).unwrap();
    log!("Hard link inode after removing original file: {:?}", link_inode);
    // Check if the link count is correct.
    assert_eq!(link_inode.links_cnt, 1, "Hard link should have a link count of 1 after removing the original file");

    // Read the data from the hard link.
    let mut buf = vec![0u8; data.len()];
    let bytes_read = fs.fread("/test_dir/test_link.txt", 0, &mut buf).unwrap();
    log!("Data read from hard link: {:?}", String::from_utf8_lossy(&buf));

    // Now remove the hard link.
    fs.remove("/test_dir/test_link.txt", FileType::Regular).unwrap();
    log!("File System after removing hard link: {}", fs.dump());
    fs.flush().unwrap();
}