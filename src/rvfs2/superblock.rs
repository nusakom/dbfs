use alloc::{collections::BTreeMap, sync::Arc, vec::Vec};
use spin::Mutex;

use vfscore::{
    superblock::{SuperType, VfsSuperBlock},
    utils::VfsFsStat,
    VfsResult,
};

use crate::{clone_db, common::DbfsTimeSpec, fs_common, inode_common::DBFS_INODE_NUMBER};

/// DBFS SuperBlock structure
pub struct DbfsSuperBlock {
    /// Database instance
    db: Arc<crate::SafeDb>,
    /// Block size
    block_size: u64,
    /// Magic number
    magic: u32,
    /// Root inode number
    root_ino: usize,
    /// Mount flags
    mount_flags: u32,
    /// Inode cache (inode_number -> Arc<DbfsInode>)
    inode_cache: Mutex<BTreeMap<usize, Arc<super::inode::DbfsInode>>>,
}

impl DbfsSuperBlock {
    /// Create a new DBFS superblock
    pub fn new(
        db: Arc<crate::SafeDb>,
        block_size: u32,
        magic: u32,
        mount_flags: u32,
    ) -> VfsResult<Self> {
        let db_clone = db.clone();
        let tx = db_clone.tx(false).map_err(|_| vfscore::error::VfsError::IoError)?;

        // Load or create superblock metadata
        let bucket = tx
            .get_bucket("super_blk".as_bytes())
            .map_err(|_| vfscore::error::VfsError::IoError)?;

        let continue_number = bucket
            .get_kv("continue_number")
            .ok_or_else(|| vfscore::error::VfsError::IoError)?;
        let continue_number = crate::usize!(continue_number.value());

        // Set the next inode number
        DBFS_INODE_NUMBER.store(continue_number, core::sync::atomic::Ordering::SeqCst);

        // Get block size from superblock
        let blk_size = bucket
            .get_kv("blk_size")
            .ok_or_else(|| vfscore::error::VfsError::IoError)?;
        let blk_size = crate::u32!(blk_size.value());

        Ok(Self {
            db,
            block_size: blk_size as u64,
            magic,
            root_ino: 1, // Root inode is always 1
            mount_flags,
            inode_cache: Mutex::new(BTreeMap::new()),
        })
    }

    /// Get the database instance
    pub fn db(&self) -> Arc<crate::SafeDb> {
        self.db.clone()
    }

    /// Get the block size
    pub fn block_size(&self) -> u64 {
        self.block_size
    }

    /// Get the magic number
    pub fn magic(&self) -> u32 {
        self.magic
    }

    /// Get the root inode number
    pub fn root_ino(&self) -> usize {
        self.root_ino
    }

    /// Insert an inode into the cache
    pub fn insert_inode(&self, ino: usize, inode: Arc<super::inode::DbfsInode>) {
        let mut cache = self.inode_cache.lock();
        cache.insert(ino, inode);
    }

    /// Get an inode from the cache
    pub fn get_inode(&self, ino: usize) -> Option<Arc<super::inode::DbfsInode>> {
        let cache = self.inode_cache.lock();
        cache.get(&ino).cloned()
    }

    /// Remove an inode from the cache
    pub fn remove_inode(&self, ino: usize) {
        let mut cache = self.inode_cache.lock();
        cache.remove(&ino);
    }
}

impl VfsSuperBlock for DbfsSuperBlock {
    fn sync_fs(&self, _wait: bool) -> VfsResult<()> {
        let db = self.db();
        let tx = db.tx(true).map_err(|_| vfscore::error::VfsError::IoError)?;
        let bucket = tx
            .get_bucket("super_blk".as_bytes())
            .map_err(|_| vfscore::error::VfsError::IoError)?;

        let continue_number =
            DBFS_INODE_NUMBER.load(core::sync::atomic::Ordering::SeqCst);
        bucket
            .put("continue_number".as_bytes(), continue_number.to_be_bytes())
            .map_err(|_| vfscore::error::VfsError::IoError)?;

        tx.commit().map_err(|_| vfscore::error::VfsError::IoError)?;
        Ok(())
    }

    fn stat_fs(&self) -> VfsResult<VfsFsStat> {
        let _stat = fs_common::dbfs_common_statfs(
            self.block_size as u64,
            self.magic as u64,
            self.mount_flags as u64,
            0,
            self.block_size,
        )
        .map_err(|_| vfscore::error::VfsError::IoError)?;

        // 手动构建 VfsFsStat（使用默认值）
        Ok(vfscore::utils::VfsFsStat {
            f_type: self.magic as i64,
            f_bsize: self.block_size as i64,
            f_blocks: 0,
            f_bfree: 0,
            f_bavail: 0,
            f_files: 0,
            f_ffree: 0,
            f_fsid: [0; 2],
            f_namelen: 255,
            f_frsize: self.block_size as isize,
            f_flags: 0,
            f_spare: [0; 4],
        })
    }

    fn super_type(&self) -> SuperType {
        SuperType::Independent
    }

    fn fs_type(&self) -> alloc::sync::Arc<dyn vfscore::fstype::VfsFsType> {
        // This will be set by the filesystem type implementation
        // For now, return a placeholder
        core::panic!("fs_type should be overridden by DbfsFsType")
    }

    fn root_inode(&self) -> VfsResult<alloc::sync::Arc<dyn vfscore::inode::VfsInode>> {
        // Get or create root inode
        if let Some(cached) = self.get_inode(self.root_ino) {
            return Ok(cached);
        }

        let root_inode = super::inode::DbfsInode::new_dir(
            Arc::new(self.clone()),
            self.root_ino,
            0o755,
            0,
            0,
            DbfsTimeSpec::default(),
        )?;

        self.insert_inode(self.root_ino, root_inode.clone());
        Ok(root_inode)
    }
}

// Make DbfsSuperBlock cloneable (Arc-like)
impl Clone for DbfsSuperBlock {
    fn clone(&self) -> Self {
        Self {
            db: self.db.clone(),
            block_size: self.block_size,
            magic: self.magic,
            root_ino: self.root_ino,
            mount_flags: self.mount_flags,
            inode_cache: Mutex::new(self.inode_cache.lock().clone()),
        }
    }
}
