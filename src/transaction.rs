use crate::wal::WriteAheadLog;
use alloc::vec::Vec;
use alloc::string::String;
use alloc::sync::Arc;
use spin::{Mutex, RwLock};

pub struct Transaction {
    pub id: u64,
    pub ops: Vec<TransactionOperation>,
}

impl Transaction {
    pub fn new(id: u64) -> Self {
        Self {
            id,
            ops: Vec::new(),
        }
    }

    pub fn record(&mut self, op: TransactionOperation) {
        self.ops.push(op);
    }
}

pub struct TransactionManager {
    wal: Mutex<WriteAheadLog>,
    next_txn_id: Mutex<u64>,
    /// Ensures that while operations are being applied, no one is reading.
    pub state_lock: RwLock<()>,
}

impl TransactionManager {
    pub fn new() -> Self {
        Self {
            wal: Mutex::new(WriteAheadLog::new()),
            next_txn_id: Mutex::new(1),
            state_lock: RwLock::new(()),
        }
    }

    pub fn begin_transaction(&self) -> Transaction {
        let mut id_gen = self.next_txn_id.lock();
        let id = *id_gen;
        *id_gen += 1;
        Transaction::new(id)
    }

    pub fn set_wal_storage(&self, storage: Arc<dyn crate::wal::WalStorage>) {
        self.wal.lock().set_storage(storage);
    }
}

    /// Simulates a crash scenario: writes to WAL but does not apply ops.
    pub fn commit_into_wal_only(&self, txn: Transaction) -> Result<(), String> {
        let mut wal = self.wal.lock();
        for op in &txn.ops {
            wal.append(txn.id, op.clone())?;
        }
        wal.flush()?;
        Ok(())
    }

    pub fn commit(&self, txn: Transaction) -> Result<(), String> {
        let mut wal = self.wal.lock();
        
        // 1. Write all ops to WAL
        for op in &txn.ops {
            wal.append(txn.id, op.clone())?;
        }
        
        // 2. Flush WAL (ensures durability)
        wal.flush()?;
        
        // --- Atomic Point ---
        // 3. Acquire exclusive lock before applying operations to Bottom FS
        let _guard = self.state_lock.write();
        
        // 4. Apply operations to Bottom FS (Deferred Execution)
        for op in txn.ops {
            op.apply()?;
        }
        
        // 5. Clear from WAL (Checkpoint)
        wal.clear_txn(txn.id);
        
        Ok(())
    }

    pub fn replay(&self) -> Result<(), String> {
        let mut wal = self.wal.lock();
        let entries = wal.recover()?;
        
        log::info!("Replaying {} transactional operations from WAL", entries.len());
        
        for entry in entries {
            entry.operation.apply()?;
        }
        
        // After replaying all entries, we can clear the memory log
        // Note: The physical log persists until we decide to truncate/checkpoint.
        // For this simple version, we assume replay is equivalent to a checkpoint.
        
        Ok(())
    }

    pub fn rollback(&self, _txn: Transaction) {
        // For deferred execution, rollback is just dropping the transaction
        // as no changes have been applied to the Bottom FS yet.
    }
}
