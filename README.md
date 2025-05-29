## Muon
__(Under development)__

Muon is a mini file system implemented in Rust, inspired by _vsfs_ (_very simple file system_, see _OSTEP_) , and primarily written for CafOS (author's operating system project). It is architected in a quite direct way easy to understand and extend. 
For simplicity, Muon is supposed to be used with a single file system per disk, and does not support multiple partitions or complex file system features (e.g. journaling).
### Architecture
Muon is organized in a 5-layer hierarchy, with each layer providing a specific functionality, shown below:
- __Block Device__  (`block_dev.rs`):
  - The lowest layer, responsible for reading and writing raw blocks from/to the disk, providing a simple interface for block operations.
  - Implemented by the user, as it is highly dependent on the underlying hardware.
- __Cache__ (`cache.rs`):
  - Muon deploys a flexible cache system. A `Cache` trait is defined, thus allowing different cache implementations. 
  - A cached block device is treated as same as a plain block device, as the `Cached<Cache, BlockDevice>` type implements the `BlockDevice` trait, by default.
  - Implemented by the user, as it is highly dependent on the caching strategy and requirements, as well as synchronization needs.
- __Inode__ (`superblock.rs`, `bitmap.rs`, `inode.rs`)
  - Inodes are data structures that store information about files and directories, such as their size, ownership, and permissions.
  - Each file or directory is represented by an inode, which is identified by a unique inode number.
- __Directory__ (`directory.rs`, `path.rs`):
    - Directories are special files that contain a list of `DirEntry`s, which are simply containers of name and inode number, allowing for hierarchical organization of files and directories.
    - Provides methods like `dir_add_entry`, `dir_rm_entry`, and `mkdir` to manage directory entries.
    - Path/Name resolution handled here.
- __File__ (`file.rs`, `fs.rs`):
  - Methods for reading and writing files, as well as file metadata management.
  - A `FileSystem` struct is defined, which provides a high-level interface for file operations.
### Storage Layout
Muon uses simple linear storage layout, with the following structure:
- __Superblock__    Metadata of the file system managed here.
- __Block Bitmap__   Bitmap for managing free blocks in the file system.
- __Inode Bitmap__   Bitmap for managing free inodes in the file system.
- __Inode Table__   Table of inodes, each inode is a fixed-size structure.
- __Data Blocks__    Actual data blocks, where file contents are stored.
### Usage
Muon is a `#[no_std]` library, and can be deployed in any Rust project. To use Muon, you need to implement the `BlockDevice` trait for your specific hardware, and optionally implement a caching strategy by implementing the `Cache` trait. Then create a `FileSystem` instance and use its methods to perform file operations.