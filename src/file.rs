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

    if buffer.len() + offset > inode.size as usize {
        return Err(Error::OutOfBounds);
    }

    let mut bytes_read = 0;
    let mut current_offset = offset;
    let mut current_relative_block_id = current_offset / BLOCK_SIZE;
    let mut remain_buf_len = buffer.len();
    let mut block_buf = Box::new([0u8; BLOCK_SIZE]);

    while remain_buf_len > 0 {
        let bytes_to_read = BLOCK_SIZE.min(remain_buf_len);
        if bytes_to_read == 0 {
            break;
        }
        let current_block_id = bmap(
            device,
            superblock,
            inode,
            current_relative_block_id as u64 * BLOCK_SIZE as u64,
            false,
        )?;
        
        device.read_block(current_block_id, block_buf.as_mut())?;
        let start_offset = current_offset % BLOCK_SIZE;
        let end_offset = start_offset + bytes_to_read;
        buffer[bytes_read..bytes_read + bytes_to_read]
            .copy_from_slice(&block_buf[start_offset..end_offset]);
        
        bytes_read += bytes_to_read;
        remain_buf_len -= bytes_to_read;
        current_offset += bytes_to_read;
        current_relative_block_id = current_offset / BLOCK_SIZE;
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
    let mut current_offset = offset;
    let mut current_relative_block_id = current_offset / BLOCK_SIZE;
    let mut remain_buf_len = buffer.len();
    let mut block_buf = Box::new([0u8; BLOCK_SIZE]);

    while remain_buf_len > 0 {
        let bytes_to_write = BLOCK_SIZE.min(remain_buf_len);
        if bytes_to_write == 0 {
            break;
        }
        let current_block_id = bmap(
            device,
            superblock,
            inode,
            current_relative_block_id as u64 * BLOCK_SIZE as u64,
            true,
        )?;
        
        device.read_block(current_block_id, block_buf.as_mut())?;
        let start_offset = current_offset % BLOCK_SIZE;
        block_buf[start_offset..start_offset + bytes_to_write]
            .copy_from_slice(&buffer[bytes_written..bytes_written + bytes_to_write]);
        device.write_block(current_block_id, block_buf.as_ref())?;
        bytes_written += bytes_to_write;
        remain_buf_len -= bytes_to_write;
        current_offset += bytes_to_write;
        current_relative_block_id = current_offset / BLOCK_SIZE;
    }

    if current_offset >= inode.size as usize {
        inode.size = current_offset as u64;
        write_inode(device, superblock, inode)?;
    }

    Ok(bytes_written)
}
