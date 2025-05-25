use crate::config::*;
use crate::BlockDevice;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SuperBlock {
    pub magic: u32,          // Magic number to identify the filesystem
    pub num_blocks: u32,    // Total number of blocks in the filesystem
    pub block_size: u32,    // Fixed to BLOCK_SIZE
    pub free_blocks: u32,   // Number of free blocks
    pub root_inode: u32,    // Inode number of the root directory

    pub block_bitmap_start: u32, // Block number where the block bitmap starts
    pub block_bitmap_size: u32, // Size of the block bitmap in blocks
    pub inode_bitmap_start: u32, // Block number where the inode bitmap starts
    pub inode_bitmap_size: u32, // Size of the inode bitmap in blocks
    pub inode_table_start: u32, // Block number where the inode table starts
    pub inode_table_size: u32, // Size of the inode table in blocks
    pub data_start: u32, // Block number where data blocks start

    pub reserved: [u8; 512 - 12 * 4], // Fill to 512 bytes.
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
    pub blocks: u32,
    pub links_cnt: u32,
    pub indirect_ptr: u32,
    pub direct_ptrs: [u32; NUM_DIRECT_PTRS],
    pub size: u64,
    pub reserved: [u8; 512 - 4 * 4 - NUM_DIRECT_PTRS as usize * 4 - 8], // Fill to 512 bytes.
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct DirEntry {
    pub inode_id: u32,
    pub name: [u8; MAX_FILE_NAME_LEN],
}

