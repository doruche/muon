use alloc::boxed::Box;

use crate::bitmap::set_inode_allocated;
use crate::{error::FsError, BlockDevice, SuperBlock};
use crate::{config::*, write_inode, Inode, Mode, Result};


pub fn read_superblock<D: BlockDevice>(device: &D) -> Result<SuperBlock> {
    let mut buf = Box::new([0u8; BLOCK_SIZE]);
    device.read_block(SUPERBLOCK_ID as u32, &mut buf);
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
    device.write_block(SUPERBLOCK_ID as u32, buf.as_ref())?;
    device.flush()?;
    Ok(())
}

pub fn format_fs(device: &impl BlockDevice, num_blocks: u32, num_inodes: u32) -> Result<SuperBlock> {
    let mut superblock = SuperBlock::new(num_blocks, num_inodes)?;
    write_superblock(device, &superblock)?;

    let zero_block = Box::new([0u8; BLOCK_SIZE]);
    
    // Zero out metadata blocks
    for i in 0..superblock.data_bitmap_blocks {
        device.write_block(superblock.data_bitmap_start + i, &zero_block)?;
    }
    for i in 0..superblock.inode_bitmap_blocks {
        device.write_block(superblock.inode_bitmap_start + i, &zero_block)?;
    }
    for i in 0..superblock.inode_table_blocks {
        device.write_block(superblock.inode_table_start + i, &zero_block)?;
    }

    // Zero out data blocks
    for i in 0..superblock.free_blocks {
        device.write_block(superblock.data_start + i, &zero_block)?;
    }

    // Initialize root inode
    let mut root_inode = Inode {
        id: ROOT_INODE_ID as u32,
        ftype: crate::FileType::Directory,
        size: 0,
        blocks: 0,
        links_cnt: 1,
        direct_ptrs: [0; NUM_DIRECT_PTRS],
        indirect_ptr: 0,
        reserved: [0; 44],
    };
    write_inode(device, &superblock, &mut root_inode)?;
    set_inode_allocated(device, &mut superblock, ROOT_INODE_ID as u32)?;
    write_superblock(device, &superblock)?;
 
    Ok(superblock)
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
        // Simple sanity check for the number of blocks.
        if num_blocks <= data_start {
            return Err(FsError::InvalidSuperBlock);
        }
        let free_blocks = num_blocks - data_start;
        
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