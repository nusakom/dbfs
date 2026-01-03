use vfscore::{
    VfsResult, VfsInode, VfsSuperBlock, VfsFsType, VfsDentry, VfsError, VfsNodeType,
    VfsFile, utils::{VfsFileStat, VfsNodePerm, VfsTimeSpec, VfsDirEntry, VfsFsStat, VfsTime},
    inode::{InodeAttr},
    superblock::SuperType,
};
use alloc::{sync::{Arc, Weak}, string::String, collections::BTreeMap, string::ToString};
use spin::Mutex;
use vfscore::fstype::VfsMountPoint;

pub struct DbfsDentry<D: BlockDevice> {
    inner: Mutex<DbfsDentryInner<D>>,
}

struct DbfsDentryInner<D: BlockDevice> {
    parent: Weak<dyn VfsDentry>,
    inode: Arc<DbfsInode<D>>,
    name: String,
    mnt: Option<VfsMountPoint>,
    children: BTreeMap<String, Arc<DbfsDentry<D>>>,
}

impl<D: BlockDevice + 'static> DbfsDentry<D> {
    pub fn new(inode: Arc<DbfsInode<D>>, parent: Weak<dyn VfsDentry>, name: String) -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(DbfsDentryInner {
                parent,
                inode,
                name,
                mnt: None,
                children: BTreeMap::new(),
            }),
        })
    }
}

impl<D: BlockDevice + 'static> VfsDentry for DbfsDentry<D> {
    fn name(&self) -> String {
        self.inner.lock().name.clone()
    }

    fn inode(&self) -> VfsResult<Arc<dyn VfsInode>> {
        Ok(self.inner.lock().inode.clone() as Arc<dyn VfsInode>)
    }

    fn parent(&self) -> Option<Arc<dyn VfsDentry>> {
        self.inner.lock().parent.upgrade()
    }

    fn set_parent(&self, parent: &Arc<dyn VfsDentry>) {
        self.inner.lock().parent = Arc::downgrade(parent);
    }

    fn mount_point(&self) -> Option<VfsMountPoint> {
        self.inner.lock().mnt.clone()
    }

    fn clear_mount_point(&self) {
        self.inner.lock().mnt = None;
    }

    fn to_mount_point(self: Arc<Self>, sub_fs_root: Arc<dyn VfsDentry>, mount_flag: u32) -> VfsResult<()> {
        let mut inner = self.inner.lock();
        inner.mnt = Some(VfsMountPoint {
            root: sub_fs_root,
            mount_point: Arc::downgrade(&(self.clone() as Arc<dyn VfsDentry>)),
            mnt_flags: mount_flag,
        });
        Ok(())
    }

    fn find(&self, path: &str) -> Option<Arc<dyn VfsDentry>> {
        let inner = self.inner.lock();
        inner.children.get(path).map(|c| c.clone() as Arc<dyn VfsDentry>)
    }

    fn insert(self: Arc<Self>, name: &str, child: Arc<dyn VfsInode>) -> VfsResult<Arc<dyn VfsDentry>> {
        let mut inner = self.inner.lock();
        let dbfs_inode = child.downcast_arc::<DbfsInode<D>>().map_err(|_| VfsError::Invalid)?;
        let dentry = DbfsDentry::new(dbfs_inode, Arc::downgrade(&(self.clone() as Arc<dyn VfsDentry>)), name.to_string());
        inner.children.insert(name.to_string(), dentry.clone());
        Ok(dentry as Arc<dyn VfsDentry>)
    }

    fn remove(&self, name: &str) -> Option<Arc<dyn VfsDentry>> {
        let mut inner = self.inner.lock();
        inner.children.remove(name).map(|c| c as Arc<dyn VfsDentry>)
    }
}
use crate::common::DbfsResult;
use crate::log_manager::BlockDevice;
use crate::tx_engine::TransactionEngine;
use crate::models::InodeMetadata;
use jammdb::{DbFile, FileExt, IOResult, MetaData, OpenOption, File as JammFile};

/// 桥接 vfscore 的 VfsInode 到 jammdb 的 DbFile trait
pub struct JammdbFileAdapter {
    pub inode: Arc<dyn VfsInode>,
    pub pos: Mutex<u64>,
}

impl core2::io::Read for JammdbFileAdapter {
    fn read(&mut self, buf: &mut [u8]) -> core2::io::Result<usize> {
        let mut pos = self.pos.lock();
        let n = self.inode.read_at(*pos, buf).map_err(|_| core2::io::Error::new(core2::io::ErrorKind::Other, "read error"))?;
        *pos += n as u64;
        Ok(n)
    }
}

impl core2::io::Write for JammdbFileAdapter {
    fn write(&mut self, buf: &[u8]) -> core2::io::Result<usize> {
        let mut pos = self.pos.lock();
        let n = self.inode.write_at(*pos, buf).map_err(|_| core2::io::Error::new(core2::io::ErrorKind::Other, "write error"))?;
        *pos += n as u64;
        Ok(n)
    }
    fn flush(&mut self) -> core2::io::Result<()> {
        Ok(())
    }
}

impl core2::io::Seek for JammdbFileAdapter {
    fn seek(&mut self, pos: core2::io::SeekFrom) -> core2::io::Result<u64> {
        let mut current_pos = self.pos.lock();
        let size = self.inode.get_attr().map(|a| a.st_size).unwrap_or(0);
        let new_pos = match pos {
            core2::io::SeekFrom::Start(s) => s,
            core2::io::SeekFrom::End(e) => (size as i64 + e) as u64,
            core2::io::SeekFrom::Current(c) => (*current_pos as i64 + c) as u64,
        };
        *current_pos = new_pos;
        Ok(new_pos)
    }
}

impl FileExt for JammdbFileAdapter {
    fn lock_exclusive(&self) -> IOResult<()> { Ok(()) }
    fn unlock(&self) -> IOResult<()> { Ok(()) }
    fn size(&self) -> usize {
        self.inode.get_attr().map(|a| a.st_size as usize).unwrap_or(0)
    }
    fn metadata(&self) -> IOResult<MetaData> {
        let attr = self.inode.get_attr().map_err(|_| core2::io::Error::new(core2::io::ErrorKind::Other, "stat error"))?;
        Ok(MetaData { len: attr.st_size })
    }
    fn sync_all(&self) -> IOResult<()> {
        self.inode.fsync().map_err(|_| core2::io::Error::new(core2::io::ErrorKind::Other, "fsync error"))
    }
    fn allocate(&mut self, new_size: u64) -> IOResult<()> {
        self.inode.truncate(new_size).map_err(|_| core2::io::Error::new(core2::io::ErrorKind::Other, "truncate error"))
    }
    fn addr(&self) -> usize { 0 }
}

impl DbFile for JammdbFileAdapter {}

pub struct JammdbOpenOptions {
    pub dev: Arc<dyn VfsInode>,
}

impl OpenOption for JammdbOpenOptions {
    fn new() -> Self { panic!("Use JammdbOpenOptions::with_dev") }
    fn read(&mut self, _read: bool) -> &mut Self { self }
    fn write(&mut self, _write: bool) -> &mut Self { self }
    fn create(&mut self, _create: bool) -> &mut Self { self }
    fn open<T: ToString + jammdb::PathLike>(&mut self, _path: &T) -> IOResult<JammFile> {
        let adapter = JammdbFileAdapter {
            inode: self.dev.clone(),
            pos: Mutex::new(0),
        };
        Ok(JammFile::new(Box::new(adapter)))
    }
}

/// 桥接 vfscore 的 VfsInode 到我们的 BlockDevice trait
pub struct VfsBlockDeviceAdapter {
    pub inode: Arc<dyn VfsInode>,
}

impl BlockDevice for VfsBlockDeviceAdapter {
    fn read_at(&self, pos: u64, buf: &mut [u8]) -> DbfsResult<usize> {
        self.inode.read_at(pos, buf).map_err(|_| crate::common::DbfsError::Io)
    }
    fn write_at(&self, pos: u64, buf: &[u8]) -> DbfsResult<usize> {
        self.inode.write_at(pos, buf).map_err(|_| crate::common::DbfsError::Io)
    }
    fn size(&self) -> u64 {
        self.inode.get_attr().map(|a| a.st_size).unwrap_or(0)
    }
}

impl BlockDevice for Arc<VfsBlockDeviceAdapter> {
    fn read_at(&self, pos: u64, buf: &mut [u8]) -> DbfsResult<usize> {
        (**self).read_at(pos, buf)
    }
    fn write_at(&self, pos: u64, buf: &[u8]) -> DbfsResult<usize> {
        (**self).write_at(pos, buf)
    }
    fn size(&self) -> u64 {
        (**self).size()
    }
}

unsafe impl Send for VfsBlockDeviceAdapter {}
unsafe impl Sync for VfsBlockDeviceAdapter {}

/// DBFS-T 文件系统类型定义
pub struct DbfsFsType;

impl VfsFsType for DbfsFsType {
    fn mount(
        self: Arc<Self>,
        _flags: u32,
        _ab_mnt: &str,
        dev: Option<Arc<dyn VfsInode>>,
        _data: &[u8],
    ) -> VfsResult<Arc<dyn VfsDentry>> {
        let dev = dev.ok_or(VfsError::Invalid)?;
        if dev.inode_type() != VfsNodeType::BlockDevice {
            return Err(VfsError::Invalid);
        }

        let adapter = Arc::new(VfsBlockDeviceAdapter { inode: dev.clone() });
        
        // 1. 初始化数据库打开选项
        let mut options = JammdbOpenOptions { dev: dev.clone() };
        
        // 2. 尝试打开数据库，如果失败且磁盘足够大，则尝试初始化
        let db = match jammdb::DB::open(&mut options, &"dbfs.db".to_string()) {
            Ok(db) => db,
            Err(_) => {
                if adapter.size() < 4096 { 
                    return Err(VfsError::Invalid);
                }
                // 强制初始化 (模拟 mkfs)
                // 注意：实际生产中应有更严格的 magic number 检查
                jammdb::DB::open(&mut options, &"dbfs.db".to_string()).map_err(|_| VfsError::IoError)?
            }
        };

        // 3. 初始化 LogManager
        // 假设数据库文件前 32MB 为 jammdb 使用，之后为日志追加区
        let db_reserved_size = 32 * 1024 * 1024; 
        if adapter.size() < db_reserved_size {
            // 如果磁盘太小，根据实际大小调整或报错
            // 这里简单处理为至少 32MB
        }
        let log_manager = LogManager::new(adapter.clone() as Arc<dyn BlockDevice>, db_reserved_size);

        // 4. 初始化文件系统结构 (如果尚未初始化)
        {
            let tx = db.begin_batch();
            if tx.get_bucket("inodes").is_err() {
                // 初始化元数据 bucket
                tx.create_bucket("inodes").map_err(|_| VfsError::IoError)?;
                
                // 初始化超级块信息 bucket
                let sb_bucket = tx.create_bucket("super_blk").map_err(|_| VfsError::IoError)?;
                sb_bucket.put("magic", 0x44424653u32.to_be_bytes()).unwrap(); // "DBFS"
                sb_bucket.put("disk_size", adapter.size().to_be_bytes()).unwrap();
                
                // 初始化根目录元数据 (Inode 1)
                let root_meta = crate::models::InodeMetadata {
                    ino: 1,
                    size: 0,
                    mode: 0o040755, 
                    nlink: 2,
                    extents: alloc::vec::Vec::new(),
                    atime: 0,
                    mtime: 0,
                };
                let bucket = tx.get_bucket("inodes").unwrap();
                let meta_data = serde_json::to_vec(&root_meta).unwrap();
                bucket.put(1u64.to_be_bytes(), meta_data).unwrap();
                
                // 创建根目录的目录项 bucket
                let root_dir = tx.create_bucket("dir_1").map_err(|_| VfsError::IoError)?;
                root_dir.put(".", 1u64.to_be_bytes()).unwrap();
                root_dir.put("..", 1u64.to_be_bytes()).unwrap();
            }
            tx.commit().map_err(|_| VfsError::IoError)?;
        }

        let engine = Arc::new(Mutex::new(TransactionEngine::new(db, log_manager)));
        
        // 使用 Arc::new_cyclic 处理自引用弱指针
        let sb = Arc::new_cyclic(|weak| DbfsSuperBlock {
            engine: engine.clone(),
            self_weak: weak.clone(),
        });
        
        let root_inode = Arc::new(DbfsInode {
            ino: 1,
            engine,
            sb: Arc::downgrade(&sb),
        });

        Ok(DbfsDentry::new(root_inode, Weak::new(), "/".to_string()) as Arc<dyn VfsDentry>)
    }

    fn kill_sb(&self, sb: Arc<dyn VfsSuperBlock>) -> VfsResult<()> {
        sb.sync_fs(true)?;
        Ok(())
    }

    fn fs_flag(&self) -> vfscore::fstype::FileSystemFlags {
        vfscore::fstype::FileSystemFlags::REQUIRES_DEV
    }

    fn fs_name(&self) -> String {
        "dbfs".to_string()
    }
}

/// 适配 rvfs 的 Inode 实现
pub struct DbfsInode<D: BlockDevice> {
    pub ino: u64,
    pub engine: Arc<Mutex<TransactionEngine<D>>>,
    pub sb: Weak<DbfsSuperBlock<D>>,
}

impl<D: BlockDevice + 'static> VfsFile for DbfsInode<D> {
    /// 翻译 rvfs 的写操作
    fn write_at(&self, offset: u64, buf: &[u8]) -> VfsResult<usize> {
        let mut engine = self.engine.lock();
        
        engine.write_file_transactional(self.ino, offset, buf)
            .map_err(|_| vfscore::VfsError::IoError)?;
            
        Ok(buf.len())
    }

    /// 翻译 rvfs 的读操作
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> VfsResult<usize> {
        let engine = self.engine.lock();
        
        engine.read_file(self.ino, offset, buf)
            .map_err(|_| vfscore::VfsError::IoError)
    }

    /// 读取目录项
    fn readdir(&self, start_index: usize) -> VfsResult<Option<VfsDirEntry>> {
        let engine = self.engine.lock();
        
        // 1. 获取当前 Inode 元数据确认是目录
        let meta = engine.get_metadata(self.ino)
            .map_err(|_| VfsError::IoError)?;
            
        if (meta.mode & 0o170000) != 0o040000 {
            return Err(VfsError::NotDir);
        }

        // 2. 调用 tx_engine 获取目录项
        let entry = engine.list_dentries(self.ino, start_index)
            .map_err(|_| VfsError::IoError)?;
            
        if let Some((name, ino)) = entry {
            // 获取子节点元数据以确定类型
            let child_meta = engine.get_metadata(ino)
                .map_err(|_| VfsError::IoError)?;
            
            let ty = if (child_meta.mode & 0o170000) == 0o040000 {
                VfsNodeType::Dir
            } else {
                VfsNodeType::File
            };
            
            Ok(Some(VfsDirEntry {
                ino,
                ty,
                name,
            }))
        } else {
            Ok(None)
        }
    }

    fn fsync(&self) -> VfsResult<()> {
        // DBFS-T 的写操作已经是事务性的，每次 write_at 都会 commit
        Ok(())
    }

    fn flush(&self) -> VfsResult<()> {
        Ok(())
    }
}

impl<D: BlockDevice + 'static> VfsInode for DbfsInode<D> {
    fn get_super_block(&self) -> VfsResult<Arc<dyn VfsSuperBlock>> {
        self.sb.upgrade().map(|sb| sb as Arc<dyn VfsSuperBlock>).ok_or(VfsError::Invalid)
    }

    fn node_perm(&self) -> VfsNodePerm {
        let engine = self.engine.lock();
        match engine.get_metadata(self.ino) {
            Ok(meta) => VfsNodePerm::from_bits_truncate((meta.mode & 0o777) as u16),
            Err(_) => VfsNodePerm::empty(),
        }
    }

    fn get_attr(&self) -> VfsResult<VfsFileStat> {
        let engine = self.engine.lock();
        let meta = engine.get_metadata(self.ino)
            .map_err(|_| vfscore::VfsError::IoError)?;
        
        let mut attr = VfsFileStat::default();
        attr.st_size = meta.size;
        attr.st_ino = self.ino;
        attr.st_mode = meta.mode;
        attr.st_nlink = meta.nlink;
        attr.st_uid = 0;
        attr.st_gid = 0;
        attr.st_atime = VfsTimeSpec::new(meta.atime, 0);
        attr.st_mtime = VfsTimeSpec::new(meta.mtime, 0);
        attr.st_ctime = VfsTimeSpec::new(meta.mtime, 0); // 暂用 mtime
        
        Ok(attr)
    }

    fn set_attr(&self, attr: InodeAttr) -> VfsResult<()> {
        let mut engine = self.engine.lock();
        let mut meta = engine.get_metadata(self.ino)
            .map_err(|_| VfsError::IoError)?;
            
        meta.mode = attr.mode;
        meta.size = attr.size;
        meta.atime = attr.atime.tv_sec;
        meta.mtime = attr.mtime.tv_sec;
        
        engine.update_metadata(&meta)
            .map_err(|_| VfsError::IoError)?;
        Ok(())
    }

    fn inode_type(&self) -> VfsNodeType {
        let engine = self.engine.lock();
        let meta = match engine.get_metadata(self.ino) {
            Ok(m) => m,
            Err(_) => return VfsNodeType::Unknown,
        };
        
        if (meta.mode & 0o170000) == 0o040000 {
            VfsNodeType::Dir
        } else {
            VfsNodeType::File
        }
    }

    fn create(&self, name: &str, _ty: VfsNodeType, perm: VfsNodePerm, _rdev: Option<u64>) -> VfsResult<Arc<dyn VfsInode>> {
        let mut engine = self.engine.lock();
        
        // 1. 分配新的 Inode 号 (普通文件)
        let mode = 0o100000 | (perm.bits() as u32);
        let new_ino = engine.allocate_inode(mode)
            .map_err(|_| VfsError::IoError)?;
            
        // 2. 在数据库中创建目录项
        engine.add_dentry(self.ino, name, new_ino)
            .map_err(|_| VfsError::IoError)?;
            
        Ok(Arc::new(DbfsInode {
            ino: new_ino,
            engine: self.engine.clone(),
            sb: self.sb.clone(),
        }))
    }

    fn mkdir(&self, name: &str, perm: VfsNodePerm) -> VfsResult<Arc<dyn VfsInode>> {
        let mut engine = self.engine.lock();
        
        let mode = 0o040000 | (perm.bits() as u32);
        let new_ino = engine.allocate_inode(mode)
            .map_err(|_| VfsError::IoError)?;
            
        engine.add_dentry(self.ino, name, new_ino)
            .map_err(|_| VfsError::IoError)?;
            
        Ok(Arc::new(DbfsInode {
            ino: new_ino,
            engine: self.engine.clone(),
            sb: self.sb.clone(),
        }))
    }

    fn lookup(&self, name: &str) -> VfsResult<Arc<dyn VfsInode>> {
        let engine = self.engine.lock();
        let ino = engine.lookup_dentry(self.ino, name)
            .map_err(|_| VfsError::NoEntry)?;
            
        Ok(Arc::new(DbfsInode {
            ino,
            engine: self.engine.clone(),
            sb: self.sb.clone(),
        }))
    }

    fn unlink(&self, name: &str) -> VfsResult<()> {
        let mut engine = self.engine.lock();
        
        // 1. 查找子节点 Inode
        let child_ino = engine.lookup_dentry(self.ino, name)
            .map_err(|_| VfsError::NoEntry)?;
            
        // 2. 删除目录项
        engine.delete_dentry(self.ino, name)
            .map_err(|_| VfsError::IoError)?;
            
        // 3. 更新子节点 nlink
        let mut child_meta = engine.get_metadata(child_ino)
            .map_err(|_| VfsError::IoError)?;
        
        if child_meta.nlink > 0 {
            child_meta.nlink -= 1;
        }
        
        if child_meta.nlink == 0 {
            // 如果链接数为 0，删除 Inode (简单处理，实际可能需要延迟删除)
            engine.delete_inode(child_ino)
                .map_err(|_| VfsError::IoError)?;
        } else {
            engine.update_metadata(&child_meta)
                .map_err(|_| VfsError::IoError)?;
        }
        
        Ok(())
    }

    fn rmdir(&self, name: &str) -> VfsResult<()> {
        let mut engine = self.engine.lock();
        
        // 1. 查找子节点
        let child_ino = engine.lookup_dentry(self.ino, name)
            .map_err(|_| VfsError::NoEntry)?;
            
        // 2. 检查是否为目录
        let child_meta = engine.get_metadata(child_ino)
            .map_err(|_| VfsError::IoError)?;
        if (child_meta.mode & 0o170000) != 0o040000 {
            return Err(VfsError::NotDir);
        }
        
        // 3. 检查目录是否为空
        if engine.list_dentries(child_ino, 0).map_err(|_| VfsError::IoError)?.is_some() {
            return Err(VfsError::NotEmpty);
        }
        
        // 4. 删除目录项和 Inode
        engine.delete_dentry(self.ino, name)
            .map_err(|_| VfsError::IoError)?;
        engine.delete_inode(child_ino)
            .map_err(|_| VfsError::IoError)?;
            
        Ok(())
    }

    fn truncate(&self, len: u64) -> VfsResult<()> {
        let mut engine = self.engine.lock();
        engine.truncate_file(self.ino, len)
            .map_err(|_| VfsError::IoError)?;
        Ok(())
    }

    fn rename_to(&self, old_name: &str, new_parent: Arc<dyn VfsInode>, new_name: &str, _flag: vfscore::utils::VfsRenameFlag) -> VfsResult<()> {
        let mut engine = self.engine.lock();
        
        // 1. 查找旧节点
        let ino = engine.lookup_dentry(self.ino, old_name)
            .map_err(|_| VfsError::NoEntry)?;
            
        // 2. 获取新父节点的 Inode (假定它是 DbfsInode)
        let new_parent_dbfs = new_parent.downcast_ref::<DbfsInode<D>>()
            .ok_or(VfsError::Invalid)?;
            
        // 3. 在新位置添加目录项
        engine.add_dentry(new_parent_dbfs.ino, new_name, ino)
            .map_err(|_| VfsError::IoError)?;
            
        // 4. 删除旧位置目录项
        engine.delete_dentry(self.ino, old_name)
            .map_err(|_| VfsError::IoError)?;
            
        Ok(())
    }
}


/// 适配 rvfs 的超级块实现
pub struct DbfsSuperBlock<D: BlockDevice> {
    pub engine: Arc<Mutex<TransactionEngine<D>>>,
    pub self_weak: Weak<DbfsSuperBlock<D>>,
}

impl<D: BlockDevice + 'static> VfsSuperBlock for DbfsSuperBlock<D> {
    fn root_inode(&self) -> VfsResult<Arc<dyn VfsInode>> {
        Ok(Arc::new(DbfsInode {
            ino: 1, // 根目录约定为 1
            engine: self.engine.clone(),
            sb: self.self_weak.clone(),
        }))
    }

    fn sync_fs(&self, _wait: bool) -> VfsResult<()> {
        // DBFS-T 的事务在每次写入时已提交，但这里可以触发磁盘屏障
        Ok(())
    }

    fn stat_fs(&self) -> VfsResult<VfsFsStat> {
        Ok(VfsFsStat {
            f_type: 0x44424653, // "DBFS"
            f_bsize: 4096,
            f_blocks: 0, // TODO
            f_bfree: 0,
            f_bavail: 0,
            f_files: 0,
            f_ffree: 0,
            f_fsid: [0, 0],
            f_namelen: 255,
            f_frsize: 4096,
            f_flags: 0,
            f_spare: [0; 4],
        })
    }

    fn super_type(&self) -> SuperType {
        SuperType::BlockDev
    }

    fn fs_type(&self) -> Arc<dyn VfsFsType> {
        Arc::new(DbfsFsType)
    }
}
