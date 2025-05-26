//! In Muon, cache layer is actually implemented as a block device that wraps 'real' block devices.
//! This design efficiently decouples the cache logic from the underlying block device,
//! allowing for flexible caching strategies.

use crate::BlockDevice;

mod lru;
mod lfu;

pub trait Cache : Send + Sync {

}

pub struct Cached<D: BlockDevice, C: Cache> {
    device: D,
    cache: C,
}

impl<D, C> BlockDevice for Cached<D, C>
where 
    D: BlockDevice,
    C: Cache,
{
    fn block_size(&self) -> usize {
        self.device.block_size()
    }

    fn num_blocks(&self) -> usize {
        self.device.num_blocks()
    }

    fn read_block(&self, block_id: u32, buf: &mut [u8; crate::BLOCK_SIZE]) -> Result<(), crate::Error> {
        todo!()
    }

    fn write_block(&self, block_id: u32, buf: &[u8; crate::BLOCK_SIZE]) -> Result<(), crate::Error> {
        todo!()
    }

    fn flush(&self) -> Result<(), crate::Error> {
        todo!()
    }
}