//! Path resolution and manipulation utilities.

use alloc::{boxed::Box, string::{String, ToString}, vec::Vec};

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

    if !path.starts_with('/') {
        return Err(Error::InvalidPath);
    }
 
    println!("[resolve] path: {}", path);
    let mut current_inode_id = ROOT_INODE_ID as u32;
    let mut current_inode = get_inode(device, superblock, current_inode_id)?;
    let mut parent_inode_id = ROOT_INODE_ID as u32;

    let components: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    for (i, &component) in components.iter().enumerate() {
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
            current_inode = get_inode(device, superblock, current_inode_id)?;
        }
    }

    Err(Error::NotFound)
}

/// Splits a path into its directory and file name components.
/// Always absolute paths are expected.
/// eg. "/home/user/file.txt" -> ("/home/user", "file.txt")
///     "/file.txt" -> ("/", "file.txt")
///     "parent/file.txt" -> ("parent", "file.txt")
///     "file.txt" -> ("", "file.txt")
pub fn split(path: &str) -> (String, String) {
    if let Some(pos) = path.rfind('/') {
        if pos == 0 {
            ("/".to_string(), path[1..].to_string())
        } else {
            (path[..pos].to_string(), path[pos + 1..].to_string())
        }
    } else {
        ("".to_string(), path.to_string())
    }
}
#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_split() {
        let (dir, file) = split("/home/user/file.txt");
        assert_eq!(dir, "/home/user");
        assert_eq!(file, "file.txt");

        let (dir, file) = split("file.txt");
        assert_eq!(dir, "");
        assert_eq!(file, "file.txt");

        let (dir, file) = split("/file.txt");
        assert_eq!(dir, "/");
        assert_eq!(file, "file.txt");

        let (dir, file) = split("/");
        assert_eq!(dir, "/");
        assert_eq!(file, "");
    }
}