//! Muon is a tiny file system primarily designed for CafOS.
//! For simplicity, no support for permissions, timestamps, or other advanced features.
//! 
//! Muon File System's linear layout:
//! - Superblock
//! - Block Bitmap
//! - Inode Bitmap
//! - Inode Table
//! - Data Blocks
//! 
//! Muon's 7 layers (from bottom to top):
//! 1. Block Device: Abstraction for block storage devices.     Storage device synchronization
//! 2. Cache: Optional caching layer for performance.           Storage device synchronization
//! 3. Bitmap: Manages allocation of data blocks and inodes.    Superblock/bitmap synchronization
//! 4. Inode: Represents files and directories                  Inode synchronization
//! 5. Directory: Manages directory entries and structure.      Inode synchronization
//! 6. File: Represents file operations and data access.        Inode synchronization
//! 7. MuonFS: The main file system interface for users.

#![allow(unused)]
//#![no_std]

// Users of this crate must enable the `alloc` feature for heap allocations.
extern crate alloc;

mod config;
mod block_dev;
mod cache;
mod structs;
mod bitmap;
mod superblock;
mod inode;
mod directory;
mod path;
mod file;
mod fs;
mod error;

pub use block_dev::BlockDevice;
pub use config::*;
pub use superblock::*;
pub use structs::*;
pub use inode::*;
pub use path::*;
pub use directory::*;
pub use file::*;
pub use fs::*;
pub use error::FsError as Error;
pub use error::Result;