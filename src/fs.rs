use alloc::{string::ToString, sync::Arc, vec::Vec};
use crate::{alloc_inode, bmap, dir_is_empty, directory::{dir_add_entry, dir_rm_entry}, file::{fread, fwrite}, free_inode, get_inode, mkdir, path::{self, resolve, split}, read_dir, read_superblock, resolve_without_last, structs::*, superblock, write_inode, write_superblock, BlockDevice, Error, Result, DOTDOT_NAME, DOT_NAME, ROOT_INODE_ID};
use crate::structs::*;
use crate::config::*;

#[derive(Debug)]
pub struct FileSystem<D: BlockDevice> {
    device: Arc<D>,
    /// In-memory copy of the superblock.
    superblock: SuperBlock,
}

impl<D: BlockDevice> FileSystem<D> {

    /// Formats the filesystem on the given block device.
    /// Initializes the superblock and zeroes out the metadata blocks.
    /// Returns a new `FileSystem` instance.
    pub fn format(device: Arc<D>, num_blocks: u32, num_inodes: u32) -> Result<Self> {
        let mut superblock = SuperBlock::new(num_blocks, num_inodes)?;

        let zero_block = Box::new([0u8; BLOCK_SIZE]);
        
        // Zero out metadata blocks
        for i in 0..superblock.data_bitmap_blocks {
            device.write_block(superblock.data_bitmap_start + i, &zero_block)?;
        }
        for i in 0..superblock.inode_bitmap_blocks {
            device.write_block(superblock.inode_bitmap_start + i, &zero_block)?;
        }
        for i in 0..superblock.inode_table_blocks {
            device.write_block(superblock.inode_table_start + i, &zero_block)?;
        }
        // No need to zero out data blocks, as they will be zeroed on allcations.

        // Initialize root inode
        let _ = alloc_inode(device.as_ref(),&mut superblock, FileType::Special, Mode::None)?;

        let mut root_inode = alloc_inode(
            device.as_ref(), 
            &mut superblock, 
            FileType::Directory, 
            Mode::RW
        )?;
        assert!(root_inode.id == ROOT_INODE_ID as u32, "Root inode ID mismatch");
        
        // On formating, root inode has no parent (or itself), so we have to set '.' and '..' entries manually.        
        dir_add_entry(
            &*device, 
            &mut superblock, 
            &mut root_inode, 
            &DirEntry::new(ROOT_INODE_ID, DOT_NAME)?
        )?;
        dir_add_entry(
            &*device, 
            &mut superblock, 
            &mut root_inode, 
            &DirEntry::new(ROOT_INODE_ID, DOTDOT_NAME)?
        )?;
        root_inode.links_cnt = 2; // '.' and '..' entries
        assert!(root_inode.size == 2 * DIR_ENTRY_SIZE as u64, "Root inode size mismatch");
        assert!(root_inode.blocks == 1, "Root inode blocks count mismatch");
        assert!(root_inode.size == DIR_ENTRY_SIZE as u64 * 2, "Root inode size mismatch");
        write_inode(&*device, &mut superblock, &root_inode)?; // Write root inode to inode table

        write_superblock(&*device, &superblock)?;

        let mut fs_inst = Self {
            device: Arc::clone(&device),
            superblock,
        };

        Ok(fs_inst)
    }

    /// Mounts the filesystem from the given block device.
    /// Reads the superblock and initializes the filesystem instance.
    pub fn mount(device: Arc<D>) -> Result<Self> {
        let superblock = read_superblock(&*device)?;
        Ok(Self {
            device,
            superblock,
        })
    }

    pub fn flush(&mut self) -> Result<()> {
        self.device.flush()?;
        Ok(())
    }

    pub fn get_inode(&self, inode_id: u32) -> Result<Inode> {
        get_inode(self.device.as_ref(), &self.superblock, inode_id)
    }

    /// Unmounts the filesystem, writing the superblock back to the device.
    /// This should be called before the device is closed to ensure all metadata is saved.
    pub fn unmount(&mut self) -> Result<()> {
        write_superblock(self.device.as_ref(), &self.superblock)?;
        self.device.flush()?;
        Ok(())
    }
    
    /// Query the inode ID for the given path.
    /// Returns the inode ID and its file type.
    pub fn lookup(&mut self, path: &str) -> Result<(u32, FileType)> {
        let (_, inode_id) = resolve(&*self.device, &mut self.superblock, path)?;
        let inode = get_inode(&*self.device, &self.superblock, inode_id)?;
        Ok((inode_id, inode.ftype))
    }

    pub fn creat(
        &mut self,
        path: &str,
        file_type: FileType,
        mode: Mode,
    ) -> Result<u32> {
        let (parent_path, file_name) = split(path)?;
        let (_, parent_inode_id) = resolve(self.device.as_ref(), &mut self.superblock, &parent_path)?;
        let mut parent_inode = get_inode(&*self.device, &mut self.superblock, parent_inode_id)?;
        if parent_inode.ftype != FileType::Directory {
            return Err(Error::NotDirectory);
        }
        println!("parent inode: {:?}", parent_inode);
        match file_type {
            FileType::Regular => {
                let mut new_inode = alloc_inode(
                    self.device.as_ref(), 
                    &mut self.superblock,
                    FileType::Regular, 
                    mode
                )?;
                dir_add_entry(
                    self.device.as_ref(),
                    &mut self.superblock,
                    &mut parent_inode,
                    &DirEntry::new(new_inode.id, file_name.as_bytes())?
                )?;
                new_inode.links_cnt = 1;
                write_inode(self.device.as_ref(), &self.superblock, &parent_inode)?;
                write_inode(self.device.as_ref(), &mut self.superblock, &new_inode)?;
                Ok(new_inode.id)
            },
            FileType::Directory => {
                let dir_inode_id = mkdir(
                    self.device.as_ref(), 
                    &mut self.superblock, 
                    &mut parent_inode, 
                    file_name.as_bytes()
                )?;
                Ok(dir_inode_id)
            },
            _ => return Err(Error::InvalidArgument),
        }
    }

    pub fn remove(&mut self, path: &str, ftype: FileType) -> Result<()> {
        let (parent_path, file_name) = split(path)?;
        let (_, parent_inode_id) =  resolve(&*self.device, &mut self.superblock, &parent_path)?;
        let mut parent_inode = get_inode(&*self.device, &mut self.superblock, parent_inode_id)?;
        if parent_inode.ftype != FileType::Directory {
            return Err(Error::NotDirectory);
        }
        println!("[remove] parent inode: {:?}", parent_inode);
        let (_, inode_id) = if ftype != FileType::Symlink {
            resolve(&*self.device, &mut self.superblock, path)?
        } else {
            resolve_without_last(&*self.device, &mut self.superblock, path)?
        };
        println!("[remove] inode_id: {}", inode_id);
        let mut file_inode = get_inode(&*self.device, &mut self.superblock, inode_id)?;
        
        if matches!(ftype, FileType::Special) {
            return Err(Error::InvalidArgument);
        }
        if file_inode.ftype != ftype {
            return Err(Error::InvalidArgument);
        }

        if ftype == FileType::Directory && !dir_is_empty(self.device.as_ref(), &mut self.superblock, &file_inode)? {
            return Err(Error::DirNotEmpty);
        }

        dir_rm_entry(
            &*self.device,
            &mut self.superblock,
            &mut parent_inode,
            file_name.as_bytes(),
        )?;

        // Free the inode if hard links count reaches 0.
        file_inode.links_cnt -= 1;
        if ftype == FileType::Directory {
            // .
            file_inode.links_cnt -= 1;
            // ..
            parent_inode.links_cnt -= 1;
            write_inode(self.device.as_ref(), &mut self.superblock, &parent_inode)?;
        }

        if file_inode.links_cnt == 0 {
            println!("[remove] Freeing inode: {}", file_inode.id);
            free_inode(self.device.as_ref(), &mut self.superblock, file_inode.id)?;
        } else {
            write_inode(self.device.as_ref(), &mut self.superblock, &file_inode)?;
        }

        Ok(())
    }

    pub fn read_dir(&mut self, path: &str) -> Result<Vec<DirEntry>> {
        let (_, inode_id) = resolve(&*self.device, &mut self.superblock, path)?;
        let mut inode = get_inode(&*self.device, &self.superblock, inode_id)?;
        if inode.ftype != FileType::Directory {
            return Err(Error::NotDirectory);
        }
        let entries = read_dir(
            self.device.as_ref(), 
            &mut self.superblock, 
            &mut inode
        )?;

        Ok(entries)
    }

    pub fn fread(
        &mut self,
        path: &str,
        offset: usize,
        buf: &mut [u8],
    ) -> Result<usize> {
        let (_, inode_id) = resolve(&*self.device, &mut self.superblock, path)?;
        let mut inode = get_inode(&*self.device, &self.superblock, inode_id)?; 
        if inode.ftype != FileType::Regular {
            return Err(Error::NotRegular);
        }

        if !matches!(inode.mode, Mode::Read|Mode::RW|Mode::RWE) {
            return Err(Error::PermissionDenied);
        }
        let bytes_read = fread(
            self.device.as_ref(),
            &mut self.superblock,
            &mut inode,
            offset,
            buf,
        )?;
        Ok(bytes_read)   
    }

    pub fn fwrite(
        &mut self,
        path: &str,
        offset: usize,
        buf: &[u8],
    ) -> Result<usize> {
        let (_, inode_id) = resolve(&*self.device, &mut self.superblock, path)?;
        let mut inode = get_inode(&*self.device, &self.superblock, inode_id)?;
        if inode.ftype != FileType::Regular {
            return Err(Error::NotRegular);
        }
        if !matches!(inode.mode, Mode::Write|Mode::RW|Mode::RWE) {
            return Err(Error::PermissionDenied);
        }
        let bytes_written = fwrite(
            self.device.as_ref(),
            &mut self.superblock,
            &mut inode,
            offset,
            buf,
        )?;
        Ok(bytes_written)
    }

    /// Creates a hard link to the target file with the given link name.
    /// The link target must be a regular file.
    /// The link name must not already exist in the target's parent directory.
    /// The link name must be an absolute path.
    /// Returns the inode ID of the linked file.
    pub fn link(
        &mut self,
        target: &str,
        link_name: &str,
    ) -> Result<u32> {
        let (parent_path, link_name) = path::split(link_name)?;
        let (_, parent_inode_id) = resolve(&*self.device, &mut self.superblock, &parent_path)?;
        let mut parent_inode = get_inode(&*self.device, &mut self.superblock, parent_inode_id)?;
        if parent_inode.ftype != FileType::Directory {
            return Err(Error::NotDirectory);
        }
        let (_, target_inode_id) = resolve(&*self.device, &mut self.superblock, target)?;
        let mut target_inode = get_inode(&*self.device, &mut self.superblock, target_inode_id)?;
        if target_inode.ftype != FileType::Regular {
            return Err(Error::NotRegular);
        }
        dir_add_entry(
            self.device.as_ref(),
            &mut self.superblock,
            &mut parent_inode,
            &DirEntry::new(target_inode_id, link_name.as_bytes())?,
        )?;
        target_inode.links_cnt += 1;
        write_inode(self.device.as_ref(), &mut self.superblock, &target_inode)?;

        Ok(target_inode_id)
    }

    /// Creates a symbolic link to the target file with the given link name.
    /// Generates only absolute paths.
    /// Returns the inode ID of the symlink.
    pub fn symlink(
        &mut self,
        target: &str,
        link_name: &str,
    ) -> Result<u32> {
        if target.as_bytes().len() > MAX_PATH_LEN {
            return Err(Error::PathTooLong);
        }

        let (parent_path, link_name) = path::split(link_name)?;
        let (_, parent_inode_id) = resolve(&*self.device, &mut self.superblock, &parent_path)?;
        let mut parent_inode = get_inode(&*self.device, &mut self.superblock, parent_inode_id)?;
        if parent_inode.ftype != FileType::Directory {
            return Err(Error::NotDirectory);
        }
        let mut new_inode = alloc_inode(
            self.device.as_ref(),
            &mut self.superblock,
            FileType::Symlink,
            Mode::Read,
        )?;
        let path_buf = new_inode.get_path_mut()?;
        path_buf[..target.as_bytes().len()].copy_from_slice(target.as_bytes());
        dir_add_entry(
            self.device.as_ref(),
            &mut self.superblock,
            &mut parent_inode,
            &DirEntry::new(new_inode.id, link_name.as_bytes())?,
        )?;
        new_inode.links_cnt = 1; // symlink itself
        write_inode(self.device.as_ref(), &mut self.superblock, &new_inode)?;
        
        Ok(new_inode.id)
    }

    /// Reads the target of a symbolic link.
    /// Returns a byte array containing the target path.
    pub fn read_link(
        &mut self,
        link_name: &str,
        buf: &mut [u8; MAX_PATH_LEN],
    ) -> Result<()> {
        let (_, inode_id) = resolve_without_last(self.device.as_ref(), &mut self.superblock, link_name)?;
        let inode = get_inode(self.device.as_ref(), &self.superblock, inode_id)?;
        let path_buf = inode.get_path()?;
        buf.copy_from_slice(path_buf);
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

    pub fn dump(&self) -> String {
        format!("{:?}", self.superblock)
    }
}