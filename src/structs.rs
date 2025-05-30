use crate::config::*;
use crate::trim_zero;
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

    // pub reserved: [u8; 456],
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
pub struct BlockPtr {
    pub indirect: Option<u32>,
    pub direct: [Option<u32>; NUM_DIRECT_PTRS],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union InodePtr {
    block_ptr: BlockPtr,    // Normal inode with direct and indirect pointers
    path: [u8; MAX_PATH_LEN], // Symlink inode with a path
}

impl core::fmt::Debug for InodePtr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "InodePtr {{ block_ptr: {:?} }}", unsafe { self.block_ptr });
        write!(f, "InodePtr {{ path: {:?} }}", unsafe { String::from_utf8_lossy(trim_zero(&self.path)) })
    }
}

impl InodePtr {
    pub const ZERO: Self = Self {
        block_ptr: BlockPtr { indirect: None, direct: [None; NUM_DIRECT_PTRS] },
    };

    pub fn new() -> Self {
        Self::ZERO
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Inode {
    pub ftype: FileType,
    pub mode: Mode,
    pub id: u32,
    /// Number of data blocks, excluding the block used to contain indirect pointers.
    pub blocks: u32,
    // When links_cnt decreases to 0 and all file descriptors are closed, the inode can be freed.
    pub links_cnt: u32, 
    pub inode_ptr: InodePtr,
    pub size: u64,
}

impl Inode {
    pub const ZERO: Self = Self {
        ftype: FileType::Regular,
        mode: Mode::None,
        id: 0,
        blocks: 0,
        links_cnt: 0,
        inode_ptr: InodePtr { block_ptr: BlockPtr { indirect: None, direct: [None; NUM_DIRECT_PTRS] } },
        size: 0,
    };

    pub fn new(ftype: FileType, mode: Mode, id: u32) -> Self {
        Self {
            ftype,
            mode,
            id,
            blocks: 0,
            links_cnt: 0,
            inode_ptr: InodePtr::new(),
            size: 0,
        }
    }
}

impl Inode {
    pub fn is_directory(&self) -> bool {
        self.ftype == FileType::Directory
    }

    pub fn is_regular_file(&self) -> bool {
        self.ftype == FileType::Regular
    }

    pub fn is_symlink(&self) -> bool {
        self.ftype == FileType::Symlink
    }

    pub fn is_special(&self) -> bool {
        self.ftype == FileType::Special
    }

    pub fn get_block_ptrs(&self) -> Result<&BlockPtr> {
        if self.ftype != FileType::Regular && self.ftype != FileType::Directory {
            return Err(Error::InvalidFileType);
        }
        unsafe {
            Ok(&self.inode_ptr.block_ptr)
        }
    }
    
    pub fn get_block_ptrs_mut(&mut self) -> Result<&mut BlockPtr> {
        if self.ftype != FileType::Regular && self.ftype != FileType::Directory {
            return Err(Error::InvalidFileType);
        }
        unsafe {
            Ok(&mut self.inode_ptr.block_ptr)
        }
    }
    pub fn get_path(&self) -> Result<&[u8; MAX_PATH_LEN]> {
        if self.ftype != FileType::Symlink {
            return Err(Error::NotSymlink);
        }
        unsafe {
            Ok(&self.inode_ptr.path)
        }
    }

    pub fn get_path_mut(&mut self) -> Result<&mut [u8; MAX_PATH_LEN]> {
        if self.ftype != FileType::Symlink {
            return Err(Error::NotSymlink);
        }
        unsafe {
            Ok(&mut self.inode_ptr.path)
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct DirEntry {
    pub inode_id: u32,
    /// Name of the file or directory, padded with zero to fit MAX_FILE_NAME_LEN, if the name is shorter.
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
