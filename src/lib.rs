//! Muon is a tiny file system primarily designed for CafOS.
//! For simplicity, no support for permissions, timestamps, or other advanced features.
//! Muon File System's layout:
//! - Superblock
//! - Block Bitmap
//! - Inode Bitmap
//! - Inode Table
//! - Data Blocks
//! NOTE Currently, Muon does not support cacheing or journaling, which may be extended in the future.

#![allow(unused)]
#![no_std]

// Users of this crate must enable the `alloc` feature for heap allocations.
extern crate alloc;

mod config;
mod block_dev;
mod structs;
mod bitmap;
mod superblock;
mod fs;
mod error;

pub use block_dev::BlockDevice;
pub use config::*;
pub use structs::*;
pub use error::FsError as Error;
pub use error::Result;