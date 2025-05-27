//! Management of reading and writing to inodes in inode table.

use alloc::boxed::Box;
use alloc::vec;

use crate::{bitmap, FileType, Inode, Mode, Result, SuperBlock, BLOCK_SIZE, INODE_SIZE, NUM_DIRECT_PTRS, PTRS_PER_BLOCK};
use crate::BlockDevice;
use crate::error::FsError;
use crate::bitmap::{alloc_data_block, free_data_block};

/// Query an inode by its ID.
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
    let mut buf = Box::new([0u8; BLOCK_SIZE]);
    device.read_block(block_id, buf.as_mut())?;
    
    let inode = unsafe {
        core::ptr::read_unaligned(buf.as_ptr().add(block_inner_offset as usize) as *const Inode)
    };

    Ok(inode)
}

/// Write an inode to inode table.
/// The inode must be allocated by 'alloc_inode'.
/// This means user can only modify existing inodes, not create new ones with this function.
pub fn write_inode(
    device: &impl BlockDevice,
    superblock: &SuperBlock,
    inode: &Inode
) -> Result<()> {
    let block_id = superblock.inode_table_start + (inode.id / (BLOCK_SIZE / INODE_SIZE) as u32);
    let block_inner_offset = (inode.id % (BLOCK_SIZE / INODE_SIZE) as u32) * INODE_SIZE as u32;
    let mut buf = Box::new([0u8; BLOCK_SIZE]);
    device.read_block(block_id, buf.as_mut())?;
    unsafe {
        core::ptr::write_unaligned(
            buf.as_mut_ptr().add(block_inner_offset as usize) as *mut Inode,
            *inode
        );
    }
    device.write_block(block_id, buf.as_ref())?;
    Ok(())
}

pub fn alloc_inode(
    device: &impl BlockDevice,
    superblock: &mut SuperBlock,
    ftype: FileType,
    mode: Mode,
) -> Result<Inode> {
    let id = bitmap::alloc_inode_id(device, superblock)?;
    // Superblock already updated by alloc_inode_id.
    let inode = Inode {
        id,
        ftype,
        blocks: 0,
        links_cnt: 0, // Increased by the first link.
        size: 0,
        indirect_ptr: None,
        direct_ptrs: [None; NUM_DIRECT_PTRS],
    };
    write_inode(device, superblock, &inode)?;
    Ok(inode)
}

/// Frees an inode and all its data blocks, clearing according bitmap entry.
/// This function does not remove the inode from the directory entries.
/// Make sure to remove the inode from the directory entries before calling this function.
/// Returns the freed inode.
pub fn free_inode(
    device: &impl BlockDevice,
    superblock: &mut SuperBlock,
    inode_id: u32
) -> Result<Inode> {
    let inode = get_inode(device, superblock, inode_id)?;

    //Direct blocks
    for block_id in inode.direct_ptrs.iter() {
        if let Some(block_id) = block_id {
            free_data_block(device, superblock, *block_id)?;
        }
    }
    
    //Indirect block
    if let Some(indirect_ptr) = inode.indirect_ptr {
        let mut ptr_buf = Box::new([0u8; BLOCK_SIZE]);
        device.read_block(indirect_ptr, ptr_buf.as_mut())?;
        let ptrs = unsafe {
            core::slice::from_raw_parts_mut(
                ptr_buf.as_mut_ptr() as *mut u32,
                PTRS_PER_BLOCK
            )
        };
        for &block_id in ptrs.iter() {
            if block_id != 0 {
                free_data_block(device, superblock, block_id)?;
            }
        }
        free_data_block(device, superblock, indirect_ptr)?;
    }

    bitmap::free_inode_id(device, superblock, inode_id)?;

    // Free inode record in inode table.
    write_inode(device, superblock, &Inode {
        id: inode_id,
        ..Inode::ZERO
    })?;

    Ok(inode)
}


/// Block map. Maps a file offset to a block ID in the filesystem.
/// The offset is required to be divided by BLOCK_SIZE.
pub fn bmap(
    device: &impl BlockDevice,
    superblock: &mut SuperBlock,
    inode: &mut Inode,
    file_offset: u64,
    create: bool,
) -> Result<u32> {
    if file_offset % BLOCK_SIZE as u64 != 0 {
        return Err(FsError::InvalidArgument);
    }

    let block_offset = file_offset / BLOCK_SIZE as u64;

    if create && file_offset >= inode.size {
        inode.size = file_offset + 1;
        inode.blocks = ((inode.size + BLOCK_SIZE as u64 - 1) / BLOCK_SIZE as u64) as u32;
    }

    // Direct blocks
    if block_offset < NUM_DIRECT_PTRS as u64 {
        let block_id = match inode.direct_ptrs[block_offset as usize] {
            Some(block_id) => {
                block_id
            },
            None if create => {
                let block_id = alloc_data_block(device, superblock)?;
                inode.direct_ptrs[block_offset as usize] = Some(block_id);
                inode.blocks += 1;
                write_inode(device, superblock, &inode)?;
                block_id
            },
            _ => return Err(FsError::OutOfBounds),
        };
        return Ok(block_id);
    }

    // Indirect blocks
    let indirect_offset = block_offset - NUM_DIRECT_PTRS as u64;
    if indirect_offset < PTRS_PER_BLOCK as u64 {
        let indirect_block_id =  match inode.indirect_ptr {
            Some(indirect_block_id) => {
                indirect_block_id
            },
            None if create => {
                let indirect_block_id = alloc_data_block(device, superblock)?;
                inode.indirect_ptr = Some(indirect_block_id);
                indirect_block_id
            },
            _ => return Err(FsError::OutOfBounds),
        };

        let mut indirect_ptr_buf = Box::new([0u8; BLOCK_SIZE]);
        device.read_block(indirect_block_id, indirect_ptr_buf.as_mut())?;

        let ptrs = unsafe {
            core::slice::from_raw_parts_mut(
                indirect_ptr_buf.as_mut_ptr() as *mut u32,
                PTRS_PER_BLOCK
            )
        };

        let mut data_block_id = ptrs[indirect_offset as usize];
        if data_block_id == 0 {
            if !create {
                return Err(FsError::OutOfBounds);
            }
            data_block_id = alloc_data_block(device, superblock)?;
            ptrs[indirect_offset as usize] = data_block_id;
            inode.blocks += 1;
            write_inode(device, superblock, &inode)?;
            // Write back the updated indirect block
            device.write_block(indirect_block_id, indirect_ptr_buf.as_ref())?;
        }

        return Ok(data_block_id);
    } else {
        return Err(FsError::FileTooLarge);
    }
}

