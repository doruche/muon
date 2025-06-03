//! Encapsulation of inode operations.

use alloc::boxed::Box;

use crate::{bitmap::free_data_block, bmap, write_inode, BlockDevice, Error, FileType, Inode, Result, SuperBlock, BLOCK_SIZE, PTRS_PER_BLOCK};

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
    let mut current_offset = offset;
    let mut current_relative_block_id = current_offset / BLOCK_SIZE;
    let mut remain_buf_len = buffer.len();
    let mut block_buf = Box::new([0u8; BLOCK_SIZE]);

    while remain_buf_len > 0 {
        let bytes_to_read = BLOCK_SIZE.min(remain_buf_len).min(inode.size as usize - bytes_read - offset);
        if bytes_to_read == 0  {
            break;
        }
        let current_block_id = match bmap(
            device,
            superblock,
            inode,
            current_relative_block_id as u64 * BLOCK_SIZE as u64,
            true,
        ) {
            Ok(block_id) => block_id,
            Err(Error::OutOfBounds) => {
                // If we reach beyond the file size, we stop reading.
                break;
            }
            Err(e) => return Err(e),
        };
        
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

pub fn ftruncate(
    device: &impl BlockDevice,
    superblock: &mut SuperBlock,
    inode: &mut Inode,
) -> Result<()> {
    if inode.ftype != FileType::Regular {
        return Err(Error::NotWritable);
    }

    let blk_ptr = inode.get_block_ptrs_mut()?;
    for direct_blk in blk_ptr.direct.iter_mut() {
        if let Some(block_id) = *direct_blk {
            free_data_block(device, superblock, block_id)?;
            *direct_blk = None;
        }
    }
    if let Some(indirect_block) = blk_ptr.indirect {
        let mut indirect_ptr_buf = Box::new([0u8; BLOCK_SIZE]);
        device.read_block(indirect_block, indirect_ptr_buf.as_mut())?;
        
        let ptrs = unsafe {
            core::slice::from_raw_parts_mut(
                indirect_ptr_buf.as_mut_ptr() as *mut u32,
                PTRS_PER_BLOCK
            )
        };
        
        for &block_id in ptrs.iter() {
            if block_id != 0 {
                free_data_block(device, superblock, block_id)?;
            }
        }

        free_data_block(device, superblock, indirect_block)?;
        blk_ptr.indirect = None;
    }

    inode.blocks = 0;
    inode.size = 0;
    write_inode(device, superblock, inode)?;
    
    Ok(())
}