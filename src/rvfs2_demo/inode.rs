//! DBFS Inode - Minimal demo implementation
//!
//! Provides:
//! - Root inode (ino = 1)
//! - lookup("hello") returns fixed inode (ino = 2)
//! - read_at() returns fixed content "Hello, DBFS!"

use alloc::{sync::Arc, vec::Vec};
use core::sync::atomic::{AtomicUsize, Ordering};
use spin::Mutex;
use vfscore::{
    error::VfsError,
    file::VfsFile,
    inode::{InodeAttr, VfsInode},
    superblock::VfsSuperBlock,
    utils::{
        VfsDirEntry, VfsInodeMode, VfsNodePerm, VfsNodeType, VfsRenameFlag, VfsTime, VfsTimeSpec,
    },
    VfsResult,
}

use super::superblock::DbfsSuperBlock;

/// DBFS Inode - minimal demo implementation
pub struct DbfsInode {
    /// Reference to superblock
    sb: Arc<DbfsSuperBlock>,
    /// Inode number
    ino: usize,
    /// Inode type
    inode_type: VfsNodeType,
    /// Cached size
    size: Arc<AtomicUsize>,
    /// Number of hard links
    nlink: Arc<AtomicUsize>,
}

impl DbfsInode {
    /// Create root inode (ino = 1)
    pub fn new_root(sb: Arc<DbfsSuperBlock>) -> Arc<Self> {
        Arc::new(Self {
            sb,
            ino: 1,
            inode_type: VfsNodeType::Dir,
            size: Arc::new(AtomicUsize::new(4096)),
            nlink: Arc::new(AtomicUsize::new(2)),
        })
    }

    /// Create a fixed "hello" file inode (ino = 2)
    fn new_hello_file(sb: Arc<DbfsSuperBlock>) -> Arc<Self> {
        Arc::new(Self {
            sb,
            ino: 2,
            inode_type: VfsNodeType::File,
            size: Arc::new(AtomicUsize::new(12)), // "Hello, DBFS!"
            nlink: Arc::new(AtomicUsize::new(1)),
        })
    }

    /// Get current time (simplified)
    fn current_time() -> VfsTimeSpec {
        VfsTimeSpec::default()
    }
}

impl VfsInode for DbfsInode {
    fn inode_type(&self) -> VfsNodeType {
        self.inode_type
    }

    fn mode(&self) -> VfsNodePerm {
        if self.inode_type == VfsNodeType::Dir {
            VfsNodePerm::from_bits_truncate(0o755)
        } else {
            VfsNodePerm::from_bits_truncate(0o644)
        }
    }

    fn uid(&self) -> usize {
        0
    }

    fn gid(&self) -> usize {
        0
    }

    fn size(&self) -> usize {
        self.size.load(Ordering::SeqCst)
    }

    fn blocks(&self) -> u64 {
        1
    }

    fn nlink(&self) -> usize {
        self.nlink.load(Ordering::SeqCst)
    }

    fn atime(&self) -> VfsTimeSpec {
        Self::current_time()
    }

    fn mtime(&self) -> VfsTimeSpec {
        Self::current_time()
    }

    fn ctime(&self) -> VfsTimeSpec {
        Self::current_time()
    }

    fn fnode(&self) -> VfsResult<u64> {
        Ok(self.ino as u64)
    }

    fn lookup(&self, name: &str) -> VfsResult<Arc<dyn VfsInode>> {
        if self.inode_type != VfsNodeType::Dir {
            return Err(VfsError::NotDir);
        }

        // Demo: Only support "hello" file
        if name == "hello" {
            info!("✓ DBFS Demo: lookup(\"hello\") -> inode 2");
            return Ok(Self::new_hello_file(self.sb.clone()));
        }

        // Also support "." and ".."
        if name == "." || name == ".." {
            return Ok(self.sb.root_inode()?);
        }

        Err(VfsError::NoEntry)
    }

    fn create(
        &self,
        _name: &str,
        _ty: VfsNodeType,
        _perm: VfsNodePerm,
        _rdev: Option<u64>,
    ) -> VfsResult<Arc<dyn VfsInode>> {
        Err(VfsError::NoSys)
    }

    fn link(&self, _name: &str, _src: Arc<dyn VfsInode>) -> VfsResult<Arc<dyn VfsInode>> {
        Err(VfsError::NoSys)
    }

    fn unlink(&self, _name: &str) -> VfsResult<()> {
        Err(VfsError::NoSys)
    }

    fn symlink(
        &self,
        _name: &str,
        _target: &str,
    ) -> VfsResult<Arc<dyn VfsInode>> {
        Err(VfsError::NoSys)
    }

    fn mkdir(
        &self,
        _name: &str,
        _perm: VfsNodePerm,
    ) -> VfsResult<Arc<dyn VfsInode>> {
        Err(VfsError::NoSys)
    }

    fn rmdir(&self, _name: &str) -> VfsResult<()> {
        Err(VfsError::NoSys)
    }

    fn readdir(&self) -> VfsResult<Vec<VfsDirEntry>> {
        if self.inode_type != VfsNodeType::Dir {
            return Err(VfsError::NotDir);
        }

        let mut entries = Vec::new();

        // Add "."
        entries.push(VfsDirEntry {
            ino: 1,
            offset: 0,
            type_: VfsNodeType::Dir,
            name: ".".to_string(),
        });

        // Add ".."
        entries.push(VfsDirEntry {
            ino: 1,
            offset: 1,
            type_: VfsNodeType::Dir,
            name: "..".to_string(),
        });

        // Add "hello" file
        entries.push(VfsDirEntry {
            ino: 2,
            offset: 2,
            type_: VfsNodeType::File,
            name: "hello".to_string(),
        });

        info!("✓ DBFS Demo: readdir() returned {} entries", entries.len());
        Ok(entries)
    }

    fn rename_to(
        &self,
        _old_name: &str,
        _new_parent: Arc<dyn VfsInode>,
        _new_name: &str,
        _flags: VfsRenameFlag,
    ) -> VfsResult<()> {
        Err(VfsError::NoSys)
    }
}

impl VfsFile for DbfsInode {
    fn read_at(&self, _offset: u64, buf: &mut [u8]) -> VfsResult<usize> {
        if self.inode_type != VfsNodeType::File {
            return Err(VfsError::IsDir);
        }

        // Demo: Return fixed content "Hello, DBFS!"
        let content = b"Hello, DBFS!";
        let bytes_to_copy = core::cmp::min(buf.len(), content.len());
        buf[..bytes_to_copy].copy_from_slice(&content[..bytes_to_copy]);

        info!(
            "✓ DBFS Demo: read_at() returned {} bytes: \"{}\"",
            bytes_to_copy,
            core::str::from_utf8(&content[..bytes_to_copy]).unwrap_or("<invalid>")
        );

        Ok(bytes_to_copy)
    }

    fn write_at(&self, _buf: &[u8], _offset: u64) -> VfsResult<usize> {
        // Demo: Read-only filesystem
        Err(VfsError::NoSys)
    }

    fn flush(&self) -> VfsResult<()> {
        Ok(())
    }

    fn fsync(&self, _datasync: bool) -> VfsResult<()> {
        Ok(())
    }
}
