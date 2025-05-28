//! Common utilities for tests
#![allow(unused)]

use std::{collections::VecDeque, sync::{Arc, Mutex}};

use muon::*;

pub const ORANGE: &str = "\x1b[38;5;214m";
pub const RESET: &str = "\x1b[0m";

/// Provides a macro for logging messages during tests.
/// e.g. log!("placeholder") -> println!("[test] placeholder");
#[macro_export]
macro_rules! log {
    ($msg:expr, $($arg:tt)*) => {
        println!("{}[test] {}{}", crate::common::ORANGE, format!($msg, $($arg)*), crate::common::RESET)
    };
}

#[derive(Debug)]
pub struct RamDisk {
    inner: Arc<Mutex<Vec<u8>>>,
    num_blocks: usize,
}

impl RamDisk {
    /// Creates a new RamDisk with the specified number of blocks.
    /// Each block is BLOCK_SIZE bytes.
    pub fn new(num_blocks: usize) -> Self {
        let size = num_blocks * BLOCK_SIZE;
        let inner = Arc::new(Mutex::new(vec![0u8; size]));
        RamDisk {
            inner,
            num_blocks,
        }
    }
}


impl BlockDevice for RamDisk {
    fn num_blocks(&self) -> usize {
        self.num_blocks
    }

    fn read_block(&self, block_id: u32, buf: &mut [u8; BLOCK_SIZE]) -> std::result::Result<(), muon::Error> {
        if block_id >= self.num_blocks as u32 {
            return Err(Error::InvalidBlockId);
        }
        let start = block_id as usize * BLOCK_SIZE;
        let end = start as usize + BLOCK_SIZE;
        let data = self.inner.lock().unwrap();
        buf.copy_from_slice(&data[start..end]);
        Ok(())
    }

    fn write_block(&self, block_id: u32, buf: &[u8; BLOCK_SIZE]) -> std::result::Result<(), muon::Error> {
        if block_id >= self.num_blocks as u32 {
            return Err(Error::InvalidBlockId);
        }
        let start = block_id as usize * BLOCK_SIZE;
        let end = start as usize + BLOCK_SIZE;
        let mut data = self.inner.lock().unwrap();
        data[start..end].copy_from_slice(buf);
        Ok(())
    }

    fn flush(&self) -> std::result::Result<(), muon::Error> {
        // In a RAM disk, flushing is a no-op since data is already in memory.
        Ok(())
    }
}

#[derive(Debug)]
pub struct CacheBuffer {
    buf: [u8; BLOCK_SIZE],
    block_id: u32,
    dirty: bool,
}

pub struct LruCache {
    inner: Mutex<LruInner>,
    capacity: usize,
}

struct LruInner {
    cache: VecDeque<CacheBuffer>,
}

impl LruCache {
    pub fn new(capacity: usize) -> Self {
        LruCache {
            inner: Mutex::new(LruInner {
                cache: VecDeque::with_capacity(capacity),
            }),
            capacity,
        }
    }
}

impl Cache for LruCache {
    fn write_cache(&self, block_id: u32, buf: &[u8; BLOCK_SIZE]) -> Result<()> {
        let mut inner = self.inner.lock().unwrap();
        if let Some(idx) = inner.cache.iter().position(|b| b.block_id == block_id) {
            let buffer = &mut inner.cache[idx];
            buffer.buf.copy_from_slice(buf);
            buffer.dirty = true;
            // Move the accessed buffer to the front (most recently used)
            let buffer = inner.cache.remove(idx).unwrap();
            inner.cache.push_front(buffer);
        } else {
            if inner.cache.len() >= self.capacity {
                // Evict and flush immediately, for simplicity
                let evicted_buffer = inner.cache.iter().last().unwrap();
                return Err(muon::Error::CacheEvict(evicted_buffer.block_id));
                // Cache evicted.
            }
            // Add new buffer
            let new_buffer = CacheBuffer {
                buf: *buf,
                block_id,
                dirty: true, // Dirty since the block is being written to cache
            };
            inner.cache.push_front(new_buffer);
        }
        Ok(())
    }

    fn read_cache(&self, block_id: u32, buf: &mut [u8; BLOCK_SIZE]) -> Result<()> {
        let mut inner = self.inner.lock().unwrap();
        if let Some(idx) = inner.cache.iter().position(|b| b.block_id == block_id) {
            let buffer = &inner.cache[idx];
            buf.copy_from_slice(&buffer.buf);
            // Move the accessed buffer to the front (most recently used)
            let buffer = inner.cache.remove(idx).unwrap();
            inner.cache.push_front(buffer);
        } else {
            return Err(muon::Error::CacheMiss);
        }
        
        Ok(())
    }

    fn flush(&self, device: &impl BlockDevice) -> Result<()> {
        let mut inner = self.inner.lock().unwrap();
        for buffer in inner.cache.iter_mut() {
            if buffer.dirty {
                device.write_block(buffer.block_id, &buffer.buf)?;
                buffer.dirty = false;
            }
        }        
        Ok(())
    }

    fn evict(&self, device: &impl BlockDevice, block_id: u32) -> Result<()> {
        let mut inner = self.inner.lock().unwrap();
        if let Some(idx) = inner.cache.iter().position(|b| b.block_id == block_id) {
            let buffer = inner.cache.remove(idx).unwrap();
            if buffer.dirty {
                device.write_block(buffer.block_id, &buffer.buf)?;
            }
        } else {
            return Err(muon::Error::CacheMiss);
        }
        Ok(())
    }
}

