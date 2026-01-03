//! DBFS FsType for Alien Integration
//!
//! Phase 1: 基本挂载功能

use alloc::{string::String, string::ToString, sync::Arc};
use log::info;
use vfscore::{
    dentry::VfsDentry,
    error::VfsError,
    fstype::{FileSystemFlags, VfsFsType},
    inode::VfsInode,
    VfsResult,
};

use super::{dentry::DbfsDentry, superblock::DbfsSuperBlock};

/// DBFS Filesystem Type
///
/// Phase 1: 可以在 Alien OS 中注册和挂载
pub struct DbfsFsType {
    /// Database path (Phase 1: 暂不使用，为未来预留)
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
        self: Arc<Self>,
        _flags: u32,
        _ab_mnt: &str,
        _dev: Option<Arc<dyn VfsInode>>,
        _data: &[u8],
    ) -> VfsResult<Arc<dyn VfsDentry>> {
        info!("✓ DBFS: Mounting DBFS filesystem");

        // Create superblock
        let sb = Arc::new(DbfsSuperBlock::new(self._db_path.clone()));

        // Create root inode
        let root_inode = sb.root_inode()?;

        // Create root dentry
        let root_dentry = Arc::new(DbfsDentry::root(root_inode));

        info!("✓ DBFS: Mount successful");
        Ok(root_dentry)
    }

    fn kill_sb(
        &self,
        _sb: Arc<dyn vfscore::superblock::VfsSuperBlock>,
    ) -> VfsResult<()> {
        info!("✓ DBFS: Unmounting DBFS");
        Ok(())
    }

    fn fs_flag(&self) -> FileSystemFlags {
        FileSystemFlags::empty()
    }

    fn fs_name(&self) -> String {
        "dbfs".to_string()
    }
}

/// Dummy FsType for superblock.fs_type() to return
///
/// This is only used to satisfy the VfsSuperBlock trait requirement.
/// The real functionality is in DbfsFsType.
pub struct DummyFsType {
    pub name: String,
}

impl VfsFsType for DummyFsType {
    fn mount(
        self: Arc<Self>,
        _flags: u32,
        _ab_mnt: &str,
        _dev: Option<Arc<dyn VfsInode>>,
        _data: &[u8],
    ) -> VfsResult<Arc<dyn VfsDentry>> {
        Err(VfsError::NoSys)
    }

    fn kill_sb(
        &self,
        _sb: Arc<dyn vfscore::superblock::VfsSuperBlock>,
    ) -> VfsResult<()> {
        Ok(())
    }

    fn fs_flag(&self) -> FileSystemFlags {
        FileSystemFlags::empty()
    }

    fn fs_name(&self) -> String {
        self.name.clone()
    }
}
