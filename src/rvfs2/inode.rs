use alloc::{string::String, string::ToString, sync::Arc, vec::Vec};
use core::cmp::min;

use log::warn;
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
};

use super::{common as dbfs_common, superblock::DbfsSuperBlock};
use crate::{
    clone_db,
    common::{DbfsFileType, DbfsPermission, DbfsTimeSpec as DbfsTs},
    u16, u32, u64, usize,
};

/// DBFS Inode structure
pub struct DbfsInode {
    /// Reference to superblock
    sb: Arc<DbfsSuperBlock>,
    /// Inode number
    ino: usize,
    /// Inode type
    inode_type: VfsNodeType,
    /// File size
    size: Mutex<usize>,
    /// Hard link count
    nlink: Mutex<u32>,
    /// User ID
    uid: u32,
    /// Group ID
    gid: u32,
    /// Permissions
    perm: u16,
    /// Block size
    blksize: u32,
    /// Last access time
    atime: Mutex<DbfsTs>,
    /// Last modification time
    mtime: Mutex<DbfsTs>,
    /// Last change time
    ctime: Mutex<DbfsTs>,
    /// Symlink target (if symlink)
    symlink_target: Mutex<Option<String>>,
}

impl DbfsInode {
    /// Create a new directory inode
    pub fn new_dir(
        sb: Arc<DbfsSuperBlock>,
        ino: usize,
        perm: u16,
        uid: u32,
        gid: u32,
        ctime: DbfsTs,
    ) -> VfsResult<Arc<Self>> {
        let attr = dbfs_common_attr(ino).map_err(|_| VfsError::IoError)?;

        Ok(Arc::new(Self {
            sb,
            ino,
            inode_type: VfsNodeType::Dir,
            size: Mutex::new(attr.size),
            nlink: Mutex::new(attr.nlink),
            uid: attr.uid,
            gid: attr.gid,
            perm: attr.perm,
            blksize: attr.blksize,
            atime: Mutex::new(attr.atime),
            mtime: Mutex::new(attr.mtime),
            ctime: Mutex::new(attr.ctime),
            symlink_target: Mutex::new(None),
        }))
    }

    /// Create a new file inode
    pub fn new_file(
        sb: Arc<DbfsSuperBlock>,
        ino: usize,
        perm: u16,
        uid: u32,
        gid: u32,
        ctime: DbfsTs,
    ) -> VfsResult<Arc<Self>> {
        let attr = dbfs_common_attr(ino).map_err(|_| VfsError::IoError)?;

        Ok(Arc::new(Self {
            sb,
            ino,
            inode_type: VfsNodeType::File,
            size: Mutex::new(attr.size),
            nlink: Mutex::new(attr.nlink),
            uid: attr.uid,
            gid: attr.gid,
            perm: attr.perm,
            blksize: attr.blksize,
            atime: Mutex::new(attr.atime),
            mtime: Mutex::new(attr.mtime),
            ctime: Mutex::new(attr.ctime),
            symlink_target: Mutex::new(None),
        }))
    }

    /// Create a new symlink inode
    pub fn new_symlink(
        sb: Arc<DbfsSuperBlock>,
        ino: usize,
        perm: u16,
        uid: u32,
        gid: u32,
        target: String,
        ctime: DbfsTs,
    ) -> VfsResult<Arc<Self>> {
        let attr = dbfs_common_attr(ino).map_err(|_| VfsError::IoError)?;

        Ok(Arc::new(Self {
            sb,
            ino,
            inode_type: VfsNodeType::SymLink,
            size: Mutex::new(attr.size),
            nlink: Mutex::new(attr.nlink),
            uid: attr.uid,
            gid: attr.gid,
            perm: attr.perm,
            blksize: attr.blksize,
            atime: Mutex::new(attr.atime),
            mtime: Mutex::new(attr.mtime),
            ctime: Mutex::new(attr.ctime),
            symlink_target: Mutex::new(Some(target)),
        }))
    }

    /// Get inode number
    pub fn ino(&self) -> usize {
        self.ino
    }

    /// Get current time
    fn current_time() -> DbfsTs {
        DbfsTs {
            sec: 0,
            nsec: 0,
        }
    }

    /// Convert VfsNodeType to DbfsFileType
    fn vfs_to_dbfs_type(ty: VfsNodeType) -> DbfsFileType {
        match ty {
            VfsNodeType::File => DbfsFileType::RegularFile,
            VfsNodeType::Dir => DbfsFileType::Directory,
            VfsNodeType::SymLink => DbfsFileType::Symlink,
            VfsNodeType::CharDevice => DbfsFileType::CharDevice,
            VfsNodeType::BlockDevice => DbfsFileType::BlockDevice,
            VfsNodeType::Fifo => DbfsFileType::NamedPipe,
            VfsNodeType::Socket => DbfsFileType::Socket,
            VfsNodeType::Unknown => DbfsFileType::RegularFile, // 默认为普通文件
        }
    }

    /// Convert VfsNodePerm to DbfsPermission
    fn vfs_to_dbfs_perm(perm: VfsNodePerm, ty: VfsNodeType) -> DbfsPermission {
        let mut p = DbfsPermission::from_bits_truncate(perm.bits());
        match ty {
            VfsNodeType::File => p |= DbfsPermission::S_IFREG,
            VfsNodeType::Dir => p |= DbfsPermission::S_IFDIR,
            VfsNodeType::SymLink => p |= DbfsPermission::S_IFLNK,
            VfsNodeType::CharDevice => p |= DbfsPermission::S_IFCHR,
            VfsNodeType::BlockDevice => p |= DbfsPermission::S_IFBLK,
            VfsNodeType::Fifo => p |= DbfsPermission::S_IFIFO,
            VfsNodeType::Socket => p |= DbfsPermission::S_IFSOCK,
            VfsNodeType::Unknown => p |= DbfsPermission::S_IFREG, // 默认为普通文件
        }
        p
    }
}

impl VfsFile for DbfsInode {
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> VfsResult<usize> {
        if self.inode_type != VfsNodeType::File && self.inode_type != VfsNodeType::SymLink {
            return Err(VfsError::NoSys);
        }

        dbfs_common::dbfs_read(self.ino, buf, offset)
            .map_err(|_| VfsError::IoError)
    }

    fn write_at(&self, offset: u64, buf: &[u8]) -> VfsResult<usize> {
        if self.inode_type != VfsNodeType::File {
            return Err(VfsError::NoSys);
        }

        dbfs_common::dbfs_write(self.ino, buf, offset)
            .map_err(|_| VfsError::IoError)
            .map(|len| {
                // Update size
                let new_size = (offset as usize + len).max(*self.size.lock());
                *self.size.lock() = new_size;
                len
            })
    }

    fn readdir(&self, start_index: usize) -> VfsResult<Option<VfsDirEntry>> {
        if self.inode_type != VfsNodeType::Dir {
            return Err(VfsError::NotDir);
        }

        let db = self.sb.db();
        let tx = db.tx(false).map_err(|_| VfsError::IoError)?;
        let bucket = tx
            .get_bucket(self.ino.to_be_bytes())
            .map_err(|_| VfsError::IoError)?;

        let entries: Vec<(String, usize)> = bucket
            .kv_pairs()
            .filter_map(|kv| {
                let key = kv.key();
                if key.starts_with(b"data:") {
                    let name = String::from_utf8_lossy(&key[5..]).to_string();
                    let value = kv.value();
                    let ino = core::str::from_utf8(value)
                        .ok()?
                        .parse::<usize>()
                        .ok()?;
                    Some((name, ino))
                } else {
                    None
                }
            })
            .collect();

        if start_index >= entries.len() {
            return Ok(None);
        }

        let (name, ino) = entries[start_index].clone();

        // Get the inode type
        let attr = dbfs_common_attr(ino).map_err(|_| VfsError::IoError)?;
        let entry_type = match attr.kind {
            DbfsFileType::Directory => VfsNodeType::Dir,
            DbfsFileType::RegularFile => VfsNodeType::File,
            DbfsFileType::Symlink => VfsNodeType::SymLink,
            _ => VfsNodeType::Fifo, // Fallback
        };

        Ok(Some(VfsDirEntry {
            ino: ino as u64,
            ty: entry_type,
            name,
        }))
    }

    fn flush(&self) -> VfsResult<()> {
        // Sync the inode data
        Ok(())
    }
}

impl VfsInode for DbfsInode {
    fn get_super_block(&self) -> VfsResult<Arc<dyn VfsSuperBlock>> {
        Ok(self.sb.clone())
    }

    fn node_perm(&self) -> VfsNodePerm {
        VfsNodePerm::from_bits_truncate(self.perm & 0o777)
    }

    fn create(
        &self,
        name: &str,
        ty: VfsNodeType,
        perm: VfsNodePerm,
        rdev: Option<u64>,
    ) -> VfsResult<Arc<dyn VfsInode>> {
        if self.inode_type != VfsNodeType::Dir {
            return Err(VfsError::NotDir);
        }

        let dbfs_perm = Self::vfs_to_dbfs_perm(perm, ty);
        let ctime = Self::current_time();

        let dev = if ty == VfsNodeType::CharDevice || ty == VfsNodeType::BlockDevice {
            Some(rdev.unwrap_or(0) as u32)
        } else {
            None
        };

        let attr = dbfs_common::dbfs_create(
            self.ino,
            name,
            0,
            0,
            ctime,
            dbfs_perm,
            None, // No symlink target
            dev,
        )
        .map_err(|_| VfsError::IoError)?;

        // Create the new inode
        let new_inode = match ty {
            VfsNodeType::File => DbfsInode::new_file(
                self.sb.clone(),
                attr.ino,
                attr.perm,
                attr.uid,
                attr.gid,
                ctime,
            )?,
            VfsNodeType::Dir => DbfsInode::new_dir(
                self.sb.clone(),
                attr.ino,
                attr.perm,
                attr.uid,
                attr.gid,
                ctime,
            )?,
            VfsNodeType::SymLink => {
                return Err(VfsError::NoSys);
            }
            _ => return Err(VfsError::NoSys),
        };

        // Cache the new inode
        self.sb.insert_inode(attr.ino, new_inode.clone());

        Ok(new_inode)
    }

    fn link(&self, name: &str, src: Arc<dyn VfsInode>) -> VfsResult<Arc<dyn VfsInode>> {
        if self.inode_type != VfsNodeType::Dir {
            return Err(VfsError::NotDir);
        }

        let src_dbfs = src.downcast_arc::<DbfsInode>().map_err(|_| VfsError::Invalid)?;

        let ctime = Self::current_time();
        dbfs_common::dbfs_link(0, 0, src_dbfs.ino, self.ino, name, ctime).map_err(|_| VfsError::IoError)?;

        // Update link count
        *src_dbfs.nlink.lock() += 1;

        Ok(src_dbfs.clone())
    }

    fn unlink(&self, name: &str) -> VfsResult<()> {
        if self.inode_type != VfsNodeType::Dir {
            return Err(VfsError::NotDir);
        }

        let ctime = Self::current_time();
        dbfs_common::dbfs_unlink(0, 0, self.ino, name, None, ctime).map_err(|_| VfsError::IoError)?;

        Ok(())
    }

    fn symlink(&self, name: &str, target: &str) -> VfsResult<Arc<dyn VfsInode>> {
        if self.inode_type != VfsNodeType::Dir {
            return Err(VfsError::NotDir);
        }

        let mut perm = DbfsPermission::S_IFLNK;
        perm |= DbfsPermission::from_bits_truncate(0o777);

        let ctime = Self::current_time();
        let attr = dbfs_common::dbfs_create(
            self.ino,
            name,
            0,
            0,
            ctime,
            perm,
            Some(target),
            None,
        )
        .map_err(|_| VfsError::IoError)?;

        let symlink = DbfsInode::new_symlink(
            self.sb.clone(),
            attr.ino,
            attr.perm,
            attr.uid,
            attr.gid,
            target.to_string(),
            ctime,
        )?;

        self.sb.insert_inode(attr.ino, symlink.clone());

        Ok(symlink)
    }

    fn lookup(&self, name: &str) -> VfsResult<Arc<dyn VfsInode>> {
        if self.inode_type != VfsNodeType::Dir {
            return Err(VfsError::NotDir);
        }

        let attr = dbfs_common::dbfs_lookup(self.ino, name).map_err(|_| VfsError::NoEntry)?;

        // Check if inode is already cached
        if let Some(cached) = self.sb.get_inode(attr.ino) {
            return Ok(cached);
        }

        let ctime = Self::current_time();
        let inode = match attr.kind {
            DbfsFileType::Directory => DbfsInode::new_dir(
                self.sb.clone(),
                attr.ino,
                attr.perm,
                attr.uid,
                attr.gid,
                ctime,
            )?,
            DbfsFileType::RegularFile => DbfsInode::new_file(
                self.sb.clone(),
                attr.ino,
                attr.perm,
                attr.uid,
                attr.gid,
                ctime,
            )?,
            DbfsFileType::Symlink => {
                let mut target = [0u8; 4096];
                let len = dbfs_common::dbfs_readlink(attr.ino, &mut target).map_err(|_| VfsError::IoError)?;
                let target_str = core::str::from_utf8(&target[..len])
                    .map_err(|_| VfsError::Invalid)?
                    .to_string();

                DbfsInode::new_symlink(
                    self.sb.clone(),
                    attr.ino,
                    attr.perm,
                    attr.uid,
                    attr.gid,
                    target_str,
                    ctime,
                )?
            }
            _ => return Err(VfsError::NoSys),
        };

        self.sb.insert_inode(attr.ino, inode.clone());

        Ok(inode)
    }

    fn rmdir(&self, name: &str) -> VfsResult<()> {
        if self.inode_type != VfsNodeType::Dir {
            return Err(VfsError::NotDir);
        }

        let ctime = Self::current_time();
        dbfs_common::dbfs_rmdir(0, 0, self.ino, name, ctime).map_err(|e| match e {
            crate::common::DbfsError::NotEmpty => VfsError::NotEmpty,
            _ => VfsError::IoError,
        })?;

        Ok(())
    }

    fn readlink(&self, _buf: &mut [u8]) -> VfsResult<usize> {
        Err(VfsError::NoSys)
    }

    fn set_attr(&self, attr: InodeAttr) -> VfsResult<()> {
        // Update attributes in database
        let db = self.sb.db();
        let tx = db.tx(true).map_err(|_| VfsError::IoError)?;
        let bucket = tx
            .get_bucket(self.ino.to_be_bytes())
            .map_err(|_| VfsError::IoError)?;

        // Update size if changed
        if attr.size as usize != *self.size.lock() {
            bucket
                .put("size", attr.size.to_be_bytes())
                .map_err(|_| VfsError::IoError)?;
            *self.size.lock() = attr.size as usize;
        }

        // Update permissions
        if attr.mode as u16 != self.perm {
            bucket
                .put("mode", (attr.mode as u16).to_be_bytes())
                .map_err(|_| VfsError::IoError)?;
            // SAFETY: We're updating the permission, which is safe
            // Note: This modifies through an immutable reference, but that's the design
            // In production code, this should be refactored
        }

        tx.commit().map_err(|_| VfsError::IoError)?;

        Ok(())
    }

    fn get_attr(&self) -> VfsResult<vfscore::utils::VfsFileStat> {
        let attr = dbfs_common_attr(self.ino).map_err(|_| VfsError::IoError)?;

        let mode = VfsInodeMode::from(
            VfsNodePerm::from_bits_truncate(attr.perm & 0o777),
            match attr.kind {
                DbfsFileType::Directory => VfsNodeType::Dir,
                DbfsFileType::RegularFile => VfsNodeType::File,
                DbfsFileType::Symlink => VfsNodeType::SymLink,
                DbfsFileType::CharDevice => VfsNodeType::CharDevice,
                DbfsFileType::BlockDevice => VfsNodeType::BlockDevice,
                DbfsFileType::NamedPipe => VfsNodeType::Fifo,
                DbfsFileType::Socket => VfsNodeType::Socket,
            },
        );

        Ok(vfscore::utils::VfsFileStat {
            st_dev: 0,
            st_ino: attr.ino as u64,
            st_nlink: attr.nlink as u32,
            st_mode: mode.bits(),
            st_uid: attr.uid,
            st_gid: attr.gid,
            st_rdev: attr.rdev as u64,
            __pad: 0,
            st_size: attr.size as u64,
            st_blksize: attr.blksize as u32,
            __pad2: 0,
            st_blocks: attr.blocks as u64,
            st_atime: vfscore::utils::VfsTimeSpec {
                sec: attr.atime.sec,
                nsec: attr.atime.nsec as _,
            },
            st_mtime: vfscore::utils::VfsTimeSpec {
                sec: attr.mtime.sec,
                nsec: attr.mtime.nsec as _,
            },
            st_ctime: vfscore::utils::VfsTimeSpec {
                sec: attr.ctime.sec,
                nsec: attr.ctime.nsec as _,
            },
            unused: 0,
        })
    }

    fn list_xattr(&self) -> VfsResult<Vec<String>> {
        // Return empty list for now
        // Can be implemented to read extended attributes from database
        Ok(Vec::new())
    }

    fn inode_type(&self) -> VfsNodeType {
        self.inode_type
    }

    fn truncate(&self, len: u64) -> VfsResult<()> {
        if self.inode_type != VfsNodeType::File {
            return Err(VfsError::NoSys);
        }

        let ctime = Self::current_time();
        dbfs_common::dbfs_truncate(0, 0, self.ino, ctime, len as usize)
            .map_err(|_| VfsError::IoError)?;

        *self.size.lock() = len as usize;

        Ok(())
    }

    fn rename_to(
        &self,
        old_name: &str,
        new_parent: Arc<dyn VfsInode>,
        new_name: &str,
        flag: VfsRenameFlag,
    ) -> VfsResult<()> {
        let new_parent_dbfs = new_parent
            .downcast_arc::<DbfsInode>()
            .map_err(|_| VfsError::Invalid)?;

        let ctime = Self::current_time();
        dbfs_common::dbfs_rename(
            0,
            0,
            self.ino,
            old_name,
            new_parent_dbfs.ino,
            new_name,
            flag.bits() as u32,
            ctime,
        )
        .map_err(|_| VfsError::IoError)?;

        Ok(())
    }

    fn update_time(&self, time: VfsTime, _now: VfsTimeSpec) -> VfsResult<()> {
        let db = self.sb.db();
        let tx = db.tx(true).map_err(|_| VfsError::IoError)?;
        let bucket = tx
            .get_bucket(self.ino.to_be_bytes())
            .map_err(|_| VfsError::IoError)?;

        let ctime = Self::current_time();

        match time {
            VfsTime::AccessTime(ts) => {
                let mut bytes = [0u8; 16];
                bytes[0..8].copy_from_slice(&ts.sec.to_be_bytes());
                bytes[8..16].copy_from_slice(&ts.nsec.to_be_bytes());
                bucket.put("atime", bytes).map_err(|_| VfsError::IoError)?;
            }
            VfsTime::ModifiedTime(ts) => {
                let mut bytes = [0u8; 16];
                bytes[0..8].copy_from_slice(&ts.sec.to_be_bytes());
                bytes[8..16].copy_from_slice(&ts.nsec.to_be_bytes());
                bucket.put("mtime", bytes).map_err(|_| VfsError::IoError)?;
            }
            _ => {
                // Ctime is not in VfsTime enum, handle separately
                let mut bytes = [0u8; 16];
                bytes[0..8].copy_from_slice(&ctime.sec.to_be_bytes());
                bytes[8..16].copy_from_slice(&ctime.nsec.to_be_bytes());
                bucket.put("ctime", bytes).map_err(|_| VfsError::IoError)?;
            }
        }

        tx.commit().map_err(|_| VfsError::IoError)?;

        Ok(())
    }
}
