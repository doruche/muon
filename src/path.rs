//! Path resolution and manipulation utilities.

use std::{collections::VecDeque, f32::consts::E, fs::File};

use alloc::{boxed::Box, string::{String, ToString}, vec::Vec};

use crate::{directory::dir_lookup, get_inode, trim_zero, BlockDevice, Error, FileType, Result, SuperBlock, DOTDOT_NAME, DOT_NAME, ROOT_INODE_ID, SYMLOOP_MAX};


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
    
    let mut canonicalized_path = canonicalize(device, superblock, path, false)?;

    if canonicalized_path == "/" {
        return Ok((ROOT_INODE_ID as u32, ROOT_INODE_ID as u32));
    }
    println!("Canonicalized path for {}: {}", path, canonicalized_path);

    let mut current_inode_id = ROOT_INODE_ID;
    let mut parent_inode_id = ROOT_INODE_ID;
    let mut current_inode = get_inode(device, superblock, current_inode_id)?;
    let mut components = canonicalized_path
        .split('/')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>();


    for (i, component) in components.iter().enumerate() {
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
        
        if i == components.len() - 1 {
            return Ok((parent_inode_id as u32, current_inode_id as u32));
        }

        current_inode = get_inode(device, superblock, current_inode_id)?;
    }

    Err(Error::NotFound)
}

pub fn resolve_without_last(
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
    
    let mut canonicalized_path = canonicalize(device, superblock, path, true)?;

    if canonicalized_path == "/" {
        return Ok((ROOT_INODE_ID as u32, ROOT_INODE_ID as u32));
    }
    println!("Canonicalized path for {}: {}", path, canonicalized_path);

    let mut current_inode_id = ROOT_INODE_ID;
    let mut parent_inode_id = ROOT_INODE_ID;
    let mut current_inode = get_inode(device, superblock, current_inode_id)?;
    let mut components = canonicalized_path
        .split('/')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>();


    for (i, component) in components.iter().enumerate() {
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
        
        if i == components.len() - 1 {
            return Ok((parent_inode_id as u32, current_inode_id as u32));
        }

        current_inode = get_inode(device, superblock, current_inode_id)?;
    }

    Err(Error::NotFound)
}

pub fn canonicalize(
    device: &impl BlockDevice,
    superblock: &mut SuperBlock,
    path: &str,
    not_cano_last_symlink: bool,
) -> Result<String> {
    if !path.starts_with("/") {
        return Err(Error::InvalidPath);
    }

    if path == "/" {
        return Ok("/".to_string());
    }

    let mut canonical_components = VecDeque::new();
    let mut current_inode_id = ROOT_INODE_ID;
    let mut parent_inode_id = ROOT_INODE_ID;
    let mut current_inode = get_inode(device, superblock, current_inode_id)?;
    let mut link_depth = 0;
    let mut components_to_process: VecDeque<String> = path
        .split('/')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();

    loop {
        if link_depth >= SYMLOOP_MAX {
            return Err(Error::PathTooLong);
        }
        println!("Current components to process: {:?}", components_to_process);
        if components_to_process.is_empty() {
            break;
        }
        let cur_component = components_to_process.pop_front().unwrap();
        if cur_component == "." {
            continue;
        }
        if cur_component == ".." {
            if current_inode_id == ROOT_INODE_ID {
                println!(".. encountered at root, ignoring");
                continue;
            }
            println!(".. encountered, current components: {:?}", canonical_components);
            current_inode_id = parent_inode_id;
            current_inode = get_inode(device, superblock, current_inode_id)?;
            canonical_components.pop_back();
            continue;
        }
        let next_inode_id = dir_lookup(
            device,
            superblock,
            &mut current_inode,
            cur_component.as_bytes(),
        )?;
        println!("Next inode ID for component '{}': {}", cur_component, next_inode_id);
        let next_inode = get_inode(device, superblock, next_inode_id)?;
        if next_inode.ftype == FileType::Symlink {
            if components_to_process.is_empty() && not_cano_last_symlink {
                canonical_components.push_back(cur_component);
                break;
            }

            link_depth += 1;
            let sym_target = trim_zero(next_inode.get_path()?);
            let sym_target_str = String::from_utf8_lossy(sym_target).to_string();
            if sym_target_str.starts_with("/") {
                // Absolute symlink
                canonical_components.clear();
                current_inode_id = ROOT_INODE_ID;
                parent_inode_id = ROOT_INODE_ID;
                current_inode = get_inode(device, superblock, current_inode_id)?;
                
            }
            let mut new_components: VecDeque<String> = sym_target_str
                    .split('/')
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
                    .collect();
            new_components.append(&mut components_to_process);
            components_to_process = new_components;
            println!("Symlink target: {}, new components to process: {:?}", sym_target_str, components_to_process);
            continue;
        }
        if next_inode.ftype != FileType::Directory &&
            !components_to_process.is_empty() {
            return Err(Error::NotDirectory);
        }
        if components_to_process.is_empty() {
            canonical_components.push_back(cur_component);
            break;
        }

        parent_inode_id = current_inode_id;
        current_inode_id = next_inode_id;
        current_inode = next_inode;
        canonical_components.push_back(cur_component);
    }

    if canonical_components.is_empty() {
        return Ok("/".to_string());
    } else {
        Ok(format!(
            "/{}",
            canonical_components
                .into_iter()
                .map(|s| s.trim_matches('/').to_string())
                .collect::<Vec<String>>()
                .join("/")
        ))
    }
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