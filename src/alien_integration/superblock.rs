//! DBFS SuperBlock for Alien Integration
//!
//! Phase 1: 最小化实现，为事务预留结构

use alloc::{string::String, string::ToString, sync::Arc};
use vfscore::{
    fstype::VfsFsType,
    superblock::{SuperType, VfsSuperBlock},
    utils::VfsFsStat,
    VfsResult,
};

use super::{fstype::DummyFsType, inode::DbfsInode};

/// DBFS SuperBlock
///
/// Phase 1: 简化实现，使用内存存储
/// 未来: 可以添加真实数据库支持
pub struct DbfsSuperBlock {
    /// Block size (固定 4KB)
    block_size: u64,
    /// 文件系统类型引用 (用于 fs_type())
    db_path: String,
}

impl DbfsSuperBlock {
    /// Create a new superblock
    pub fn new(db_path: String) -> Self {
        Self {
            block_size: 4096,
            db_path,
        }
    }

    /// Create root inode
    pub fn root_inode(self: &Arc<Self>) -> VfsResult<Arc<dyn vfscore::inode::VfsInode>> {
        Ok(DbfsInode::new_root(self.clone()))
    }
}

impl VfsSuperBlock for DbfsSuperBlock {
    fn sync_fs(&self, _wait: bool) -> VfsResult<()> {
        // Phase 1: 同步操作，无需实际 sync
        Ok(())
    }

    fn stat_fs(&self) -> VfsResult<VfsFsStat> {
        Ok(VfsFsStat {
            f_bsize: self.block_size as i64,
            f_frsize: self.block_size as i64,
            f_blocks: 1024,    // 假设 4MB 空间
            f_bfree: 512,
            f_bavail: 512,
            f_files: 100,      // 最多 100 个 inode
            f_ffree: 50,
            f_favail: 50,
            f_fsid: 0x44424653, // "DBFS"
            f_flag: 0,
            f_namemax: 255,
            name: [0; 32],
        })
    }

    fn super_type(&self) -> SuperType {
        SuperType::Other
    }

    fn fs_type(&self) -> Arc<dyn VfsFsType> {
        // Phase 1: 返回 dummy FsType
        Arc::new(DummyFsType {
            name: self.db_path.clone(),
        })
    }

    fn root_inode(&self) -> VfsResult<Arc<dyn vfscore::inode::VfsInode>> {
        // Phase 1: 创建一个新的 root inode
        // 注意: 每次调用都会创建新的，这是一个简化实现
        let dummy_sb = Arc::new(DbfsSuperBlock::new(self.db_path.clone()));
        Ok(DbfsInode::new_root(dummy_sb))
    }
}
