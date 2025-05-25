//! Management of data bitmap and inode bitmap.
//! Data bitmap then uses these pointers to track which blocks are used for according data.
//! Inode bitmap for tracking files' inodes, which tell direct and indirect pointers to data blocks.

use alloc::vec;

use crate::superblock::write_superblock;
use crate::{config::*, BlockDevice, Result, SuperBlock};
use crate::error::FsError;

/// Set the first fit bit in the bitmap.
/// 'Fit' means that it will set the first bit that is not equal to 'value'.
/// Returns the item ID of the bit that was set or cleared.
fn set_first_fit_bit<D: BlockDevice>(
    device: &D,
    bitmap_start: u32,
    bitmap_blocks: u32,
    total_items: u32,
    value: bool, // true for setting first false to true, false for setting first true to false
) -> Result<u32> {
    let mut buf = vec![0; BLOCK_SIZE];

    for i in 0..bitmap_blocks {
        let current_block_id = (bitmap_start + i) as usize;
        device.read_block(current_block_id, buf.as_mut_slice())?;

        for j in 0..BLOCK_SIZE {
            let byte = buf[j];
            for k in 0..8 {
                let current_item_id = i * BLOCK_SIZE as u32 * 8 + j as u32 * 8 + k as u32;
                if current_item_id >= total_items {
                    return Err(FsError::OutOfBounds);
                }
                let is_set = (byte & (1 << k)) != 0;
                if is_set != value {
                    if value {
                        // Set the bit
                        buf[j] |= 1 << k;
                    } else {
                        // Clear the bit
                        buf[j] &= !(1 << k);
                    }
                    device.write_block(current_block_id, buf.as_ref())?;
                    return Ok(current_item_id);
                }
            }
        }
    }

    Err(FsError::NotFound)
}

/// Sets a specific bit in the bitmap to indicate whether an item (like a block or inode) is used or free.
/// 'total_items' seems unnecessary here, but we keep it for forcing bounds checking,
/// otherwise we have to mark this function as unsafe.
/// Returns previously set value of the bit.
fn set_bit_at<D: BlockDevice>(
    device: &D,
    bitmap_start: u32,
    bitmap_blocks: u32,
    item_id: u32,
    total_items: u32,
    set_value: bool,
) -> Result<bool> {
    if item_id >= total_items {
        return Err(FsError::OutOfBounds);
    }

    let block_id = item_id / (BLOCK_SIZE as u32 * 8);
    let byte_offset = (item_id % (BLOCK_SIZE as u32 * 8)) / 8;
    let bit_offset = item_id % 8;
    
    if block_id >= bitmap_blocks {
        return Err(FsError::OutOfBounds);
    }

    let target_block_id = (bitmap_start + block_id) as usize;
    let mut buf = vec![0; BLOCK_SIZE];

    device.read_block(target_block_id, buf.as_mut_slice())?;
    let pre_value = (buf[byte_offset as usize] & (1 << bit_offset)) != 0;
    if set_value {
        buf[byte_offset as usize] |= 1 << bit_offset;
    } else {
        buf[byte_offset as usize] &= !(1 << bit_offset);
    }
    device.write_block(target_block_id, buf.as_ref())?;

    Ok(pre_value)
}

// Public API for managing data bitmap and inode bitmap.

/// Allocates a new data block, setting bit in the data bitmap.
/// Returns the actual block ID of the allocated block.
/// Index in data region can be calculated as 'result - data_start'.
pub fn alloc_data_block<D: BlockDevice>(
    device: &D,
    superblock: &mut SuperBlock,
) -> Result<u32> {
    let block_id = set_first_fit_bit(
        device, 
        superblock.data_bitmap_start, 
        superblock.data_bitmap_blocks, 
        superblock.num_blocks - superblock.data_start,
        true)?;
    superblock.free_blocks -= 1;
    // TODO: Should be done by the upper layer, not here.
    // write_superblock(device, superblock)?;
    Ok(block_id + superblock.data_start)
}

pub fn free_data_block<D: BlockDevice>(
    device: &D,
    superblock: &mut SuperBlock,
    block_id: u32,
) -> Result<()> {
    let relative_block_id = block_id - superblock.data_start;
    if block_id < superblock.data_start || relative_block_id >= superblock.num_blocks - superblock.data_start {
        return Err(FsError::OutOfBounds);
    }

    set_bit_at(
        device, 
        superblock.data_bitmap_start, 
        superblock.data_bitmap_blocks, 
        relative_block_id, 
        superblock.num_blocks - superblock.data_start, 
        false
    )?;
    superblock.free_blocks += 1;
    // write_superblock(device, superblock)?;
    Ok(())
}

pub fn alloc_inode<D: BlockDevice>(
    device: &D,
    superblock: &mut SuperBlock
) -> Result<u32> {
    let inode_id = set_first_fit_bit(
        device, 
        superblock.inode_bitmap_start, 
        superblock.inode_bitmap_blocks, 
        superblock.num_inodes,
        true)?;
    superblock.free_inodes -= 1;
    // write_superblock(device, superblock)?;
    Ok(inode_id)
}

pub fn free_inode<D: BlockDevice>(
    device: &D,
    superblock: &mut SuperBlock,
    inode_id: u32,
) -> Result<()> {
    set_bit_at(
        device, 
        superblock.inode_bitmap_start, 
        superblock.inode_bitmap_blocks, 
        inode_id, 
        superblock.num_inodes, 
        false
    )?;
    superblock.free_inodes += 1;
    // write_superblock(device, superblock)?;
    Ok(())
}