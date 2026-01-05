use crate::operation::TransactionOperation;
use alloc::vec::Vec;
use alloc::string::String;
use alloc::sync::Arc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct WalEntry {
    pub txn_id: u64,
    pub operation: TransactionOperation,
}

pub trait WalStorage: Send + Sync {
    fn write(&self, offset: u64, data: &[u8]) -> Result<(), String>;
    fn read(&self, offset: u64, buf: &mut [u8]) -> Result<(), String>;
    fn truncate(&self, length: u64) -> Result<(), String>;
    fn flush(&self) -> Result<(), String>;
}

pub struct WriteAheadLog {
    entries: Vec<WalEntry>,
    storage: Option<Arc<dyn WalStorage>>,
    next_offset: u64,
}

impl WriteAheadLog {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            storage: None,
            next_offset: 0,
        }
    }

    pub fn set_storage(&mut self, storage: Arc<dyn WalStorage>) {
        self.storage = Some(storage);
    }

    pub fn append(&mut self, txn_id: u64, op: TransactionOperation) -> Result<(), String> {
        let entry = WalEntry { txn_id, operation: op };
        
        // Serialize
        let data = serde_json::to_vec(&entry).map_err(|e| e.to_string())?;
        
        // Format: [size: u32] [data: Vec<u8>]
        if let Some(ref storage) = self.storage {
            let size_bytes = (data.len() as u32).to_le_bytes();
            storage.write(self.next_offset, &size_bytes)?;
            self.next_offset += 4;

            storage.write(self.next_offset, &data)?;
            self.next_offset += data.len() as u64;
        }

        self.entries.push(entry);
        Ok(())
    }

    pub fn flush(&mut self) -> Result<(), String> {
        if let Some(ref storage) = self.storage {
            storage.flush()?;
        }
        Ok(())
    }

    pub fn recover(&mut self) -> Result<Vec<WalEntry>, String> {
        let mut recovered = Vec::new();
        let mut offset = 0;

        if let Some(ref storage) = self.storage {
            loop {
                let mut size_buf = [0u8; 4];
                // Try to read the size
                if let Ok(_) = storage.read(offset, &mut size_buf) {
                    let size = u32::from_le_bytes(size_buf);
                    if size == 0 || size > 1024 * 1024 { // Sanity check
                        break;
                    }
                    offset += 4;

                    let mut data = alloc::vec![0u8; size as usize];
                    if let Ok(_) = storage.read(offset, &mut data) {
                        if let Ok(entry) = serde_json::from_slice::<WalEntry>(&data) {
                            recovered.push(entry);
                        }
                        offset += size as u64;
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
        }
        self.next_offset = offset;
        Ok(recovered)
    }
    
    pub fn clear_txn(&mut self, txn_id: u64) {
        self.entries.retain(|e| e.txn_id != txn_id);
        
        // If all transactions are cleared, we can checkpoint
        if self.entries.is_empty() {
            if let Err(e) = self.checkpoint() {
                log::error!("WAL checkpoint failed: {}", e);
            }
        }
    }

    pub fn checkpoint(&mut self) -> Result<(), String> {
        if let Some(ref storage) = self.storage {
            storage.truncate(0)?;
            self.next_offset = 0;
        }
        Ok(())
    }
}
