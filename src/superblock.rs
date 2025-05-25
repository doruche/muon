use alloc::boxed::Box;

use crate::{error::FsError, BlockDevice, SuperBlock};
use crate::config::*;


pub fn read_superblock<D: BlockDevice>(device: &D) -> Result<SuperBlock, FsError> {
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

pub fn write_superblock<D: BlockDevice>(device: &D, superblock: SuperBlock) -> Result<(), FsError> {
    let mut buf = Box::new([0u8; BLOCK_SIZE]);
    unsafe {
        core::ptr::write_unaligned(
            buf.as_mut_ptr() as *mut SuperBlock,
            superblock
        );
    }
    device.write_block(SUPERBLOCK_ID, buf.as_ref())?;
    device.flush()?;
    Ok(())
}