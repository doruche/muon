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
//! Muon's 6 layers (from bottom to top):
//! 1. Block Device: Abstraction for low level devices.         Storage device synchronization  | User implemented (hardware-specific)
//! 2. Cache: Optional caching layer for performance.           Storage device synchronization  | User implemented (sync, strategy, etc.)
//! 3. Inode: Represents file metadata and operations.          Inode synchronization           | Fs implemented
//! 4. Directory/Path: Manages directory entries and structure. Inode synchronization           | Fs implemented
//! 5. File: Represents file operations and data access.        Inode synchronization           | Fs implemented
//! 6. MuonFS: The main file system interface for users.                                        | User implemented (ofile, process, etc.)

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