use alloc::boxed::Box;
use alloc::vec;

use crate::{alloc_inode, bmap, write_inode, write_superblock, BlockDevice};
use crate::error::{FsError, Result};
use crate::config::*;
use crate::structs::*;

pub fn trim_zero(name: &[u8]) -> &[u8] {
    let mut end = name.len();
    while end > 0 && name[end - 1] == 0 {
        end -= 1;
    }
    &name[..end]
}

fn name_cmp(n1: &[u8], n2: &[u8]) -> bool {
    let nn1 = trim_zero(n1);
    let nn2 = trim_zero(n2);

    if nn1.len() != nn2.len() {
        return false;
    }

    for i in 0.. nn1.len() {
        if nn1[i] != nn2[i] {
            return false;
        }
        if nn1[i] == 0 {
            break; 
        }
    }
    true
}

fn name_is_empty(name: &[u8]) -> bool {
    name.iter().all(|&c| c == 0)
}

impl DirEntry {
    pub fn is_empty(&self) -> bool {
        self.inode_id == 0 && name_is_empty(&self.name)
    }

    pub fn name_eq(&self, name: &[u8]) -> bool {
        name_cmp(&self.name, name)
    }

    pub fn name_eq_str(&self, name: &str) -> bool {
        name_cmp(&self.name, name.as_bytes())
    }
}

/// Query inode id of a file by name in the parent directory inode.
/// Returns the inode ID of the file if found, or an error if not found or if the parent is not a directory.
pub fn dir_lookup(
    device: &impl BlockDevice,
    superblock: &mut SuperBlock,
    parent_inode: &mut Inode, // Parent directory inode
    name: &[u8],
) -> Result<u32> {
    if parent_inode.ftype != FileType::Directory {
        return Err(FsError::NotDirectory);
    }

    if name.len() > MAX_FILE_NAME_LEN {
        return Err(FsError::InvalidFileName);
    }

    let num_dirents = (parent_inode.size / DIR_ENTRY_SIZE as u64) as usize;
    let mut num_looked_up = 0;
    let num_blocks = parent_inode.blocks;

    for i in 0..num_blocks as usize {
        let block_id = bmap(
            device,
            superblock,
            parent_inode,
            i as u64 * BLOCK_SIZE as u64,
            false,
        )?;
        let mut direntries_buf = Box::new([0u8; BLOCK_SIZE]);
        device.read_block(block_id, direntries_buf.as_mut())?;
        for j in 0..NUM_ENTRY_PER_BLOCK {
            if num_looked_up >= num_dirents {
                break; // No more entries to check
            }
            let cur_entry_offset = j * DIR_ENTRY_SIZE;
            let entry_ptr = unsafe {
                direntries_buf.as_mut_ptr().add(cur_entry_offset) as *mut DirEntry
            };
            let entry = unsafe { &*entry_ptr };
            
            if entry.inode_id == 0 {
                continue;
            }
            num_looked_up += 1;
            println!("[dir_lookup] Checking entry: {}, inode_id: {}, query {}",
                String::from_utf8_lossy(&entry.name), entry.inode_id, String::from_utf8_lossy(name));
            if name_cmp(&entry.name, name) {
                println!("[dir_lookup] Found entry: {}, inode_id: {}", 
                    String::from_utf8_lossy(&entry.name), entry.inode_id);
                return Ok(entry.inode_id);
            }
        }   
    }

    Err(FsError::NotFound)
}

/// Add a new directory entry to a parent directory inode.
/// Would not increase links count of the child inode, which is caller's responsibility.
/// Child inode must be already allocated and initialized.
pub fn dir_add_entry(
    device: &impl BlockDevice,
    superblock: &mut SuperBlock,
    parent_inode: &mut Inode,
    child_entry: &DirEntry,
) -> Result<()> {
    if parent_inode.ftype != FileType::Directory {
        return Err(FsError::NotDirectory);
    }

    if let Ok(_) = dir_lookup(device, superblock, parent_inode, &child_entry.name) {
        return Err(FsError::AlreadyExists);
    }

    let prev_size = parent_inode.size;

    // Check if we need to allocate a new block for the directory entry
    let mut block_id_to_write = 0;
    let mut block_inner_offset = 0;
    let mut cur_block_buf = Box::new([0u8; BLOCK_SIZE]);

    let num_dirents = (prev_size / DIR_ENTRY_SIZE as u64) as usize;
    let num_blocks = parent_inode.blocks as u64;
    println!("Adding entry: {}, prev size: {}, num dirents: {}, num blocks: {}", 
        String::from_utf8_lossy(&child_entry.name), prev_size, num_dirents, num_blocks);

    'found_slot: for i in 0..num_blocks as usize {
        let block_id = bmap(
            device,
            superblock,
            parent_inode,
            i as u64 * BLOCK_SIZE as u64,
            false,
        )?;
        device.read_block(block_id, cur_block_buf.as_mut())?;

        for j in 0..NUM_ENTRY_PER_BLOCK {
            println!("Checking block {}, entry {}", i, j);
            let cur_dirent_offset = j * DIR_ENTRY_SIZE;
            let dirent_ptr = unsafe {
                cur_block_buf.as_mut_ptr().add(cur_dirent_offset) as *mut DirEntry
            };
            let dirent = unsafe { &*dirent_ptr };
                println!("entry {} name {}", i, String::from_utf8_lossy(&dirent.name));
            if dirent.inode_id == 0 && name_is_empty(&dirent.name) {
                // Found an empty slot
                block_id_to_write = block_id;
                block_inner_offset = cur_dirent_offset;
                parent_inode.size = prev_size + DIR_ENTRY_SIZE as u64;
                let new_blocks = ((parent_inode.size + BLOCK_SIZE as u64 - 1) / BLOCK_SIZE as u64) as u32;
                assert_eq!(new_blocks, parent_inode.blocks);
                write_inode(device, superblock, &parent_inode)?;
                break 'found_slot;
            }
        }
    }

    if block_id_to_write == 0 {
        // Allocate a new block for the directory entry
        block_id_to_write = bmap(
            device,
            superblock,
            parent_inode,
            prev_size,
            true,
        )?;
        println!("blocks: {}, new block id: {}", parent_inode.blocks, block_id_to_write);
        block_inner_offset = (prev_size % BLOCK_SIZE as u64) as usize;
        device.read_block(block_id_to_write, cur_block_buf.as_mut())?;

        parent_inode.size = (num_dirents + 1) as u64 * DIR_ENTRY_SIZE as u64;
        write_inode(device, superblock, &parent_inode)?;
    }

    let dest_ptr = unsafe {
        cur_block_buf.as_mut_ptr().add(block_inner_offset) as *mut DirEntry
    };
    unsafe {
        dest_ptr.write_unaligned(*child_entry);
    }
    device.write_block(block_id_to_write, cur_block_buf.as_ref())?;

    println!("After adding entry: {}, new size: {}, new blocks: {}", 
        String::from_utf8_lossy(&child_entry.name), parent_inode.size, parent_inode.blocks);

    Ok(())
}

/// Remove a directory entry from a parent directory inode.
/// Would not reclaim the inode or data blocks of the removed entry, caller responsible for that.
/// Returns the inode ID of the removed entry if successful, or an error if not found or if the parent is not a directory.
pub fn dir_rm_entry(
    device: &impl BlockDevice,
    superblock: &mut SuperBlock,
    parent_inode: &mut Inode, // Parent directory inode
    name: &[u8],
) -> Result<u32> {
    if parent_inode.ftype != FileType::Directory {
        return Err(FsError::NotDirectory);
    }
    if name.len() == 0 || name.len() > MAX_FILE_NAME_LEN {
        return Err(FsError::InvalidFileName);
    }
    // For simplicity, don't do too much validation on the name.
    if name_cmp(name, DOT_NAME) || name_cmp(name, DOTDOT_NAME) {
        return Err(FsError::InvalidFileName);
    }

    let num_dirents = (parent_inode.size / DIR_ENTRY_SIZE as u64) as usize;
    let mut num_looked_up = 0;
    let num_blocks = parent_inode.blocks;
    
    let mut cur_block_buf = Box::new([0u8; BLOCK_SIZE]);
    let mut inode_id_to_remove = None;
    let mut block_id_to_modify = None;
    let mut block_inner_offset = 0;
    'found_entry: for i in 0..num_blocks as usize {
        println!("[dir_rm_entry] Checking block {} for entry {}", i, String::from_utf8_lossy(name));
        let block_id = bmap(
            device, 
            superblock, 
            parent_inode, 
            (i * BLOCK_SIZE) as u64, 
            false
        )?;
        device.read_block(block_id, cur_block_buf.as_mut())?;
        for j in 0..NUM_ENTRY_PER_BLOCK {
            if num_looked_up >= num_dirents {
                break; // No more entries to check
            }
            let cur_entry_offset = j * DIR_ENTRY_SIZE;
            let entry_ptr = unsafe {
                cur_block_buf.as_mut_ptr().add(cur_entry_offset) as *mut DirEntry
            };
            let entry = unsafe { &mut *entry_ptr };
            if entry.inode_id == 0 {
                continue; // Empty entry, skip
            }
            num_looked_up += 1;
            if name_cmp(&entry.name, name) {
                assert!(entry.inode_id != 0, "Entry should have a valid inode ID");
                inode_id_to_remove = Some(entry.inode_id);
                block_id_to_modify = Some(block_id);
                block_inner_offset = cur_entry_offset;
                break 'found_entry;
            }
        }
    }

    if block_id_to_modify.is_none() {
        println!("Entry not found: {}", String::from_utf8_lossy(name));
        return Err(FsError::NotFound);
    }

    let dest_ptr = unsafe {
        cur_block_buf.as_mut_ptr().add(block_inner_offset) as *mut DirEntry
    };
    unsafe {
        dest_ptr.write_unaligned(DirEntry::NULL);
    }

    // For simplicity, no possible reclaiming of data blocks.
    parent_inode.size -= DIR_ENTRY_SIZE as u64;
    // parent_inode.blocks = ((parent_inode.size + BLOCK_SIZE as u64 - 1) / BLOCK_SIZE as u64) as u32;

    write_inode(device, superblock, &parent_inode)?;
    device.write_block(block_id_to_modify.unwrap(), cur_block_buf.as_ref())?;

    // If the inode's links reaches 0 after this operation, caller should reclaim the inode.
    Ok(inode_id_to_remove.unwrap())
}

pub fn dir_is_empty(
    device: &impl BlockDevice,
    superblock: &mut SuperBlock,
    dir_inode: &Inode,
) -> Result<bool> {
    if dir_inode.ftype != FileType::Directory {
        return Err(FsError::NotDirectory);
    }
    
    // Check if the directory has any entries other than '.' and '..'
    let num_dirents = (dir_inode.size / DIR_ENTRY_SIZE as u64) as usize;
    println!("Checking if directory {} is empty, num dirents: {}", 
        dir_inode.id, num_dirents);
    if num_dirents == 2 {
        return Ok(true); // Only '.' and '..' entries
    } else if num_dirents < 2 {
        panic!("Directory should have at least '.' and '..' entries");
    } else {
        Ok(false)
    }
}

/// Create a new directory with the given name in the parent directory inode.
/// The new directory will be created with an initial inode and a '.' and '..' entry.
/// Returns the inode ID of the new directory if successful, or an error if the parent is not a directory or if the name is invalid.
pub fn mkdir(
    device: &impl BlockDevice,
    superblock: &mut SuperBlock,
    parent_inode: &mut Inode,
    dir_name: &[u8],
) -> Result<u32> {
    if parent_inode.ftype != FileType::Directory {
        return Err(FsError::NotDirectory);
    }
    if dir_name.len() == 0 || dir_name.len() > MAX_FILE_NAME_LEN {
        return Err(FsError::InvalidFileName);
    }
    if name_cmp(dir_name, DOT_NAME) || name_cmp(dir_name, DOTDOT_NAME) {
        return Err(FsError::InvalidFileName);
    }

    // Check if the directory already exists
    if let Ok(_) = dir_lookup(device, superblock, parent_inode, dir_name) {
        return Err(FsError::AlreadyExists);
    }

    let mut dir_inode = alloc_inode(
        device, 
        superblock, 
        FileType::Directory, 
        Mode::RW
    )?;
    let dir_inode_id = dir_inode.id;

    dir_add_entry(
        device, 
        superblock, 
        parent_inode, 
        &DirEntry::new(dir_inode_id, dir_name)?
    )?;
    dir_inode.links_cnt += 1;
    dir_add_entry(
        device, 
        superblock, 
        &mut dir_inode, 
        &DirEntry::new(dir_inode_id, DOT_NAME)?
    )?;
    dir_inode.links_cnt += 1; // '.' entry counts as a link
    dir_add_entry(
        device, 
        superblock, 
        &mut dir_inode, 
        &DirEntry::new(parent_inode.id, DOTDOT_NAME)?
    )?;
    parent_inode.links_cnt += 1; // '..' entry counts as a link
    assert!(dir_inode.size == 2 * DIR_ENTRY_SIZE as u64);
    assert!(dir_inode.blocks == 1);
    write_inode(device, superblock, &parent_inode)?;
    write_inode(device, superblock, &dir_inode)?;

    Ok(dir_inode_id)
}

pub fn read_dir(
    device: &impl BlockDevice,
    superblock: &mut SuperBlock,
    dir_inode: &mut Inode,
) -> Result<Vec<DirEntry>> {
    if dir_inode.ftype != FileType::Directory {
        return Err(FsError::NotDirectory);
    }

    let num_dirents = (dir_inode.size / DIR_ENTRY_SIZE as u64) as usize;
    let num_blocks = (dir_inode.size + BLOCK_SIZE as u64 - 1) / BLOCK_SIZE as u64;

    println!("Reading directory: {}, num dirents: {}, num blocks: {}", 
        dir_inode.id, num_dirents, num_blocks);

    let mut entries = vec![];
    let mut cur_block_buf = Box::new([0u8; BLOCK_SIZE]);

    for i in 0..num_blocks as usize {
        let block_id = bmap(
            device,
            superblock,
            dir_inode,
            i as u64 * BLOCK_SIZE as u64,
            false,
        )?;
        device.read_block(block_id, cur_block_buf.as_mut())?;
        for j in 0..NUM_ENTRY_PER_BLOCK {
            if i * NUM_ENTRY_PER_BLOCK + j >= num_dirents {
                break; // No more entries to read
            }
            let cur_entry_offset = j * DIR_ENTRY_SIZE;
            let entry_ptr = unsafe {
                cur_block_buf.as_mut_ptr().add(cur_entry_offset) as *mut DirEntry
            };
            let entry = unsafe { &*entry_ptr };
            
            if entry.inode_id != 0 || !name_is_empty(&entry.name) {
                entries.push(*entry);
            }
        }
    }

    Ok(entries)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_name_cmp() {
        assert_eq!(name_cmp(b"test", b"test"), true);
        assert_eq!(name_cmp(b"test", b"test1"), false);
        assert_eq!(name_cmp(b"test", b"tes"), false);
    }
}