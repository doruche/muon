//! Encapsulation of inode operations.

use crate::{BlockDevice, Inode, Result, SuperBlock};

pub fn fread(
    device: &impl BlockDevice,
    superblock: &mut SuperBlock,
    inode: &mut Inode,
    offset: usize,
    buffer: &mut [u8],
) -> Result<usize> {
    todo!()
}

pub fn fwrite(
    device: &impl BlockDevice,
    superblock: &mut SuperBlock,
    inode: &mut Inode,
    offset: usize,
    buffer: &[u8],
) -> Result<usize> {
    todo!()
}

pub fn ftruncate(
    device: &impl BlockDevice,
    superblock: &mut SuperBlock,
    inode: &mut Inode,
    size: usize,
) -> Result<()> {
    todo!()
}