//! Management of reading and writing to inodes.

use alloc::vec;

use crate::{Inode, Result, SuperBlock, BLOCK_SIZE, INODE_SIZE, NUM_DIRECT_PTRS, PTRS_PER_BLOCK};
use crate::BlockDevice;
use crate::error::FsError;
use crate::bitmap::*;

pub fn get_inode(
    device: &impl BlockDevice,
    superblock: &SuperBlock,
    inode_id: u32,
) -> Result<Inode> {
    if inode_id >= superblock.num_inodes {
        return Err(FsError::OutOfBounds);
    }
    
    let block_id = superblock.inode_table_start + (inode_id / (BLOCK_SIZE / INODE_SIZE) as u32);
    let block_inner_offset = (inode_id % (BLOCK_SIZE / INODE_SIZE) as u32) * INODE_SIZE as u32;
    let mut buf = vec![0; BLOCK_SIZE];
    device.read_block(block_id as usize, buf.as_mut_slice())?;
    
    let inode: Inode = unsafe {
        core::ptr::read_unaligned(buf.as_ptr().add(block_inner_offset as usize) as *const Inode)
    };

    Ok(inode)
}

pub fn write_inode(
    device: &impl BlockDevice,
    superblock: &SuperBlock,
    inode_id: u32,
    inode: &Inode
) -> Result<()> {
    if inode_id >= superblock.num_inodes {
        return Err(FsError::OutOfBounds);
    }

    let block_id = superblock.inode_table_start + (inode_id / (BLOCK_SIZE / INODE_SIZE) as u32);
    let block_inner_offset = (inode_id % (BLOCK_SIZE / INODE_SIZE) as u32) * INODE_SIZE as u32;
    let mut buf = vec![0; BLOCK_SIZE];
    device.read_block(block_id as usize, buf.as_mut_slice())?;
    unsafe {
        core::ptr::write_unaligned(
            buf.as_mut_ptr().add(block_inner_offset as usize) as *mut Inode,
            *inode
        );
    }
    device.write_block(block_id as usize, buf.as_ref())?;
    Ok(())
}

/// Maps a file offset to a block ID in the filesystem.
pub fn bmap(
    device: &impl BlockDevice,
    superblock: &mut SuperBlock,
    inode: &mut Inode,
    file_offset: u64,
    create: bool,
) -> Result<u32> {
    let block_offset = file_offset / BLOCK_SIZE as u64;


    if create && file_offset >= inode.size {
        inode.size = file_offset + 1;
    }

    inode.blocks = ((inode.size + BLOCK_SIZE as u64 - 1) / BLOCK_SIZE as u64) as u32;

    // Direct blocks
    if block_offset < NUM_DIRECT_PTRS as u64 {
        let mut block_id = inode.direct_ptrs[block_offset as usize];
        if block_id == 0 {
            if !create {
                return Err(FsError::NotFound);
            }
            block_id = alloc_data_block(device, superblock)?;
            inode.direct_ptrs[block_offset as usize] = block_id;
            inode.blocks += 1;
        }
        return Ok(block_id);
    }

    // Indirect blocks
    let indirect_offset = block_offset - NUM_DIRECT_PTRS as u64;
    if indirect_offset < PTRS_PER_BLOCK as u64 {
        let mut indirect_block_id = inode.indirect_ptr;
        if indirect_block_id == 0 {
            if !create {
                return Err(FsError::NotFound);
            }
            inode.indirect_ptr = alloc_data_block(device, superblock)?;
            indirect_block_id = inode.indirect_ptr;
            inode.blocks += 1;
            // Zero out the new indirect block
            device.write_block(indirect_block_id as usize, vec![0; BLOCK_SIZE].as_ref())?;
        }

        let mut indirect_ptr_buf = vec![0; BLOCK_SIZE];
        device.read_block(indirect_block_id as usize, indirect_ptr_buf.as_mut_slice())?;

        let ptrs = unsafe {
            core::slice::from_raw_parts_mut(
                indirect_ptr_buf.as_mut_ptr() as *mut u32,
                PTRS_PER_BLOCK
            )
        };
        let mut data_block_id = ptrs[indirect_offset as usize];
        if data_block_id == 0 {
            if !create {
                return Err(FsError::NotFound);
            }
            data_block_id = alloc_data_block(device, superblock)?;
            ptrs[indirect_offset as usize] = data_block_id;
            inode.blocks += 1;
            // Write back the updated indirect block
            device.write_block(indirect_block_id as usize, indirect_ptr_buf.as_ref())?;
        }

        return Ok(data_block_id);
    } else {
        return Err(FsError::FileTooLarge);
    }
}