//! DBFS Inode for Alien Integration
//!
//! Phase 1: 实现基本文件和目录操作
//!
//! 支持的操作:
//! - ✅ lookup: 查找文件/目录
//! - ✅ create: 创建文件
//! - ✅ mkdir: 通过 create 实现
//! - ✅ read_at: 读取文件
//! - ✅ write_at: 写入文件
//! - ✅ unlink: 删除文件
//! - ✅ rmdir: 删除目录
//!
//! ❌ 不实现: xattr, symlink, 权限检查

use alloc::{collections::BTreeMap, string::String, string::ToString, sync::Arc, vec::Vec};
use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use spin::Mutex;
use vfscore::{
    error::VfsError,
    file::VfsFile,
    inode::{InodeAttr, VfsInode},
    superblock::VfsSuperBlock,
    utils::{
        VfsDirEntry, VfsNodePerm, VfsNodeType, VfsRenameFlag, VfsTime, VfsTimeSpec,
        VfsFileStat,
    },
    VfsResult,
};

use super::superblock::DbfsSuperBlock;

/// Inode 数据存储
#[derive(Debug)]
enum InodeData {
    File { data: Vec<u8> },
    Directory {
        entries: BTreeMap<String, (u64, VfsNodeType)>, // name -> (ino, type)
    },
}

/// DBFS Inode
pub struct DbfsInode {
    /// Superblock 引用
    sb: Arc<DbfsSuperBlock>,
    /// Inode 号
    ino: u64,
    /// Inode 类型
    inode_type: VfsNodeType,
    /// Inode 数据
    data: Mutex<InodeData>,
    /// 权限
    perm: VfsNodePerm,
    /// 下一个可用的 inode 号 (全局)
    next_ino: Arc<AtomicU64>,
}

impl DbfsInode {
    /// Create root inode (ino = 1)
    pub fn new_root(sb: Arc<DbfsSuperBlock>) -> Arc<Self> {
        Arc::new(Self {
            sb,
            ino: 1,
            inode_type: VfsNodeType::Dir,
            data: Mutex::new(InodeData::Directory {
                entries: BTreeMap::new(),
            }),
            perm: VfsNodePerm::from_bits_truncate(0o755),
            next_ino: Arc::new(AtomicU64::new(2)), // 下一个从 2 开始
        })
    }

    /// Create a new inode
    fn new_inode(
        sb: Arc<DbfsSuperBlock>,
        parent: &Arc<Self>,
        name: &str,
        type_: VfsNodeType,
    ) -> Arc<Self> {
        let ino = parent.next_ino.fetch_add(1, Ordering::SeqCst);
        let data = match type_ {
            VfsNodeType::File => InodeData::File { data: Vec::new() },
            VfsNodeType::Dir => InodeData::Directory {
                entries: BTreeMap::new(),
            },
            _ => InodeData::File { data: Vec::new() },
        };

        let perm = match type_ {
            VfsNodeType::File => VfsNodePerm::from_bits_truncate(0o644),
            VfsNodeType::Dir => VfsNodePerm::from_bits_truncate(0o755),
            _ => VfsNodePerm::from_bits_truncate(0o644),
        };

        Arc::new(Self {
            sb,
            ino,
            inode_type: type_,
            data: Mutex::new(data),
            perm,
            next_ino: parent.next_ino.clone(),
        })
    }

    /// Get current time (simplified)
    fn current_time() -> VfsTimeSpec {
        VfsTimeSpec::default()
    }

    /// Get file size
    fn get_size(&self) -> usize {
        match &*self.data.lock() {
            InodeData::File { data } => data.len(),
            InodeData::Directory { entries } => entries.len() * 256, // 估算
        }
    }
}

impl VfsInode for DbfsInode {
    fn get_super_block(&self) -> VfsResult<Arc<dyn VfsSuperBlock>> {
        Ok(self.sb.clone())
    }

    fn node_perm(&self) -> VfsNodePerm {
        self.perm
    }

    fn create(
        &self,
        name: &str,
        ty: VfsNodeType,
        _perm: VfsNodePerm,
        _rdev: Option<u64>,
    ) -> VfsResult<Arc<dyn VfsInode>> {
        if self.inode_type != VfsNodeType::Dir {
            return Err(VfsError::NotDir);
        }

        // Check if exists
        let data = self.data.lock();
        if let InodeData::Directory { ref entries } = &*data {
            if entries.contains_key(name) {
                return Err(VfsError::EExist);
            }
        }
        drop(data);

        // Create new inode
        let new_inode = Self::new_inode(self.sb.clone(), self, name, ty);

        // Insert into parent
        let mut data = self.data.lock();
        if let InodeData::Directory { ref mut entries } = &mut *data {
            entries.insert(name.to_string(), (new_inode.ino, ty));
        }

        Ok(new_inode as Arc<dyn VfsInode>)
    }

    fn link(&self, _name: &str, _src: Arc<dyn VfsInode>) -> VfsResult<Arc<dyn VfsInode>> {
        Err(VfsError::NoSys)
    }

    fn unlink(&self, name: &str) -> VfsResult<()> {
        if self.inode_type != VfsNodeType::Dir {
            return Err(VfsError::NotDir);
        }

        if name == "." || name == ".." {
            return Err(VfsError::EExist); // Cannot delete . or ..
        }

        let mut data = self.data.lock();
        if let InodeData::Directory { ref mut entries } = &mut *data {
            entries.remove(name)
                .ok_or(VfsError::NoEntry)?;
        }
        Ok(())
    }

    fn symlink(
        &self,
        _name: &str,
        _sy_name: &str,
    ) -> VfsResult<Arc<dyn VfsInode>> {
        Err(VfsError::NoSys)
    }

    fn lookup(&self, name: &str) -> VfsResult<Arc<dyn VfsInode>> {
        if self.inode_type != VfsNodeType::Dir {
            return Err(VfsError::NotDir);
        }

        // Special entries
        if name == "." || name == ".." {
            // Return self for both . and ..
            return Ok(Arc::new(Self {
                sb: self.sb.clone(),
                ino: self.ino,
                inode_type: self.inode_type,
                data: Mutex::new(match &*self.data.lock() {
                    InodeData::File { data } => InodeData::File {
                        data: data.clone(),
                    },
                    InodeData::Directory { entries } => InodeData::Directory {
                        entries: entries.clone(),
                    },
                }),
                perm: self.perm,
                next_ino: self.next_ino.clone(),
            }) as Arc<dyn VfsInode>);
        }

        // Find in directory
        let data = self.data.lock();
        if let InodeData::Directory { ref entries } = &*data {
            if let Some(&(ino, type_)) = entries.get(name) {
                // Phase 1: 简化实现，创建一个临时 inode
                // 实际需要从全局 inode 表中查找
                let new_data = match type_ {
                    VfsNodeType::File => InodeData::File { data: Vec::new() },
                    VfsNodeType::Dir => InodeData::Directory {
                        entries: BTreeMap::new(),
                    },
                    _ => InodeData::File { data: Vec::new() },
                };

                let perm = match type_ {
                    VfsNodeType::File => VfsNodePerm::from_bits_truncate(0o644),
                    VfsNodeType::Dir => VfsNodePerm::from_bits_truncate(0o755),
                    _ => VfsNodePerm::from_bits_truncate(0o644),
                };

                return Ok(Arc::new(Self {
                    sb: self.sb.clone(),
                    ino,
                    inode_type: type_,
                    data: Mutex::new(new_data),
                    perm,
                    next_ino: Arc::new(AtomicU64::new(0)),
                }) as Arc<dyn VfsInode>);
            }
        }

        Err(VfsError::NoEntry)
    }

    fn rmdir(&self, name: &str) -> VfsResult<()> {
        // Phase 1: 简化实现，不检查目录是否为空
        self.unlink(name)
    }

    fn readlink(&self, _buf: &mut [u8]) -> VfsResult<usize> {
        Err(VfsError::NoSys)
    }

    fn set_attr(&self, _attr: InodeAttr) -> VfsResult<()> {
        Ok(())
    }

    fn get_attr(&self) -> VfsResult<VfsFileStat> {
        Ok(VfsFileStat {
            st_mode: 0,
            st_nlink: 1,
            st_size: self.get_size() as i64,
            st_blocks: 1,
            st_uid: 0,
            st_gid: 0,
            st_dev: 0,
            st_ino: self.ino,
            st_rdev: 0,
            st_atim: Self::current_time(),
            st_mtim: Self::current_time(),
            st_ctim: Self::current_time(),
            st_blksize: 4096,
            st_flags: 0,
        })
    }

    fn list_xattr(&self) -> VfsResult<Vec<String>> {
        Ok(Vec::new())
    }

    fn inode_type(&self) -> VfsNodeType {
        self.inode_type
    }

    fn truncate(&self, _len: u64) -> VfsResult<()> {
        Err(VfsError::NoSys)
    }

    fn rename_to(
        &self,
        _old_name: &str,
        _new_parent: Arc<dyn VfsInode>,
        _new_name: &str,
        _flag: VfsRenameFlag,
    ) -> VfsResult<()> {
        Err(VfsError::NoSys)
    }

    fn update_time(&self, _time: VfsTime, _now: VfsTimeSpec) -> VfsResult<()> {
        Ok(())
    }
}

impl VfsFile for DbfsInode {
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> VfsResult<usize> {
        if self.inode_type != VfsNodeType::File {
            return Err(VfsError::IsDir);
        }

        let data = self.data.lock();
        if let InodeData::File { ref data } = &*data {
            let start = offset as usize;
            if start >= data.len() {
                return Ok(0);
            }

            let bytes_to_read = core::cmp::min(buf.len(), data.len() - start);
            buf[..bytes_to_read].copy_from_slice(&data[start..start + bytes_to_read]);

            Ok(bytes_to_read)
        } else {
            Err(VfsError::IsDir)
        }
    }

    fn write_at(&self, offset: u64, buf: &[u8]) -> VfsResult<usize> {
        if self.inode_type != VfsNodeType::File {
            return Err(VfsError::IsDir);
        }

        let mut data = self.data.lock();
        if let InodeData::File { ref mut data } = &mut *data {
            let start = offset as usize;

            // Extend if necessary
            if start + buf.len() > data.len() {
                data.resize(start + buf.len(), 0);
            }

            // Write data
            data[start..start + buf.len()].copy_from_slice(buf);

            Ok(buf.len())
        } else {
            Err(VfsError::IsDir)
        }
    }

    fn flush(&self) -> VfsResult<()> {
        Ok(())
    }

    fn fsync(&self, _datasync: bool) -> VfsResult<()> {
        Ok(())
    }
}
