pub const MAGIC: u32 = 0x4D554F4E; // "MUON" in ASCII

pub const BLOCK_SIZE: usize = 512;
pub const SUPERBLOCK_ID: u32 = 0; // Block ID for the superblock
pub const ROOT_INODE_ID: u32 = 0; // Inode ID for the root directory
pub const MAX_FSIZE: usize = 1024 * 1024 * 1024; // 1 GiB
pub const MAX_INODES: usize = 1024; // Maximum number of inodes
pub const INODE_SIZE: usize = 128;

pub const MAX_DIR_ENTRIES: usize = 128; // Maximum number of directory entries per directory
pub const MAX_FILE_NAME_LEN: usize = 64 - 4; // DirEntry name length minus inode ID (4 bytes)
pub const DIR_ENTRY_SIZE: usize = 64; // Size of a directory entry (inode ID + name)
pub const NUM_ENTRY_PER_BLOCK: usize = BLOCK_SIZE / DIR_ENTRY_SIZE; // Number of directory entries per block
pub const DOT_NAME: &[u8; 1] = b".";
pub const DOTDOT_NAME: &[u8; 2] = b"..";

pub const NUM_DIRECT_PTRS: usize = 12; // Number of direct pointers in an inode
pub const NUM_INDIRECT_PTRS: usize = 1; // Number of indirect pointers in an inode
pub const PTRS_PER_BLOCK: usize = BLOCK_SIZE / 4; // Number of pointers per block (assuming 32-bit pointers)