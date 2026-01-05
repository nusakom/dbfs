//! DBFS adapter for new RVFS API
//!
//! This module provides an implementation of the new RVFS traits (VfsInode, VfsFile, VfsSuperBlock, VfsFsType)
//! for DBFS, allowing it to work with the updated VFS layer.

mod common;
mod dentry;
mod fstype;
mod inode;
mod superblock;

use alloc::sync::Arc;
use alloc::string::String;

pub use fstype::DbfsFsType;

pub struct VfsWalStorage {
    inode: Arc<dyn vfscore::inode::VfsInode>,
}

impl VfsWalStorage {
    pub fn new(inode: Arc<dyn vfscore::inode::VfsInode>) -> Self {
        Self { inode }
    }
}

impl crate::wal::WalStorage for VfsWalStorage {
    fn write(&self, offset: u64, data: &[u8]) -> Result<(), String> {
        // In vfscore, VfsInode often also implements VfsFile
        // or has a way to get one. For now, we assume the inode
        // representing the WAL file can be written to.
        self.inode.write_at(offset, data)
            .map(|_| ())
            .map_err(|e| alloc::format!("{:?}", e))
    }

    fn read(&self, offset: u64, buf: &mut [u8]) -> Result<(), String> {
        self.inode.read_at(offset, buf)
            .map(|_| ())
            .map_err(|e| alloc::format!("{:?}", e))
    }

    fn truncate(&self, length: u64) -> Result<(), String> {
        self.inode.truncate(length).map_err(|e| alloc::format!("{:?}", e))
    }

    fn flush(&self) -> Result<(), String> {
        self.inode.flush().map_err(|e| alloc::format!("{:?}", e))
    }
}
