use alloc::{string::String, string::ToString, sync::Arc, vec::Vec};
use log::info;

use vfscore::{
    dentry::VfsDentry,
    error::VfsError,
    fstype::{FileSystemFlags, VfsFsType},
    inode::VfsInode,
    utils::{VfsNodeType, VfsTimeSpec},
    VfsResult,
};

use super::{dentry::DbfsDentry, inode::DbfsInode, superblock::DbfsSuperBlock};
use crate::{clone_db, common::DbfsTimeSpec, fs_common};

/// DBFS Filesystem Type
pub struct DbfsFsType {
    /// Database path
    db_path: String,
    /// Transaction manager
    pub tm: Arc<crate::transaction::TransactionManager>,
}

impl DbfsFsType {
    /// Create a new DBFS filesystem type
    pub fn new(db_path: String) -> Self {
        Self {
            db_path,
            tm: Arc::new(crate::transaction::TransactionManager::new()),
        }
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
        info!("Mounting DBFS from {}", self.db_path);

        // Set up WAL storage if a device (Bottom FS) is provided
        if let Some(ref dev) = _dev {
            let wal_inode = dev.create(".dbfs.wal", VfsNodeType::File, VfsNodePerm::from_bits_truncate(0o644), None)
                .map_err(|_| VfsError::IoError)?;
            let storage = Arc::new(super::VfsWalStorage::new(wal_inode));
            self.tm.set_wal_storage(storage);
            info!("WAL storage initialized on Bottom FS");
        }

        // Open database
        let db = clone_db();

        // Initialize root inode if needed
        let ctime = DbfsTimeSpec::default();
        fs_common::dbfs_common_root_inode(0, 0, ctime).map_err(|_| VfsError::IoError)?;

        // Get superblock metadata
        let tx = db.tx(false).map_err(|_| VfsError::IoError)?;
        let bucket = tx
            .get_bucket("super_blk".as_bytes())
            .map_err(|_| VfsError::IoError)?;

        let blk_size = bucket
            .get_kv("blk_size")
            .ok_or_else(|| VfsError::IoError)?;
        let blk_size = crate::u32!(blk_size.value());

        let magic = bucket.get_kv("magic").ok_or_else(|| VfsError::IoError)?;
        let magic = crate::u32!(magic.value());

        drop(bucket);
        drop(tx);

        // Create superblock
        let sb = Arc::new(DbfsSuperBlock::new(db, blk_size, magic, 0, self.tm.clone())?) as Arc<dyn vfscore::superblock::VfsSuperBlock>;

        // Get root inode
        let root_inode = sb.root_inode()?;

        // Create root dentry
        let root_dentry = DbfsDentry::root(root_inode);

        Ok(Arc::new(root_dentry))
    }

    fn kill_sb(&self, sb: Arc<dyn vfscore::superblock::VfsSuperBlock>) -> VfsResult<()> {
        info!("Unmounting DBFS");

        // Sync filesystem using the VfsSuperBlock trait
        sb.sync_fs(true)?;

        // Call common umount
        crate::fs_common::dbfs_common_umount().map_err(|_| VfsError::IoError)?;

        Ok(())
    }

    fn fs_flag(&self) -> FileSystemFlags {
        FileSystemFlags::REQUIRES_DEV
    }

    fn fs_name(&self) -> String {
        "dbfs".to_string()
    }
}
