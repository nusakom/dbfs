use crate::common::DbfsResult;

pub trait BlockDevice: Send + Sync {
    fn read_at(&self, pos: u64, buf: &mut [u8]) -> DbfsResult<usize>;
    fn write_at(&self, pos: u64, buf: &[u8]) -> DbfsResult<usize>;
    fn size(&self) -> u64;
}

pub struct LogManager<D: BlockDevice> {
    device: D,
    next_append_pos: u64, // 下一个追加位置
}

impl<D: BlockDevice> LogManager<D> {
    pub fn new(device: D, next_append_pos: u64) -> Self {
        Self {
            device,
            next_append_pos,
        }
    }

    /// 核心操作：追加数据并返回物理偏移
    pub fn append_data(&mut self, data: &[u8]) -> DbfsResult<u64> {
        let current_pos = self.next_append_pos;
        
        // 1. 计算校验和
        let _checksum = crc32(data);
        
        // 2. 写入数据负载到磁盘
        self.device.write_at(current_pos, data)?;
        
        // 3. 更新指针
        self.next_append_pos += data.len() as u64;
        
        Ok(current_pos)
    }

    pub fn next_append_pos(&self) -> u64 {
        self.next_append_pos
    }

    /// 从指定物理位置读取数据
    pub fn read_data(&self, pos: u64, buf: &mut [u8]) -> DbfsResult<usize> {
        self.device.read_at(pos, buf)
    }
}

/// 简单的 CRC32 实现
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFFFFFFu32;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB88320;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}
