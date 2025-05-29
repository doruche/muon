//! In Muon, cache layer is actually implemented as a block device that wraps 'real' block devices.
//! This design efficiently decouples the cache logic from the underlying block device,
//! allowing for flexible caching strategies.

use crate::{BlockDevice, Error, Result, BLOCK_SIZE};

pub trait Cache: Send + Sync {
    fn write_cache(&self, block_id: u32, buf: &[u8; BLOCK_SIZE]) -> Result<()>;
    
    fn read_cache(&self, block_id: u32, buf: &mut [u8; BLOCK_SIZE]) -> Result<()>;
    
    fn flush(&self, device: &impl BlockDevice) -> Result<()>;
    
    fn evict(&self, device: &impl BlockDevice, block_id: u32) -> Result<()>;
}

pub struct Cached<D: BlockDevice, C: Cache> {
    device: D,
    cache: C,
}

impl<D, C> Cached<D, C>
where 
    D: BlockDevice,
    C: Cache,
{
    pub fn new(device: D, cache: C) -> Self {
        Cached { device, cache }
    }
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

    fn read_block(&self, block_id: u32, buf: &mut [u8; crate::BLOCK_SIZE]) -> Result<()> {
        match self.cache.read_cache(block_id, buf) {
            Ok(_) => Ok(()),
            Err(Error::CacheMiss) => {
                self.device.read_block(block_id, buf)?;
                match self.cache.write_cache(block_id, buf) {
                    Ok(_) => {
                        ;
                    },
                    Err(Error::CacheEvict(evicted_block_id)) => {
                        self.cache.evict(&self.device, evicted_block_id)?;
                        self.cache.write_cache(block_id, buf)?;
                    },
                    Err(e) => return Err(e),
                }
                Ok(())
            },
            Err(e) => Err(e),
        }
    }

    fn write_block(&self, block_id: u32, buf: &[u8; crate::BLOCK_SIZE]) -> Result<()> {
        match self.cache.write_cache(block_id, buf) {
            Ok(()) => {
                // Writing succeeded.
                ;   
            }
            Err(Error::CacheEvict(evicted_block_id)) => {
                // Cache full, need to flush the evicted block first.
                self.cache.evict(&self.device, evicted_block_id)?;
                // Then write to the cache
                self.cache.write_cache(block_id, buf)?;
            },
            Err(e) => return Err(e),
        }
        Ok(())
    }

    fn flush(&self) -> Result<()> {
        self.cache.flush(&self.device)?;
        Ok(())
    }
}