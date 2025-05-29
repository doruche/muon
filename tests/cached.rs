#![allow(unused)]
use std::{collections::VecDeque, path, sync::{Arc, Mutex}};

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

#[test]
fn test_mkdir_3() {
    let rd = RamDisk::new(64);
    let cache = LruCache::new(4);
    let cached = Cached::new(rd, cache);
    let mut fs = FileSystem::format(Arc::new(cached), 64, 80).unwrap();

    log!("File System initialized: {}", fs.dump());

    // Create:
    // /a
    // /a/b
    // /a/b/c
    // /x
    // /x/y
    fs.creat("/a", FileType::Directory, Mode::RW).unwrap();
    fs.creat("/a/b", FileType::Directory, Mode::RW).unwrap();
    fs.creat("/a/b/c", FileType::Directory, Mode::RW).unwrap();
    fs.creat("/x", FileType::Directory, Mode::RW).unwrap();
    fs.creat("/x/y", FileType::Directory, Mode::RW).unwrap();
    log!("File System after creating directories: {}", fs.dump());
    read_dir_recursive(&mut fs, "/", 3);

    // Remove
    fs.remove("/x/y", FileType::Directory).unwrap();
    fs.remove("/a/b/c", FileType::Directory).unwrap();
    fs.remove("/a/b", FileType::Directory).unwrap();
    fs.remove("/a", FileType::Directory).unwrap();
    fs.remove("/x", FileType::Directory).unwrap();
    log!("File System after removing directories: {}", fs.dump());
    read_dir_recursive(&mut fs, "/", 3);

    // allocated a new inode
    let new_inode_id = fs.creat("/x/file", FileType::Regular, Mode::RW);
    // this should fail
    assert!(new_inode_id.is_err(), "Creating a file in a removed directory should fail");
    log!("{:?}", new_inode_id.unwrap_err());

    let new_inode_id = fs.creat("/file", FileType::Regular, Mode::RW).unwrap();
    let inode = fs.get_inode(new_inode_id).unwrap();
    log!("New file inode created: {:?}", inode);
    log!("File System after creating a new file: {}", fs.dump());
}

fn read_dir_recursive(fs: &mut FileSystem<impl BlockDevice>, path: &str, depth: usize) {
    if depth == 0 {
        return;
    }

    let entries = fs.read_dir(path).unwrap();
    log!("Directory entries in '{}':", path);
    let mut next_level_entries = vec![];
    for entry in entries {
        let name = String::from_utf8_lossy(&entry.name);
        log!("  Inode ID: {}, Name: {}", entry.inode_id, name);
        
        // If the entry is a directory, read its contents recursively.
        if entry.inode_id != 0 && fs.get_inode(entry.inode_id).unwrap().ftype == FileType::Directory {
            if entry.name_eq(".".as_bytes()) || entry.name_eq("..".as_bytes()) {
                continue; // Skip current and parent directory entries.
            }
            let next_path = if path == "/" {
                format!("/{}", name)
            } else {
                format!("{}/{}", path, name)
            };
            next_level_entries.push(next_path);   
        }
    }
    log!("End of directory entries in '{}'", path);
    for next_path in next_level_entries {
        read_dir_recursive(fs, &next_path, depth - 1);
    }
}