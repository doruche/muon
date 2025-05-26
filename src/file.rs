//! Encapsulation of inode operations.

use alloc::boxed::Box;

use crate::{bmap, write_inode, BlockDevice, Error, FileType, Inode, Result, SuperBlock, BLOCK_SIZE};

/// Reads data from a file into the provided buffer.
/// The `offset` is the position in the file to start reading from.
/// Returns the number of bytes read, or an error if the operation fails.
pub fn fread(
    device: &impl BlockDevice,
    superblock: &mut SuperBlock,
    inode: &mut Inode,
    offset: usize,
    buffer: &mut [u8],
) -> Result<usize> {
    if inode.ftype != FileType::Regular {
        return Err(Error::NotReadable);
    }

    let mut bytes_read = 0;
    let mut current_offset = offset as u64;
    let mut remain_buf_len = buffer.len();
    let mut block_buf = Box::new([0u8; BLOCK_SIZE]);

    while remain_buf_len > 0 && current_offset < inode.size as u64 {
        let block_id = bmap(device, superblock, inode, current_offset, false)?;
        device.read_block(block_id, block_buf.as_mut())?;

        let bytes_in_cur_block = BLOCK_SIZE.min(inode.size as usize - current_offset as usize);
        let bytes_to_copy = bytes_in_cur_block.min(remain_buf_len);
        buffer[bytes_read..bytes_read + bytes_to_copy]
            .copy_from_slice(&block_buf[..bytes_to_copy]);
        bytes_read += bytes_to_copy;
        remain_buf_len -= bytes_to_copy;
        current_offset += bytes_to_copy as u64;
    }

    Ok(bytes_read)
}

/// Writes data from the provided buffer to a file at the specified offset.
/// Returns the number of bytes written, or an error if the operation fails.
pub fn fwrite(
    device: &impl BlockDevice,
    superblock: &mut SuperBlock,
    inode: &mut Inode,
    offset: usize,
    buffer: &[u8],
) -> Result<usize> {
    if inode.ftype != FileType::Regular {
        return Err(Error::NotWritable);
    }
    if buffer.is_empty() {
        return Ok(0);
    }

    let mut bytes_written = 0;
    let mut current_offset = offset as u64;
    let mut remain_buf_len = buffer.len();
    let mut block_buf = Box::new([0u8; BLOCK_SIZE]);

    while remain_buf_len > 0 {
        let block_id = bmap(device, superblock, inode, current_offset, true)?;
        device.read_block(block_id, block_buf.as_mut())?;

        let bytes_in_cur_block = BLOCK_SIZE.min(inode.size as usize - current_offset as usize);
        let bytes_to_copy = bytes_in_cur_block.min(remain_buf_len);
        
        block_buf[..bytes_to_copy].copy_from_slice(&buffer[bytes_written..bytes_written + bytes_to_copy]);
        device.write_block(block_id, block_buf.as_ref())?;

        bytes_written += bytes_to_copy;
        remain_buf_len -= bytes_to_copy;
        current_offset += bytes_to_copy as u64;
    }

    if current_offset > inode.size as u64 {
        // inode.blocks already updated in bmap
        inode.size = current_offset as u64;
    }
    write_inode(device, superblock, inode)?;

    Ok(bytes_written)
}
