#![allow(unused)]

use std::fs::File;
use std::sync::{Arc, Mutex};

mod common;

use common::RamDisk;
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
        mode: Mode::RW,
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
    log!("{}", fs.dump());
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

    // creating files with the same name should fail
    let result = fs.creat("/test.txt", FileType::Regular, Mode::RW);
    assert!(result.is_err(), "Expected error when creating file with existing name");
    if let Err(e) = result {
        println!("Expected error: {:?}", e);
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

#[test]
fn test_resource_release() {
    // Test inode releasing.
    let rd = RamDisk::new(64);
    let mut fs = FileSystem::format(Arc::new(rd), 64, 80).unwrap();
    let sb = fs.superblock();
    let num_inodes = sb.num_inodes;
    assert_eq!(sb.free_inodes, num_inodes - 2); // One inode for placeholder and one for root.
    let file_inode_id = fs.creat("/test.txt", FileType::Regular, Mode::RW).unwrap();
    let file_inode = fs.get_inode(file_inode_id).unwrap();
    assert_eq!(fs.superblock().free_inodes, num_inodes - 3); // One more inode used.
    assert_eq!(file_inode.id, 2); // First user inode after root.
    fs.remove("/test.txt", FileType::Regular).unwrap();
    assert_eq!(fs.superblock().free_inodes, num_inodes - 2); // Inode released.
    let file_inode_id = fs.creat("/test2.txt", FileType::Regular, Mode::RW).unwrap();
    assert_eq!(file_inode_id, 2); // Reused inode.
}

#[test]
fn test_mkdir() {
    let rd = RamDisk::new(64);
    let mut fs = FileSystem::format(Arc::new(rd), 64, 80).unwrap();
    let dir_inode_id = fs.creat("/test_dir", FileType::Directory, Mode::RW).unwrap();
    
    let entries = fs.read_dir("/").unwrap();
    for entry in entries {
        println!("Inode {} Name {}", entry.inode_id, String::from_utf8_lossy(&entry.name));
    }
    let (inode_id, ftype) = fs.lookup("/test_dir").unwrap();
    let inode = fs.get_inode(inode_id).unwrap();
    assert_eq!(inode.ftype, FileType::Directory);

    let entries = fs.read_dir("/test_dir").unwrap();
    for entry in entries {
        println!("Inode {} Name {}", entry.inode_id, String::from_utf8_lossy(&entry.name));
    }

    // create files inside the directory
    fs.creat("/test_dir/file1.txt", FileType::Regular, Mode::RW).unwrap();
    fs.creat("/test_dir/file2.txt", FileType::Regular, Mode::RW).unwrap();
    let entries = fs.read_dir("/test_dir").unwrap();
    for entry in entries {
        println!("Inode {} Name {}", entry.inode_id, String::from_utf8_lossy(&entry.name));
    }
    // remove files inside the directory
    fs.remove("/test_dir/file1.txt", FileType::Regular).unwrap();
    fs.remove("/test_dir/file2.txt", FileType::Regular).unwrap();
    let entries = fs.read_dir("/test_dir").unwrap();
    for entry in entries {
        println!("Inode {} Name {}", entry.inode_id, String::from_utf8_lossy(&entry.name));
    }
}

#[test]
fn test_rmdir() {
    let rd = RamDisk::new(64);
    let mut fs = FileSystem::format(Arc::new(rd), 64, 80).unwrap();
    fs.creat("/test_dir", FileType::Directory, Mode::RW).unwrap();
    
    let entries = fs.read_dir("/").unwrap();
    for entry in entries {
        println!("Inode {} Name {}", entry.inode_id, String::from_utf8_lossy(&entry.name));
    }
    
    fs.remove("/test_dir", FileType::Directory).unwrap();
    
    let entries = fs.read_dir("/").unwrap();
    for entry in entries {
        println!("Inode {} Name {}", entry.inode_id, String::from_utf8_lossy(&entry.name));
    }

    // Remove directories with files inside should fail.
    fs.creat("/test_dir2", FileType::Directory, Mode::RW).unwrap();
    fs.creat("/test_dir2/file.txt", FileType::Regular, Mode::RW).unwrap();
    let result = fs.remove("/test_dir2", FileType::Directory);
    assert!(result.is_err(), "Expected error when removing non-empty directory");
    if let Err(e) = result {
        println!("Expected error: {:?}", e);
    }
}

#[test]
fn test_mkdir_2() {
    // Test creating a directory inside another directory.
    let rd = RamDisk::new(64);
    let mut fs = FileSystem::format(Arc::new(rd), 64, 80).unwrap();
    fs.creat("/parent_dir", FileType::Directory, Mode::RW).unwrap();
    
    let entries = fs.read_dir("/").unwrap();
    for entry in entries {
        log!("Inode {} Name {}", entry.inode_id, String::from_utf8_lossy(&entry.name));
    }
    
    fs.creat("/parent_dir/child_dir", FileType::Directory, Mode::RW).unwrap();
    
    let entries = fs.read_dir("/parent_dir").unwrap();
    for entry in entries {
        log!("Inode {} Name {}", entry.inode_id, String::from_utf8_lossy(&entry.name));
    }
    let (inode_id, ftype) = fs.lookup("/parent_dir/child_dir").unwrap();
    log!("Found inode {} with type {:?}", inode_id, ftype);
    let inode = fs.get_inode(inode_id).unwrap();
    log!("Child directory inode: {:?}", inode);

    fs.creat("/parent_dir/child_dir/parent_dir", FileType::Directory, Mode::RW).unwrap();
    let entries = fs.read_dir("/parent_dir/child_dir").unwrap();
    for entry in entries {
        log!("Inode {} Name {}", entry.inode_id, String::from_utf8_lossy(&entry.name));
    }
    let (inode_id, ftype) = fs.lookup("/parent_dir/child_dir/parent_dir").unwrap();
    log!("Found inode {} with type {:?}", inode_id, ftype);
    let inode = fs.get_inode(inode_id).unwrap();
    log!("Parent directory inode: {:?}", inode);

    // Check all inodes again.
    let root_inode = fs.get_inode(fs.root_inode_id()).unwrap();
    log!("Root inode: {:?}", root_inode);
    let parent_inode_id = fs.lookup("/parent_dir").unwrap().0;
    let parent_inode = fs.get_inode(parent_inode_id).unwrap();
    log!("Parent directory inode: {:?}", parent_inode);
    let child_inode_id = fs.lookup("/parent_dir/child_dir").unwrap().0;
    let child_inode = fs.get_inode(child_inode_id).unwrap();
    log!("Child directory inode: {:?}", child_inode);
    let grandchild_inode_id = fs.lookup("/parent_dir/child_dir/parent_dir").unwrap().0;
    let grandchild_inode = fs.get_inode(grandchild_inode_id).unwrap();
    log!("Grandchild directory inode: {:?}", grandchild_inode);
}

#[test]
fn test_resource_release_2() {
    // Test inode releasing with directories.
    let rd = RamDisk::new(64);
    let mut fs = FileSystem::format(Arc::new(rd), 64, 80).unwrap();
    let free_blocks = fs.superblock().free_blocks;
    let sb = fs.superblock();
    let num_inodes = sb.num_inodes;
    assert_eq!(sb.free_inodes, num_inodes - 2); // One inode for placeholder and one for root.
    
    let dir_inode_id = fs.creat("/test_dir", FileType::Directory, Mode::RW).unwrap();
    let dir_inode = fs.get_inode(dir_inode_id).unwrap();
    assert_eq!(fs.superblock().free_inodes, num_inodes - 3); // One more inode used.
    assert_eq!(dir_inode.id, 2); // First user inode after root.
    
    fs.remove("/test_dir", FileType::Directory).unwrap();
    assert_eq!(fs.superblock().free_inodes, num_inodes - 2); // Inode released.
    
    let dir_inode_id = fs.creat("/test_dir2", FileType::Directory, Mode::RW).unwrap();
    assert_eq!(dir_inode_id, 2); // Reused inode. 

    // Create a file inside the directory.
    let file_inode_id = fs.creat("/test_dir2/file.txt", FileType::Regular, Mode::RW).unwrap();
    assert_eq!(file_inode_id, 3); // New inode for file.
    fs.remove("/test_dir2/file.txt", FileType::Regular).unwrap();
    assert_eq!(fs.superblock().free_inodes, num_inodes - 3); // File inode released.
    fs.remove("/test_dir2", FileType::Directory).unwrap();
    assert_eq!(fs.superblock().free_inodes, num_inodes - 2); // Directory inode released.
    // Check if the directory inode is reused.
    let dir_inode_id = fs.creat("/test_dir3", FileType::Directory, Mode::RW).unwrap();
    assert_eq!(dir_inode_id, 2); // Reused inode.
    fs.remove("/test_dir3", FileType::Directory).unwrap();
    assert_eq!(free_blocks, fs.superblock().free_blocks); // No blocks used for empty directory.
}

#[test]
fn test_file_rw() {
    let rd = RamDisk::new(64);
    let mut fs = FileSystem::format(Arc::new(rd), 64, 80).unwrap();
    let file_inode_id = fs.creat("/test.txt", FileType::Regular, Mode::RW).unwrap();
    let mut file_inode = fs.get_inode(file_inode_id).unwrap();
    log!("File inode created: {:?}", file_inode);

    // Write some data to the file.
    let data = b"Hello, world!";

    let bytes_written = fs.fwrite("/test.txt", 0, data).unwrap();
    assert_eq!(bytes_written, data.len());

    // Read the data back.
    let mut buf = vec![0u8; data.len()];
    let bytes_read = fs.fread("/test.txt", 0, &mut buf).unwrap();
    assert_eq!(bytes_read, data.len());
}

#[test]
fn test_file_rw_2() {
    // test reading and writing to a file with multiple blocks.
    let rd = RamDisk::new(64);
    let mut fs = FileSystem::format(Arc::new(rd), 64, 80).unwrap();
    let file_inode_id = fs.creat("/test.txt", FileType::Regular, Mode::RW).unwrap();
    let mut file_inode = fs.get_inode(file_inode_id).unwrap();
    log!("File inode created: {:?}", file_inode);
    // 20 blocks + 64 bytes of data.
    // This should allocate 21(for data) + 1(for indirect ptr block) blocks.
    log!("Free blocks before writing: {}", fs.superblock().free_blocks);
    let huge_data = vec![0u8; BLOCK_SIZE * 20 + 64];
    let bytes_written = fs.fwrite("/test.txt", 0, &huge_data).unwrap();
    assert_eq!(bytes_written, huge_data.len());
    log!("Free blocks after writing: {}", fs.superblock().free_blocks);
    // Read the data back.
    let mut buf = vec![0u8; huge_data.len()];
    let bytes_read = fs.fread("/test.txt", 0, &mut buf).unwrap();
    assert_eq!(bytes_read, huge_data.len());
    assert_eq!(buf, huge_data, "Data read from file does not match written data");
    // Check the inode after writing.
    file_inode = fs.get_inode(file_inode_id).unwrap();
    log!("File inode after writing: {:?}", file_inode);
    // Now try to read/write at different offsets.
    let mut write_buf = "Hello, Muon!".as_bytes();
    let bytes_written = fs.fwrite("/test.txt", 100, write_buf).unwrap();
    assert_eq!(bytes_written, write_buf.len(), "Bytes written mismatch");
    let mut read_buf = vec![0u8; write_buf.len()];
    let bytes_read = fs.fread("/test.txt", 100, &mut read_buf).unwrap();
    assert_eq!(bytes_read, write_buf.len(), "Bytes read mismatch");
    assert_eq!(read_buf, write_buf, "Data read from file does not match written data at offset 100");
    // If we read one more byte, the assertion should fail.
    let mut read_buf = vec![0u8; write_buf.len() + 1];
    let bytes_read = fs.fread("/test.txt", 100, &mut read_buf).unwrap();
    assert_eq!(&read_buf[..write_buf.len()], write_buf, "Data read from file does not match written data at offset 100");

    // Check proper release of resources.
    let sb = fs.superblock();
    let free_inodes = sb.free_inodes;
    let free_blocks = sb.free_blocks;
    log!("Before removing file: Free inodes: {}, Free blocks: {}", free_inodes, free_blocks);
    fs.remove("/test.txt", FileType::Regular).unwrap();
    log!("After removing file: Free inodes: {}, Free blocks: {}", fs.superblock().free_inodes, fs.superblock().free_blocks);

    // Should assure that the inode and blocks are released properly.
    // Try to reuse inode ID.
    let new_file_inode_id = fs.creat("/new_test.txt", FileType::Regular, Mode::RW).unwrap();
    let new_file_inode = fs.get_inode(new_file_inode_id).unwrap();
    log!("New file inode created: {:?}", new_file_inode);
    log!("Free inodes after creating new file: {}", fs.superblock().free_inodes);
    // Try to reuse data blocks.
    let new_huge_data = vec![0u8; BLOCK_SIZE * 10 - 64];
    let new_bytes_written = fs.fwrite("/new_test.txt", 0, &new_huge_data).unwrap();
    assert_eq!(new_bytes_written, new_huge_data.len(), "Bytes written mismatch for new file");
    log!("Free blocks after creating new file: {}", fs.superblock().free_blocks);
}

#[test]
fn test_file_rw_3() {
    // Test holes in files.
    let rd = RamDisk::new(64);
    let mut fs = FileSystem::format(Arc::new(rd), 64, 80).unwrap();
    log!("{:?}", fs.dump());
    // Read and write in a directory.
    fs.creat("/test_dir", FileType::Directory, Mode::RW).unwrap();
    let file_inode_id = fs.creat("/test_dir/test.txt", FileType::Regular, Mode::RW).unwrap();
    let mut file_inode = fs.get_inode(file_inode_id).unwrap();
    log!("File inode created: {:?}", file_inode);
    // Write some data to the file.
    let data = b"Hello, world!";
    let bytes_written = fs.fwrite("/test_dir/test.txt", 0, data).unwrap();
    assert_eq!(bytes_written, data.len(), "Bytes written mismatch");
    // Make a hole.
    let bytes_written = fs.fwrite("/test_dir/test.txt", 7 * BLOCK_SIZE, "Hollow World...".as_bytes()).unwrap();
    let mut file_inode = fs.get_inode(file_inode_id).unwrap();
    log!("File inode after writing hole: {:?}", file_inode);
    log!("Fyle System after writing hole: {}", fs.dump());

    // Read the first part of the file.
    let mut buf = vec![0u8; data.len()];
    let bytes_read = fs.fread("/test_dir/test.txt", 0, &mut buf).unwrap();
    assert_eq!(bytes_read, data.len(), "Bytes read mismatch for first part");
    log!("Data read from file: {:?}", String::from_utf8_lossy(&buf));

    // Read the second part of the file (the hole).
    let mut hole_buf = vec![0u8; 20]; // Read 20 bytes from the hole.
    let bytes_read = fs.fread("/test_dir/test.txt", 7 * BLOCK_SIZE, &mut hole_buf).unwrap();
    assert_eq!(bytes_read, 20, "Bytes read mismatch for hole");
    log!("Data read from hole: {:?}", String::from_utf8_lossy(&hole_buf));

    // Assure that we can't read beyond allocated data blocks.
    let mut beyond_buf = vec![0u8; 20];
    let bytes_read = fs.fread("/test_dir/test.txt", 8 * BLOCK_SIZE, &mut beyond_buf);
    assert!(bytes_read.is_err(), "Expected error when reading beyond allocated data blocks");
    log!("Error reading beyond allocated data blocks: {:?}", bytes_read.err());

    // Release resources.
    fs.remove("/test_dir/test.txt", FileType::Regular).unwrap();
    log!("After removing test.txt {}", fs.dump());
    fs.remove("/test_dir", FileType::Directory).unwrap();
    log!("After removing test_dir {}", fs.dump());
}

#[test]
fn test_mount() {
    let rd = Arc::new(RamDisk::new(64));
    let mut fs = FileSystem::format(rd.clone(), 64, 80).unwrap();
    // Make some changes to the device.
    fs.creat("/test.txt", FileType::Regular, Mode::RW).unwrap();
    fs.creat("/test_dir", FileType::Directory, Mode::RW).unwrap();
    fs.creat("/test_dir/test.txt", FileType::Regular, Mode::RW).unwrap();
    // Now unmount and remount the filesystem.
    let mut fs2 = FileSystem::mount(rd).unwrap();
    log!("Mounted filesystem: {}", fs2.dump());
    // Check if the changes are preserved.
    let entries = fs2.read_dir("/").unwrap();
    for entry in entries {
        log!("Inode {} Name {}", entry.inode_id, String::from_utf8_lossy(&entry.name));
        let inode = fs2.get_inode(entry.inode_id).unwrap();
        log!("Inode details: {:?}", inode);
    }
}

#[test]
fn test_hard_link() {
    let rd = RamDisk::new(64);
    let mut fs = FileSystem::format(Arc::new(rd), 64, 80).unwrap();

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
}