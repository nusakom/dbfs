use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::Mutex;
use vfscore::{VfsInode, VfsNodeType, VfsResult, VfsFile, VfsDentry};
use vfscore::utils::{VfsFileStat, VfsNodePerm, VfsTimeSpec};
use crate::rvfs_adapter::{DbfsFsType, DbfsInode};
use vfscore::fstype::VfsFsType;

/// 模拟块设备
pub struct RamDisk {
    data: Mutex<Vec<u8>>,
}

impl RamDisk {
    pub fn new(size: usize) -> Self {
        Self {
            data: Mutex::new(alloc::vec![0u8; size]),
        }
    }
}

impl VfsFile for RamDisk {
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> VfsResult<usize> {
        let data = self.data.lock();
        if offset >= data.len() as u64 {
            return Ok(0);
        }
        let end = core::cmp::min(offset as usize + buf.len(), data.len());
        let len = end - offset as usize;
        buf[..len].copy_from_slice(&data[offset as usize..end]);
        Ok(len)
    }

    fn write_at(&self, offset: u64, buf: &[u8]) -> VfsResult<usize> {
        let mut data = self.data.lock();
        if offset >= data.len() as u64 {
            return Ok(0);
        }
        let end = core::cmp::min(offset as usize + buf.len(), data.len());
        let len = end - offset as usize;
        data[offset as usize..end].copy_from_slice(&buf[..len]);
        Ok(len)
    }
}

impl VfsInode for RamDisk {
    fn get_attr(&self) -> VfsResult<VfsFileStat> {
        let mut attr = VfsFileStat::default();
        attr.st_size = self.data.lock().len() as u64;
        attr.st_mode = 0o600; // Block device
        Ok(attr)
    }

    fn inode_type(&self) -> VfsNodeType {
        VfsNodeType::BlockDevice
    }
    
    fn get_super_block(&self) -> VfsResult<Arc<dyn vfscore::VfsSuperBlock>> {
        Err(vfscore::VfsError::Invalid)
    }
    fn node_perm(&self) -> VfsNodePerm { VfsNodePerm::all() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    #[test]
    fn test_dbfs_vfs_basic() {
        let ram_disk = Arc::new(RamDisk::new(64 * 1024 * 1024)); // 64MB
        let fs_type = Arc::new(DbfsFsType);
        
        // 1. 挂载
        let root_dentry = fs_type.mount(0, "/", Some(ram_disk as Arc<dyn VfsInode>), &[]).expect("Mount failed");
        let root_inode = root_dentry.inode().expect("Get root inode failed");
        
        // 2. 创建目录
        root_inode.mkdir("test_dir", VfsNodePerm::from_bits_truncate(0o755)).expect("Mkdir failed");
        
        // 3. 查找目录
        let dir_inode = root_inode.lookup("test_dir").expect("Lookup dir failed");
        assert_eq!(dir_inode.inode_type(), VfsNodeType::Dir);
        
        // 4. 在目录中创建文件
        dir_inode.create("test_file.txt", VfsNodeType::File, VfsNodePerm::from_bits_truncate(0o644), None).expect("Create file failed");
        
        // 5. 查找文件并写入
        let file_inode = dir_inode.lookup("test_file.txt").expect("Lookup file failed");
        let test_data = b"Hello DBFS-T via VFS!";
        file_inode.write_at(0, test_data).expect("Write file failed");
        
        // 6. 读取文件并验证
        let mut read_buf = [0u8; 32];
        let n = file_inode.read_at(0, &mut read_buf).expect("Read file failed");
        assert_eq!(n, test_data.len());
        assert_eq!(&read_buf[..n], test_data);
        
        // 7. 遍历目录 (readdir)
        let mut entries = Vec::new();
        let mut idx = 0;
        while let Some(entry) = dir_inode.readdir(idx).expect("Readdir failed") {
            entries.push(entry.name);
            idx += 1;
        }
        // 注意：我们的 readdir 实现中可能包含 . 和 ..，也可能不包含，取决于具体实现
        // 在 mount 初始化时我们添加了 . 和 ..
        assert!(entries.contains(&".".to_string()));
        assert!(entries.contains(&"..".to_string()));
        assert!(entries.contains(&"test_file.txt".to_string()));
        
        // 8. 测试截断
        file_inode.truncate(5).expect("Truncate failed");
        assert_eq!(file_inode.get_attr().unwrap().st_size, 5);
        let mut read_buf_small = [0u8; 10];
        let n = file_inode.read_at(0, &mut read_buf_small).expect("Read truncated file failed");
        assert_eq!(n, 5);
        assert_eq!(&read_buf_small[..n], b"Hello");
    }
}
