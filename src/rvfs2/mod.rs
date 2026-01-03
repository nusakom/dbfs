//! DBFS adapter for new RVFS API
//!
//! This module provides an implementation of the new RVFS traits (VfsInode, VfsFile, VfsSuperBlock, VfsFsType)
//! for DBFS, allowing it to work with the updated VFS layer.

mod common;
mod dentry;
mod fstype;
mod inode;
mod superblock;

pub use fstype::DbfsFsType;
