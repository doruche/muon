use alloc::{sync::Arc, vec::Vec};
use crate::{structs::*, BlockDevice};

pub struct FileSystem {
    block_device: Arc<dyn BlockDevice>,
    superblock: SuperBlock,
    inodes: Vec<Inode>,
}