use alloc::boxed::Box;
use alloc::vec;

use crate::{bmap, write_inode, write_superblock, BlockDevice};
use crate::error::{FsError, Result};
use crate::config::*;
use crate::structs::*;

/// Search for inode id of a file in a parent directory inode.
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
    let num_blocks = (parent_inode.size + BLOCK_SIZE as u64 - 1) / BLOCK_SIZE as u64;

    for i in 0..num_blocks as usize {
        let block_id = bmap(
            device,
            superblock,
            parent_inode,
            i as u64 * BLOCK_SIZE as u64,
            false,
        )?;
        let mut direntries_buf = Box::new([0u8; BLOCK_SIZE]);
        device.read_block(block_id as usize, direntries_buf.as_mut())?;
        for j in 0..NUM_ENTRY_PER_BLOCK {
            if i * NUM_ENTRY_PER_BLOCK + j >= num_dirents {
                break; // No more entries to check
            }
            let cur_entry_offset = j * DIR_ENTRY_SIZE;
            let entry_ptr = unsafe {
                direntries_buf.as_mut_ptr().add(cur_entry_offset) as *mut DirEntry
            };
            let entry = unsafe { &*entry_ptr };
            
            let mut current_name_len = 0;
            for k in 0..MAX_FILE_NAME_LEN {
                if entry.name[k] == 0 {
                    break; // End of name
                }
                current_name_len += 1;
            }
            if current_name_len == name.len() &&
            entry.name[..current_name_len] == name[..] {
                // Found the entry
                return Ok(entry.inode_id);
            }
        }   
    }
    Err(FsError::NotFound)
}

pub fn dir_add_entry(
    device: &impl BlockDevice,
    superblock: &mut SuperBlock,
    parent_inode: &mut Inode, // Parent directory inode
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
    let num_blocks = (prev_size + BLOCK_SIZE as u64 - 1) / BLOCK_SIZE as u64;

    'found_slot: for i in 0..num_blocks as usize {
        let block_id = bmap(
            device,
            superblock,
            parent_inode,
            i as u64 * BLOCK_SIZE as u64,
            false,
        )?;
        device.read_block(block_id as usize, cur_block_buf.as_mut())?;

        for j in 0..NUM_ENTRY_PER_BLOCK {
            let cur_dirent_offset = j * DIR_ENTRY_SIZE;
            let dirent_ptr = unsafe {
                cur_block_buf.as_mut_ptr().add(cur_dirent_offset) as *mut DirEntry
            };
            let dirent = unsafe { &*dirent_ptr };
            if dirent.inode_id == 0 {
                // Found an empty slot
                block_id_to_write = block_id;
                block_inner_offset = cur_dirent_offset;
                break 'found_slot;
            }
        }
    }

    if block_id_to_write == 0 {
        // Allocate a new block for the directory entry
        let new_size = prev_size + DIR_ENTRY_SIZE as u64;
        if new_size > MAX_FSIZE as u64 {
            return Err(FsError::OutOfSpace);
        }
        block_id_to_write = bmap(
            device,
            superblock,
            parent_inode,
            prev_size,
            true,
        )?;
        block_inner_offset = (prev_size % BLOCK_SIZE as u64) as usize;
        parent_inode.size = new_size;
        parent_inode.blocks = ((new_size + BLOCK_SIZE as u64 - 1) / BLOCK_SIZE as u64) as u32;
        device.read_block(block_id_to_write as usize, cur_block_buf.as_mut())?;
    }

    let dest_ptr = unsafe {
        cur_block_buf.as_mut_ptr().add(block_inner_offset) as *mut DirEntry
    };
    unsafe {
        core::ptr::copy_nonoverlapping(
            child_entry as *const _, 
            dest_ptr, 
            1
        );
    }
    device.write_block(block_id_to_write as usize, cur_block_buf.as_ref())?;

    write_inode(device, superblock, &parent_inode)?;

    Ok(())
}

/// Remove a directory entry from a parent directory inode.
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
    if name == DOT_NAME || name == DOTDOT_NAME {
        return Err(FsError::InvalidFileName);
    }

    let inode_id = dir_lookup(device, superblock, parent_inode, name)?;

    let num_dirents = (parent_inode.size / DIR_ENTRY_SIZE as u64) as usize;
    let num_blocks = (parent_inode.size + BLOCK_SIZE as u64 - 1) / BLOCK_SIZE as u64;
    
    let mut cur_block_buf = Box::new([0u8; BLOCK_SIZE]);
    let mut inode_id_to_remove = 0;
    let mut block_id_to_modify = 0;
    let mut block_inner_offset = 0;

    'found_entry: for i in 0..num_blocks as usize {
        let block_id = bmap(
            device, 
            superblock, 
            parent_inode, 
            (i * BLOCK_SIZE) as u64, 
            false
        )?;
        device.read_block(block_id as usize, cur_block_buf.as_mut())?;
        for j in 0..NUM_ENTRY_PER_BLOCK {
            if i * NUM_ENTRY_PER_BLOCK + j >= num_dirents {
                break 'found_entry;
            }
            let cur_entry_offset = j * DIR_ENTRY_SIZE;
            let entry_ptr = unsafe {
                cur_block_buf.as_mut_ptr().add(cur_entry_offset) as *mut DirEntry
            };
            let entry = unsafe { &mut *entry_ptr };
            let mut current_name_len = 0;
            for k in 0..MAX_FILE_NAME_LEN {
                if entry.name[k] == 0 {
                    break; // End of name
                }
                current_name_len += 1;
            }
            if current_name_len == name.len() &&
               entry.name[..current_name_len] == name[..] {
                assert!(entry.inode_id != 0);
                inode_id_to_remove = entry.inode_id;
                block_id_to_modify = block_id;
                block_inner_offset = cur_entry_offset;
                break 'found_entry;
            }
        }
    }

    if block_id_to_modify == 0 {
        return Err(FsError::NotFound);
    }

    let dest_ptr = unsafe {
        cur_block_buf.as_mut_ptr().add(block_inner_offset) as *mut DirEntry
    };
    unsafe {
        core::ptr::copy_nonoverlapping(
            &DirEntry::NULL as *const _, 
            dest_ptr, 
            1
        );
    }

    // For simplicity, no reclaiming of the inode or data blocks.
    parent_inode.size -= DIR_ENTRY_SIZE as u64;
    parent_inode.blocks = ((parent_inode.size + BLOCK_SIZE as u64 - 1) / BLOCK_SIZE as u64) as u32;

    write_inode(device, superblock, &parent_inode)?;
    device.write_block(block_id_to_modify as usize, cur_block_buf.as_ref())?;

    Ok(inode_id_to_remove)
}

pub fn is_dir_empty(
    device: &impl BlockDevice,
    superblock: &mut SuperBlock,
    dir_inode: &Inode,
) -> Result<bool> {
    if dir_inode.ftype != FileType::Directory {
        return Err(FsError::NotDirectory);
    }
    
    // Check if the directory has any entries other than '.' and '..'
    let num_dirents = (dir_inode.size / DIR_ENTRY_SIZE as u64) as usize;
    if num_dirents <= 2 {
        return Ok(true); // Only '.' and '..' entries
    } else {
        return Ok(false);
    }
}