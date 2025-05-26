use alloc::{string::ToString, sync::Arc, vec::Vec};
use crate::{bitmap::alloc_inode, directory::{dir_add_entry, dir_rm_entry}, file::{fread, fwrite}, free_inode, get_inode, path::{self, resolve, split}, read_superblock, structs::*, superblock, write_inode, write_superblock, BlockDevice, Error, Result, DOTDOT_NAME, DOT_NAME, ROOT_INODE_ID};
use crate::structs::*;
use crate::config::*;

#[derive(Debug)]
pub struct FileSystem<D: BlockDevice> {
    device: Arc<D>,
    superblock: SuperBlock,
}

impl<D: BlockDevice> FileSystem<D> {
    pub fn format(device: Arc<D>, num_blocks: u32, num_inodes: u32) -> Result<Self> {
        let mut fs_inst = Self {
            device: Arc::clone(&device),
            superblock: superblock::format_fs(&*device, num_blocks, num_inodes)?,
        };
        // Create '.' and '..' entries in the root directory
        let mut root_inode = get_inode(&*device, &fs_inst.superblock, ROOT_INODE_ID)?;
        dir_add_entry(
            &*device, 
            &mut fs_inst.superblock, 
            &mut root_inode, 
            &DirEntry::new(ROOT_INODE_ID, DOT_NAME)?
        )?;
        dir_add_entry(
            &*device, 
            &mut fs_inst.superblock, 
            &mut root_inode, 
            &DirEntry::new(ROOT_INODE_ID, DOTDOT_NAME)?
        )?;

        fs_inst.superblock.free_blocks -= 1; // Decrement free blocks for root inode
        write_superblock(&*device, &fs_inst.superblock)?;

        Ok(fs_inst)
    }

    pub fn mount(device: Arc<D>) -> Result<Self> {
        let superblock = read_superblock(&*device)?;
        Ok(Self {
            device,
            superblock,
        })
    }

    // Following methods directly operates on the fs instance, user should wrap a lock around it if needed.
    pub fn open(&mut self, path: &str, mode: Mode, create: bool) -> Result<u32> {
        match resolve(&*self.device, &mut self.superblock, path) {
            Ok((_, inode_id)) => {
                let inode = get_inode(&*self.device, &self.superblock, inode_id)?;
                if inode.ftype != FileType::Regular {
                    return Err(Error::NotRegular);
                }
                Ok(inode_id)
            },
            Err(Error::NotFound) if create => {
                let (parent_path, file_name) = split(path);
                println!("Creating file: {} in parent: {}", file_name, parent_path);
                let (_, parent_inode_id) = resolve(&*self.device, &mut self.superblock, &parent_path)?;
                println!("Parent inode ID: {}", parent_inode_id);
                let mut parent_inode = get_inode(&*self.device, &mut self.superblock, parent_inode_id)?;
                println!("Parent inode type: {:?}", parent_inode.ftype);
                if parent_inode.ftype != FileType::Directory {
                    return Err(Error::NotDirectory);
                }
                let new_inode_id = alloc_inode(&*self.device, &mut self.superblock)?;
                let mut new_inode = get_inode(&*self.device, &mut self.superblock, new_inode_id)?;
                new_inode.ftype = FileType::Regular;
                new_inode.size = 0;
                new_inode.blocks = 0;
                new_inode.id = new_inode_id;
                new_inode.direct_ptrs = [0; NUM_DIRECT_PTRS];
                new_inode.indirect_ptr = 0;
                new_inode.links_cnt = 1;
                new_inode.reserved = [0; 44];
                write_inode(&*self.device, &mut self.superblock, &new_inode)?; // Write new inode to inode table
                dir_add_entry(
                    &*self.device, 
                    &mut self.superblock, 
                    &mut parent_inode, 
                    &DirEntry::new(new_inode_id, file_name.as_bytes())?
                )?;
                Ok(new_inode_id)
            },
            Err(_) => {
                return Err(Error::NotFound);
            }
        }
    }

    pub fn read(&mut self, inode_id: u32, offset: usize, buf: &mut [u8]) -> Result<usize> {
        let mut inode = get_inode(&*self.device, &self.superblock, inode_id)?;
        if inode.ftype != FileType::Regular {
            return Err(Error::NotRegular);
        }

        let bytes_read = fread(
            &*self.device, 
            &mut self.superblock, 
            &mut inode, 
            offset, 
            buf
        )?;

        Ok(bytes_read)
    }

    pub fn write(&mut self, inode_id: u32, offset: usize, buf: &[u8]) -> Result<usize> {
        let mut inode = get_inode(&*self.device, &self.superblock, inode_id)?;
        if inode.ftype != FileType::Regular {
            return Err(Error::NotRegular);
        }

        let bytes_written = fwrite(
            &*self.device,
            &mut self.superblock,
            &mut inode,
            offset,
            buf
        )?;

        Ok(bytes_written)
    }

    pub fn close(&mut self, inode_id: u32) -> Result<()> {
        // TODO: Sync the inode back to disk if needed
        Ok(())
    }

    pub fn rm(&mut self, path: &str) -> Result<()> {
        let (parent_path, file_name) = split(path);
        let (_, parent_inode_id) = resolve(&*self.device, &mut self.superblock, &parent_path)?;
        let mut parent_inode = get_inode(&*self.device, &mut self.superblock, parent_inode_id)?;
        let (_, inode_id) = resolve(&*self.device, &mut self.superblock, path)?;
        let mut file_inode = get_inode(&*self.device, &mut self.superblock, inode_id)?;
        
        if parent_inode.ftype != FileType::Directory {
            return Err(Error::NotDirectory);
        }

        dir_rm_entry(
            &*self.device,
            &mut self.superblock,
            &mut parent_inode,
            file_name.as_bytes(),
        )?;

        // Free the inode
        file_inode.links_cnt -= 1;
        if file_inode.links_cnt == 0 {
            free_inode(&*self.device, &mut self.superblock, &mut file_inode)?;
        } else {
            write_inode(&*self.device, &mut self.superblock, &file_inode)?;
        }

        Ok(())
    }

    /// Create a hard link to an existing file.
    pub fn link(&mut self, target_path: &str, link_path: &str) -> Result<()> {
        let (_, target_inode_id) = resolve(&*self.device, &mut self.superblock, target_path)?;
        let mut target_inode = get_inode(&*self.device, &mut self.superblock, target_inode_id)?;
        
        if target_inode.ftype != FileType::Regular {
            return Err(Error::NotRegular);
        }

        let (parent_path, link_name) = split(link_path);
        let (_, parent_inode_id) = resolve(&*self.device, &mut self.superblock, &parent_path)?;
        let mut parent_inode = get_inode(&*self.device, &mut self.superblock, parent_inode_id)?;

        if parent_inode.ftype != FileType::Directory {
            return Err(Error::NotDirectory);
        }

        dir_add_entry(
            &*self.device,
            &mut self.superblock,
            &mut parent_inode,
            &DirEntry::new(target_inode_id, link_name.as_bytes())?
        )?;

        target_inode.links_cnt += 1;
        write_inode(&*self.device, &mut self.superblock, &target_inode)?;

        Ok(())
    }
 
    pub fn root_inode_id(&self) -> u32 {
        ROOT_INODE_ID as u32
    }

    pub fn superblock(&self) -> &SuperBlock {
        &self.superblock
    }

    pub fn device(&self) -> Arc<D> {
        Arc::clone(&self.device)
    }
}