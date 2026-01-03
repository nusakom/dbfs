//! DBFS Filesystem Type for RVFS2 Demo
//!
//! Minimal implementation to prove mount works

use alloc::{string::String, string::ToString};
use log::info;
use vfscore::{
    dentry::VfsDentry,
    error::VfsError,
    fstype::{FileSystemFlags, VfsFsType},
    inode::VfsInode,
    VfsResult,
};

use super::{inode::DbfsInode, superblock::DbfsSuperBlock};

/// DBFS filesystem type - minimal demo implementation
pub struct DbfsFsType {
    /// Dummy database path (not used in demo)
    _db_path: String,
}

impl DbfsFsType {
    /// Create a new DBFS filesystem type
    pub fn new(db_path: String) -> Self {
        Self { _db_path: db_path }
    }
}

impl VfsFsType for DbfsFsType {
    fn mount(
        self: alloc::sync::Arc<Self>,
        _flags: u32,
        _ab_mnt: &str,
        _dev: Option<alloc::sync::Arc<dyn VfsInode>>,
        _data: &[u8],
    ) -> VfsResult<alloc::sync::Arc<dyn VfsDentry>> {
        info!("✓ DBFS Demo: Mounting DBFS filesystem");

        // Create a dummy superblock
        let sb = alloc::sync::Arc::new(DbfsSuperBlock::new());

        // Create root inode (ino = 1)
        let root_inode = DbfsInode::new_root(sb.clone());

        // Create root dentry
        let root_dentry = alloc::sync::Arc::new(crate::rvfs2_demo::dentry::DbfsDentry::root(
            root_inode,
        ));

        info!("✓ DBFS Demo: Mount successful, root dentry created");
        Ok(root_dentry)
    }

    fn kill_sb(
        &self,
        _sb: alloc::sync::Arc<dyn vfscore::superblock::VfsSuperBlock>,
    ) -> VfsResult<()> {
        info!("✓ DBFS Demo: Unmounting DBFS");
        Ok(())
    }

    fn fs_flag(&self) -> FileSystemFlags {
        FileSystemFlags::empty()
    }

    fn fs_name(&self) -> String {
        "dbfs".to_string()
    }
}
