use crate::models::{InodeMetadata, Extent};
use crate::log_manager::{LogManager, BlockDevice, crc32};
use crate::common::{DbfsResult, DbfsError};
use jammdb::DB;
use alloc::vec::Vec;

pub struct TransactionEngine<D: BlockDevice> {
    db: DB,
    log_manager: LogManager<D>,
}

impl<D: BlockDevice> TransactionEngine<D> {
    pub fn new(db: DB, log_manager: LogManager<D>) -> Self {
        Self { db, log_manager }
    }

    pub fn write_file_transactional(&mut self, ino: u64, offset: u64, data: &[u8]) -> DbfsResult<()> {
        // --- 步骤 1: 数据持久化 (数据层先走) ---
        // 即使这一步写完后断电，因为没有索引，数据在重启后是“不可见”的。
        let p_ptr = self.log_manager.append_data(data)?;

        // --- 步骤 2: 开启数据库事务 (索引层后跟) ---
        let tx = self.db.begin_batch();
        let bucket = tx.get_bucket("inodes").map_err(|_| DbfsError::NotFound)?;

        // --- 步骤 3: 读取-修改-写回 (Read-Modify-Write) ---
        let ino_key = ino.to_be_bytes();
        let kv = bucket.get(&ino_key).ok_or(DbfsError::NotFound)?;
        
        let mut meta: InodeMetadata = deserialize(kv.kv().value())?;
        
        // 增加新的映射关系
        meta.extents.push(Extent {
            logical_off: offset,
            physical_ptr: p_ptr,
            len: data.len() as u64,
            crc: crc32(data),
        });
        meta.size = core::cmp::max(meta.size, offset + data.len() as u64);
        // meta.mtime = now(); // TODO: 实现获取当前时间的逻辑

        // 将新的元数据覆盖写入数据库
        bucket.put(ino_key, serialize(&meta)?)?;

        // --- 故障注入测试点 ---
        // if cfg!(feature = "crash_test") { panic!("Simulated Crash before commit!"); }

        // --- 步骤 4: 原子提交 (The Commit) ---
        // 这是唯一的故障切换点。jammdb 保证此操作要么全成功，要么全失败。
        tx.commit().map_err(|_| DbfsError::Io)?;

        Ok(())
    }

    /// 从文件中读取数据
    pub fn read_file(&self, ino: u64, offset: u64, buf: &mut [u8]) -> DbfsResult<usize> {
        let tx = self.db.tx(false).map_err(|_| DbfsError::Io)?;
        let bucket = tx.get_bucket("inodes").map_err(|_| DbfsError::NotFound)?;
        
        let ino_key = ino.to_be_bytes();
        let kv = bucket.get(&ino_key).ok_or(DbfsError::NotFound)?;
        let meta: InodeMetadata = deserialize(kv.kv().value())?;
        
        if offset >= meta.size {
            return Ok(0);
        }
        
        let read_len = core::cmp::min(buf.len() as u64, meta.size - offset) as usize;
        let mut total_read = 0;
        
        // 遍历 extents 找到对应数据
        // 注意：这是一个简单实现，实际应按 offset 排序或使用更高效的索引
        for extent in &meta.extents {
            if total_read >= read_len {
                break;
            }
            
            // 检查 extent 是否与请求范围重叠
            let extent_end = extent.logical_off + extent.len;
            let request_end = offset + read_len as u64;
            
            if extent.logical_off < request_end && extent_end > offset {
                let overlap_start = core::cmp::max(extent.logical_off, offset);
                let overlap_end = core::cmp::min(extent_end, request_end);
                
                let extent_offset = overlap_start - extent.logical_off;
                let buf_offset = overlap_start - offset;
                let copy_len = (overlap_end - overlap_start) as usize;
                
                let mut temp_buf = alloc::vec![0u8; copy_len];
                self.log_manager.read_data(extent.physical_ptr + extent_offset, &mut temp_buf)?;
                
                buf[buf_offset as usize..(buf_offset + copy_len as u64) as usize].copy_from_slice(&temp_buf);
                total_read = core::cmp::max(total_read, (buf_offset + copy_len as u64) as usize);
            }
        }
        
        Ok(total_read)
    }

    /// 分配新的 Inode 号
    pub fn allocate_inode(&mut self, mode: u32) -> DbfsResult<u64> {
        let tx = self.db.begin_batch();
        let bucket = tx.get_bucket("inodes").map_err(|_| DbfsError::NotFound)?;
        
        // 简单实现：查找当前最大的 Inode 号并 +1
        let mut max_ino = 1u64;
        for kv in bucket.cursor() {
            let ino = u64::from_be_bytes(kv.key().try_into().map_err(|_| DbfsError::Other)?);
            if ino > max_ino {
                max_ino = ino;
            }
        }
        let new_ino = max_ino + 1;
        
        // 初始化新 Inode 元数据
        let meta = InodeMetadata {
            ino: new_ino,
            size: 0,
            mode,
            nlink: 1,
            extents: alloc::vec::Vec::new(),
            atime: 0,
            mtime: 0,
        };
        bucket.put(new_ino.to_be_bytes(), serialize(&meta)?)?;
        
        tx.commit().map_err(|_| DbfsError::Io)?;
        Ok(new_ino)
    }

    /// 添加目录项
    pub fn add_dentry(&mut self, parent_ino: u64, name: &str, child_ino: u64) -> DbfsResult<()> {
        let tx = self.db.begin_batch();
        let bucket_name = alloc::format!("dir_{}", parent_ino);
        let bucket = tx.get_or_create_bucket(&bucket_name).map_err(|_| DbfsError::Io)?;
        
        bucket.put(name.as_bytes(), child_ino.to_be_bytes())?;
        
        tx.commit().map_err(|_| DbfsError::Io)?;
        Ok(())
    }

    /// 查找目录项
    pub fn lookup_dentry(&self, parent_ino: u64, name: &str) -> DbfsResult<u64> {
        let tx = self.db.tx(false).map_err(|_| DbfsError::Io)?;
        let bucket_name = alloc::format!("dir_{}", parent_ino);
        let bucket = tx.get_bucket(&bucket_name).map_err(|_| DbfsError::NotFound)?;
        
        let val = bucket.get(name.as_bytes()).ok_or(DbfsError::NotFound)?;
        let ino = u64::from_be_bytes(val.kv().value().try_into().map_err(|_| DbfsError::Other)?);
        
        Ok(ino)
    }

    /// 列出目录项
    pub fn list_dentries(&self, parent_ino: u64, start_index: usize) -> DbfsResult<Option<(alloc::string::String, u64)>> {
        let tx = self.db.tx(false).map_err(|_| DbfsError::Io)?;
        let bucket_name = alloc::format!("dir_{}", parent_ino);
        let bucket = match tx.get_bucket(&bucket_name) {
            Ok(b) => b,
            Err(_) => return Ok(None),
        };

        let entry = bucket.cursor().nth(start_index);
        if let Some(kv) = entry {
            let name = alloc::string::String::from_utf8(kv.key().to_vec()).map_err(|_| DbfsError::Other)?;
            let ino = u64::from_be_bytes(kv.kv().value().try_into().map_err(|_| DbfsError::Other)?);
            Ok(Some((name, ino)))
        } else {
            Ok(None)
        }
    }

    /// 删除目录项
    pub fn delete_dentry(&mut self, parent_ino: u64, name: &str) -> DbfsResult<()> {
        let tx = self.db.begin_batch();
        let bucket_name = alloc::format!("dir_{}", parent_ino);
        let bucket = tx.get_bucket(&bucket_name).map_err(|_| DbfsError::NotFound)?;
        
        bucket.delete(name.as_bytes()).map_err(|_| DbfsError::Io)?;
        
        tx.commit().map_err(|_| DbfsError::Io)?;
        Ok(())
    }

    /// 删除 Inode
    pub fn delete_inode(&mut self, ino: u64) -> DbfsResult<()> {
        let tx = self.db.begin_batch();
        let bucket = tx.get_bucket("inodes").map_err(|_| DbfsError::NotFound)?;
        
        bucket.delete(&ino.to_be_bytes()).map_err(|_| DbfsError::Io)?;
        
        // 如果是目录，删除其目录项 bucket
        let _ = tx.delete_bucket(&alloc::format!("dir_{}", ino));
        
        tx.commit().map_err(|_| DbfsError::Io)?;
        Ok(())
    }

    /// 更新 Inode 元数据
    pub fn update_metadata(&mut self, meta: &InodeMetadata) -> DbfsResult<()> {
        let tx = self.db.begin_batch();
        let bucket = tx.get_bucket("inodes").map_err(|_| DbfsError::NotFound)?;
        
        bucket.put(meta.ino.to_be_bytes(), serialize(meta)?)?;
        
        tx.commit().map_err(|_| DbfsError::Io)?;
        Ok(())
    }

    /// 获取 Inode 元数据
    pub fn get_metadata(&self, ino: u64) -> DbfsResult<InodeMetadata> {
        let tx = self.db.tx(false).map_err(|_| DbfsError::Io)?;
        let bucket = tx.get_bucket("inodes").map_err(|_| DbfsError::NotFound)?;
        let kv = bucket.get(&ino.to_be_bytes()).ok_or(DbfsError::NotFound)?;
        deserialize(kv.kv().value())
    }

    /// 截断文件
    pub fn truncate_file(&mut self, ino: u64, new_size: u64) -> DbfsResult<()> {
        let tx = self.db.begin_batch();
        let bucket = tx.get_bucket("inodes").map_err(|_| DbfsError::NotFound)?;
        
        let ino_key = ino.to_be_bytes();
        let kv = bucket.get(&ino_key).ok_or(DbfsError::NotFound)?;
        let mut meta: InodeMetadata = deserialize(kv.kv().value())?;
        
        if new_size < meta.size {
            // 缩小文件：保留逻辑偏移量小于 new_size 的 extents
            // 注意：这里需要处理跨越 new_size 边界的 extent
            let mut new_extents = Vec::new();
            for mut extent in meta.extents {
                if extent.logical_off < new_size {
                    if extent.logical_off + extent.len > new_size {
                        // 截断最后一个 extent
                        extent.len = new_size - extent.logical_off;
                        // 注意：crc 可能失效，或者我们选择不更新 crc (因为 read 时会校验)
                        // 实际实现中，截断后的部分数据可能依然在磁盘上，只是索引变了
                    }
                    new_extents.push(extent);
                } else {
                    // 逻辑偏移量 >= new_size 的 extent 直接丢弃
                    // TODO: 在物理日志中标记这些空间可以回收 (DBFS-T 是追加写，暂不回收)
                }
            }
            meta.extents = new_extents;
        }
        
        meta.size = new_size;
        // meta.mtime = now();

        bucket.put(ino_key, serialize(&meta)?)?;
        tx.commit().map_err(|_| DbfsError::Io)?;
        Ok(())
    }

    /// 根据 Extents 从日志读取数据
    pub fn read_from_log(&self, meta: &InodeMetadata, offset: u64, buf: &mut [u8]) -> DbfsResult<usize> {
        if offset >= meta.size {
            return Ok(0);
        }

        let mut bytes_read = 0;
        let mut current_offset = offset;
        let mut buf_pos = 0;

        while buf_pos < buf.len() && current_offset < meta.size {
            // 查找包含 current_offset 的 extent
            let extent = meta.extents.iter().find(|e| {
                current_offset >= e.logical_off && current_offset < e.logical_off + e.len
            });

            if let Some(e) = extent {
                let off_in_extent = current_offset - e.logical_off;
                let len_in_extent = core::cmp::min(
                    (e.len - off_in_extent) as usize,
                    buf.len() - buf_pos
                );

                let physical_pos = e.physical_ptr + off_in_extent;
                self.log_manager.read_data(physical_pos, &mut buf[buf_pos..buf_pos + len_in_extent])?;

                bytes_read += len_in_extent;
                buf_pos += len_in_extent;
                current_offset += len_in_extent as u64;
            } else {
                // 如果没找到 extent，说明是空洞 (hole)
                // TODO: 寻找下一个 extent 的开始位置，中间补 0
                break;
            }
        }

        Ok(bytes_read)
    }
}

// 序列化辅助函数 (暂用 serde_json，后续可替换为更高效的 postcard 等)
fn serialize<T: serde::Serialize>(obj: &T) -> DbfsResult<Vec<u8>> {
    serde_json::to_vec(obj).map_err(|_| DbfsError::Other)
}

// 反序列化辅助函数
fn deserialize<'a, T: serde::Deserialize<'a>>(data: &'a [u8]) -> DbfsResult<T> {
    serde_json::from_slice(data).map_err(|_| DbfsError::Other)
}
