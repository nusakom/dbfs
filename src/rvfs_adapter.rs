use crate::tx_engine::TransactionEngine;
use crate::log_manager::BlockDevice;
use vfscore::{InodeOps, VfsResult, VfsInode, InodeType, SuperBlockOps, VfsFsType, VfsDentry, VfsError, VfsNodeType};
use alloc::sync::Arc;
use spin::Mutex;
use crate::common::DbfsResult;
use jammdb::DB;
use crate::log_manager::LogManager;

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
    ) -> VfsResult<Arc<VfsDentry>> {
        let dev = dev.ok_or(VfsError::Invalid)?;
        if dev.inode_type() != VfsNodeType::BlockDevice {
            return Err(VfsError::Invalid);
        }

        // 1. 初始化数据库和日志管理器
        // 注意：这里需要一个临时的 jammdb 初始化逻辑
        // 在实际内核中，我们可能需要更复杂的机制来管理 DB 文件的位置
        // 暂且假设我们直接在块设备的前部运行 jammdb，或者有其他协议
        // 这里为了演示，我们先创建一个内存 DB 或 模拟初始化
        let adapter = VfsBlockDeviceAdapter { inode: dev.clone() };
        
        // TODO: 真正的 jammdb 需要一个文件系统环境或内存环境
        // 这里我们先假设 init_dbfs 已经在某处被调用，或者我们在这里创建
        // 实际上 jammdb 需要底层的 Page 控制，这里可能需要进一步适配
        
        // 模拟创建 TransactionEngine
        // let db = DB::open_in_memory().unwrap(); // 仅作演示
        // let log_manager = LogManager::new(adapter, 0);
        // let engine = Arc::new(Mutex::new(TransactionEngine::new(db, log_manager)));
        
        // let sb = Arc::new(DbfsSuperBlock { engine });
        // Ok(VfsDentry::new(sb.root_inode(), None))
        
        Err(VfsError::NoSys) // 暂未完全打通 jammdb 与 VfsInode 的直接 Page 映射
    }
}

/// 适配 rvfs 的 Inode 实现
pub struct DbfsInode<D: BlockDevice> {
    pub ino: u64,
    pub engine: Arc<Mutex<TransactionEngine<D>>>,
}

impl<D: BlockDevice + 'static> InodeOps for DbfsInode<D> {
    /// 翻译 rvfs 的写操作
    fn write_at(&self, offset: usize, buf: &[u8]) -> VfsResult<usize> {
        let mut engine = self.engine.lock();
        
        engine.write_file_transactional(self.ino, offset as u64, buf)
            .map_err(|_| vfscore::VfsError::IoError)?;
            
        Ok(buf.len())
    }

    /// 翻译 rvfs 的读操作
    fn read_at(&self, offset: usize, buf: &mut [u8]) -> VfsResult<usize> {
        let engine = self.engine.lock();
        
        // 1. 获取元数据
        let meta = engine.get_metadata(self.ino)
            .map_err(|_| vfscore::VfsError::IoError)?;
        
        // 2. 从日志中读取数据
        let bytes_read = engine.read_from_log(&meta, offset as u64, buf)
            .map_err(|_| vfscore::VfsError::IoError)?;
        
        Ok(bytes_read)
    }

    fn get_attr(&self) -> VfsResult<vfscore::VfsAttr> {
        let engine = self.engine.lock();
        let meta = engine.get_metadata(self.ino)
            .map_err(|_| vfscore::VfsError::IoError)?;
        
        let mut attr = vfscore::VfsAttr::default();
        attr.st_size = meta.size;
        attr.st_ino = self.ino;
        // 这里的类型转换需要根据 InodeType 映射，暂设为 RegularFile
        attr.st_mode = 0o644; 
        
        Ok(attr)
    }
}

/// 适配 rvfs 的超级块实现
pub struct DbfsSuperBlock<D: BlockDevice> {
    pub engine: Arc<Mutex<TransactionEngine<D>>>,
}

impl<D: BlockDevice + 'static> SuperBlockOps for DbfsSuperBlock<D> {
    fn root_inode(&self) -> VfsInode {
        VfsInode::new(
            Arc::new(DbfsInode {
                ino: 1, // 根目录约定为 1
                engine: self.engine.clone(),
            }),
            InodeType::Directory,
        )
    }

    fn sync(&self) -> VfsResult<()> {
        // DBFS-T 的事务在每次写入时已提交，但这里可以触发磁盘屏障
        Ok(())
    }
}
