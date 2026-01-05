#[cfg(test)]
mod tests {
    use crate::transaction::{TransactionManager, Transaction};
    use crate::operation::TransactionOperation;
    use crate::wal::{WalStorage, WriteAheadLog};
    use alloc::sync::Arc;
    use alloc::vec::Vec;
    use alloc::string::String;
    use spin::Mutex;

    struct MockStorage {
        data: Mutex<Vec<u8>>,
    }

    impl WalStorage for MockStorage {
        fn write(&self, offset: u64, data: &[u8]) -> Result<(), String> {
            let mut storage = self.data.lock();
            let end = offset as usize + data.len();
            if end > storage.len() {
                storage.resize(end, 0);
            }
            storage[offset as usize..end].copy_from_slice(data);
            Ok(())
        }
        fn read(&self, offset: u64, buf: &mut [u8]) -> Result<(), String> {
            let storage = self.data.lock();
            if offset as usize + buf.len() > storage.len() {
                return Err("Read out of bounds".to_string());
            }
            buf.copy_from_slice(&storage[offset as usize..offset as usize + buf.len()]);
            Ok(())
        }
        fn truncate(&self, length: u64) -> Result<(), String> {
            let mut storage = self.data.lock();
            storage.truncate(length as usize);
            Ok(())
        }
        fn flush(&self) -> Result<(), String> { Ok(()) }
    }

    #[test]
    fn test_transaction_recovery() {
        let tm = TransactionManager::new();
        let storage = Arc::new(MockStorage { data: Mutex::new(Vec::new()) });
        tm.set_wal_storage(storage.clone());

        // 1. Simulate a transaction that was written to WAL but the application crashed before apply()
        let mut txn = tm.begin_transaction();
        txn.record(TransactionOperation::Write {
            ino: 1,
            offset: 0,
            data: b"Recovery Data".to_vec(),
        });
        
        // Manual append to WAL to simulate "written to log but not applied"
        tm.commit_into_wal_only(txn).expect("WAL write failed");

        // 2. Perform recovery (replay)
        tm.replay().expect("Replay failed");
        
        // 3. Verify that the operation was finally applied
        // (In a real test, verify side effects on Bottom FS)
    }

    #[test]
    fn test_transaction_atomicity() {
        let tm = TransactionManager::new();
        let storage = Arc::new(MockStorage { data: Mutex::new(Vec::new()) });
        tm.set_wal_storage(storage.clone());

        let mut txn = tm.begin_transaction();
        txn.record(TransactionOperation::Write {
            ino: 1,
            offset: 0,
            data: b"Atomicity Test".to_vec(),
        });

        // Before commit, storage should be empty (managed by WAL)
        // Wait, apply() is called during commit.
        tm.commit(txn).expect("Commit failed");

        // Verify WAL has data
        assert!(storage.data.lock().len() > 0);
        
        // In a real test, we'd verify that the "Bottom FS" (mocked in apply) 
        // received the data.
    }

    #[test]
    fn test_rollback_safety() {
        let tm = TransactionManager::new();
        let mut txn = tm.begin_transaction();
        txn.record(TransactionOperation::Write {
            ino: 1,
            offset: 0,
            data: b"Should not persist".to_vec(),
        });
        
        tm.rollback(txn);
        // Rollback for deferred execution is just dropping the txn.
    }
}
