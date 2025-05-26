//! Path resolution and manipulation utilities.

use alloc::{boxed::Box, vec::Vec};

use crate::{directory::dir_lookup, get_inode, BlockDevice, Error, FileType, Result, SuperBlock, ROOT_INODE_ID};

/// Resolves a path to inode ids.
/// Returns a tuple of (parent inode id, file inode id).
pub fn resolve(
    device: &impl BlockDevice,
    superblock: &mut SuperBlock,
    path: &str,
) -> Result<(u32, u32)> { 
    if path == "/" {
        return Ok((ROOT_INODE_ID as u32, ROOT_INODE_ID as u32));
    }
 
    let mut current_inode_id = ROOT_INODE_ID as u32;
    let mut current_inode = Box::new(get_inode(device, superblock, current_inode_id)?);
    let mut parent_inode_id = ROOT_INODE_ID as u32;

    let components: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    for (i, component) in components.iter().enumerate() {
        if i == components.len() - 1 {
            if current_inode.ftype != FileType::Directory {
                return Err(Error::NotDirectory);
            }
            parent_inode_id = current_inode_id;
            current_inode_id = dir_lookup(
                device, 
                superblock, 
                &mut current_inode, 
                component.as_bytes(),
            )?;
            return Ok((parent_inode_id, current_inode_id));
        } else {
            if current_inode.ftype != FileType::Directory {
                return Err(Error::NotDirectory);
            }
            parent_inode_id = current_inode_id;
            current_inode_id = dir_lookup(
                device, 
                superblock, 
                &mut current_inode, 
                component.as_bytes(),
            )?;
            current_inode = Box::new(get_inode(device, superblock, current_inode_id)?);
        }
    }

    Err(Error::InvalidPath)
}