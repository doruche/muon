use alloc::boxed::Box;

use crate::{error::FsError, BlockDevice, SuperBlock};
use crate::{config::*, Result};


pub fn read_superblock<D: BlockDevice>(device: &D) -> Result<SuperBlock> {
    let mut buf = Box::new([0u8; BLOCK_SIZE]);
    device.read_block(SUPERBLOCK_ID, buf.as_mut_slice());
    let superblock: SuperBlock = unsafe {
        core::ptr::read_unaligned(buf.as_ptr() as *const SuperBlock)
    };
    
    // Here we simply check the magic number and block size, for conceptual purposes.
    if superblock.magic != MAGIC {
        return Err(FsError::InvalidSuperBlock);
    }
    if superblock.block_size != BLOCK_SIZE as u32 {
        return Err(FsError::InvalidSuperBlock);
    }

    Ok(superblock)
}

pub fn write_superblock<D: BlockDevice>(device: &D, superblock: &SuperBlock) -> Result<()> {
    let mut buf = Box::new([0u8; BLOCK_SIZE]);
    unsafe {
        core::ptr::write_unaligned(
            buf.as_mut_ptr() as *mut SuperBlock,
            *superblock
        );
    }
    device.write_block(SUPERBLOCK_ID, buf.as_ref())?;
    device.flush()?;
    Ok(())
}

impl SuperBlock {
    pub fn new(num_blocks: u32, num_inodes: u32) -> Result<Self> {
        if num_blocks == 0 || num_inodes == 0 {
            return Err(FsError::InvalidSuperBlock);
        }

        let data_bitmap_start = SUPERBLOCK_ID as u32 + 1;
        // Not a precise calculation, for data region actually starts after superblock, 2 bitmaps and inode table, but enough.
        let data_bitmap_blocks = (num_blocks + 7) / 8;

        let inode_bitmap_start = data_bitmap_start + data_bitmap_blocks;
        let inode_bitmap_blocks = (num_inodes + 7) / 8;
        
        let inode_table_start = inode_bitmap_start + inode_bitmap_blocks;
        let inodes_per_block = (BLOCK_SIZE / INODE_SIZE) as u32;
        let inode_table_blocks = (num_inodes + inodes_per_block - 1) / inodes_per_block;

        let data_start = inode_table_start + inode_table_blocks;
        let free_blocks = num_blocks - data_start;

        // Simple sanity check for the number of blocks.
        if free_blocks <= 0 {
            return Err(FsError::InvalidSuperBlock);
        }

        Ok(SuperBlock { 
            magic: MAGIC, 
            num_blocks, 
            block_size: BLOCK_SIZE as u32, 
            free_blocks, 
            num_inodes,
            free_inodes: num_inodes,
            root_inode: ROOT_INODE_ID as u32, 
            data_bitmap_start, 
            data_bitmap_blocks, 
            inode_bitmap_start, 
            inode_bitmap_blocks, 
            inode_table_start, 
            inode_table_blocks, 
            data_start, 
            reserved: [0; BLOCK_SIZE - 14 * 4], 
        })
    }
}