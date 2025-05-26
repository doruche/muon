use crate::config::*;
use crate::BlockDevice;
use crate::Error;
use crate::Result;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SuperBlock {
    pub magic: u32,          // Magic number to identify the filesystem
    pub num_blocks: u32,    // Total number of blocks in the filesystem
    pub block_size: u32,    // Fixed to BLOCK_SIZE
    pub free_blocks: u32,   // Number of free blocks
    pub num_inodes: u32,    // Total number of inodes in the filesystem
    pub free_inodes: u32,   // Number of free inodes
    pub root_inode: u32,    // Inode number of the root directory

    pub data_bitmap_start: u32, // Block number where the data bitmap starts
    pub data_bitmap_blocks: u32, // Size of the data bitmap in blocks
    pub inode_bitmap_start: u32, // Block number where the inode bitmap starts
    pub inode_bitmap_blocks: u32, // Size of the inode bitmap in blocks
    pub inode_table_start: u32, // Block number where the inode table starts
    pub inode_table_blocks: u32, // Size of the inode table in blocks
    pub data_start: u32, // Block number where data blocks start

    pub reserved: [u8; 456],
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FileType {
    Regular = 1,
    Directory = 2,
    Symlink = 3,
    Special = 4, // For devices or other special files
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Read = 0b001,
    Write = 0b010,
    Execute = 0b100,
    RW = Self::Read as u8 | Self::Write as u8,
    RE = Self::Read as u8 | Self::Execute as u8,
    RWE = Self::Read as u8 | Self::Write as u8 | Self::Execute as u8,
    None = 0b000, // No permissions
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Inode {
    pub mode: Mode,
    pub ftype: FileType,
    pub id: u32,
    pub blocks: u32,
    pub links_cnt: u32,
    pub indirect_ptr: u32,
    pub direct_ptrs: [u32; NUM_DIRECT_PTRS],
    pub size: u64,
    pub reserved: [u8; 44],
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct DirEntry {
    pub inode_id: u32,
    pub name: [u8; MAX_FILE_NAME_LEN],
}

impl DirEntry {
    pub const NULL: Self = Self {
        inode_id: 0,
        name: [0; MAX_FILE_NAME_LEN],
    };

    pub fn new(inode_id: u32, name: &[u8]) -> Result<Self> {
        if name.len() == 0 || name.len() > MAX_FILE_NAME_LEN {
            return Err(Error::InvalidFileName);
        }
        Ok(Self {
            inode_id,
            name: {
                let mut arr = [0; MAX_FILE_NAME_LEN];
                arr[..name.len()].copy_from_slice(name);
                arr
            },
        })
    }
}
