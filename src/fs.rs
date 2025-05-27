use alloc::{string::ToString, sync::Arc, vec::Vec};
use crate::{alloc_inode, bmap, directory::{dir_add_entry, dir_rm_entry}, file::{fread, fwrite}, free_inode, get_inode, mkdir, path::{self, resolve, split}, read_dir, read_superblock, structs::*, superblock, write_inode, write_superblock, BlockDevice, Error, Result, DOTDOT_NAME, DOT_NAME, ROOT_INODE_ID};
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
        assert!(root_inode.direct_ptrs[0].unwrap() == superblock.data_start, "Root inode data pointer mismatch");
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
        let (parent_path, file_name) = split(path);
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
        let (parent_path, file_name) = split(path);
        let (_, parent_inode_id) = resolve(&*self.device, &mut self.superblock, &parent_path)?;
        let mut parent_inode = get_inode(&*self.device, &mut self.superblock, parent_inode_id)?;
        if parent_inode.ftype != FileType::Directory {
            return Err(Error::NotDirectory);
        }
        println!("[remove] parent inode: {:?}", parent_inode);
        let (_, inode_id) = resolve(&*self.device, &mut self.superblock, path)?;
        println!("[remove] inode_id: {}", inode_id);
        let mut file_inode = get_inode(&*self.device, &mut self.superblock, inode_id)?;
        
        if !matches!(ftype, FileType::Regular | FileType::Directory) {
            return Err(Error::InvalidArgument);
        }

        dir_rm_entry(
            &*self.device,
            &mut self.superblock,
            &mut parent_inode,
            file_name.as_bytes(),
        )?;

        // Free the inode if it hard links count reaches 0.
        file_inode.links_cnt -= 1;
        if (file_inode.links_cnt == 0 && ftype == FileType::Regular) ||
           (file_inode.links_cnt == 1 && ftype == FileType::Directory) {
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
        format!("{:#?}", self.superblock)
    }
}