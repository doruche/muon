//! Path resolution and manipulation utilities.

use alloc::{boxed::Box, string::{String, ToString}, vec::Vec};

use crate::{directory::dir_lookup, get_inode, BlockDevice, Error, FileType, Result, SuperBlock, DOTDOT_NAME, DOT_NAME, ROOT_INODE_ID};

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
        // if component == "." {
        //     if i == components.len() - 1 {
        //         return Ok((parent_inode_id, current_inode_id));
        //     }
        //     continue;
        // }
        
        if current_inode.ftype != FileType::Directory {
            return Err(Error::NotDirectory);
        }
        
        parent_inode_id = current_inode_id;
        let next_inode_id = dir_lookup(
            device, 
            superblock, 
            &mut current_inode, 
            component.as_bytes(),
        )?;

        if i == components.len() - 1 {
            return Ok((parent_inode_id, next_inode_id));
        }

        current_inode_id = next_inode_id;
        current_inode = get_inode(device, superblock, current_inode_id)?;
    }

    Err(Error::NotFound)
}

/// Splits a path into its directory and file name components.
/// Always absolute paths are expected.
/// If multiple slashes are present, they are treated as a single separator.
/// eg. "/home/user/file.txt" -> ("/home/user", "file.txt")
///     "/file.txt" -> ("/", "file.txt")
pub fn split(path: &str) -> Result<(String, String)> {
    if !path.starts_with('/') {
        return Err(Error::InvalidPath);
    }

    let mut components: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if components.is_empty() {
        return Ok(("/".to_string(), String::new()));
    }

    let file_name = components.pop().unwrap_or("");
    let dir_path = components.join("/");
    
    if dir_path.is_empty() {
        Ok(("/".to_string(), file_name.to_string()))
    } else {
        Ok((format!("/{}", dir_path), file_name.to_string()))
    }
}
#[cfg(test)]
mod test {

    use super::*;

    #[test]
    fn test_split() {
        let (dir, file) = split("/home/user/file.txt").unwrap();
        assert_eq!(dir, "/home/user");
        assert_eq!(file, "file.txt");

        let (dir, file) = split("/file.txt").unwrap();
        assert_eq!(dir, "/");
        assert_eq!(file, "file.txt");

        let (dir, file) = split("/").unwrap();
        assert_eq!(dir, "/");
        assert_eq!(file, "");
    }

    #[test]
    fn test_split_2() {
        let (dir, file) = split("/home/user//file.txt").unwrap();
        assert_eq!(dir, "/home/user");
        assert_eq!(file, "file.txt");

        let (dir, file) = split("//file.txt").unwrap();
        assert_eq!(dir, "/");
        assert_eq!(file, "file.txt");

        let (dir, file) = split("///").unwrap();
        assert_eq!(dir, "/");
        assert_eq!(file, "");
    }
}