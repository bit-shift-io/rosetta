use crate::persistence::MessageStore;
use std::sync::Arc;

pub struct Deduplicator {
    store: Arc<MessageStore>,
}

impl Deduplicator {
    pub fn new(store: Arc<MessageStore>) -> Self {
        Self { store }
    }

    /// Check if a message has been processed before, and mark it as processed.
    /// Returns true if the message was already processed (duplicate), false if this is the first time.
    pub fn check_and_mark(
        &self,
        source_service: &str,
        source_channel: &str,
        source_id: &str,
    ) -> anyhow::Result<bool> {
        // Check if already exists
        if self
            .store
            .exists(source_service, source_channel, source_id)?
        {
            return Ok(true);
        }

        // Not seen before - mark it as processed by inserting a dummy mapping
        // We use a special dest_service of "__dedup__" to indicate this is just a deduplication marker
        self.store.save_mapping(
            source_service,
            source_channel,
            source_id,
            "__dedup__",
            "__dedup__",
            "__dedup__",
        )?;

        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::MessageStore;
    use std::sync::Arc;
    use tempfile::TempDir;

    fn make_test_store() -> (Arc<MessageStore>, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("test.db");
        let store = Arc::new(MessageStore::new(path.to_str().unwrap()).unwrap());
        (store, temp_dir)
    }

    #[test]
    fn check_and_mark_first_call_returns_false() {
        let (store, _temp_dir) = make_test_store();
        let deduplicator = Deduplicator::new(store);

        let result = deduplicator
            .check_and_mark("matrix", "room1", "msg1")
            .unwrap();

        assert!(!result, "First call should return false (not duplicate)");
    }

    #[test]
    fn check_and_mark_second_call_returns_true() {
        let (store, _temp_dir) = make_test_store();
        let deduplicator = Deduplicator::new(store);

        // First call - mark as seen
        deduplicator
            .check_and_mark("matrix", "room1", "msg1")
            .unwrap();

        // Second call - should return true (duplicate)
        let result = deduplicator
            .check_and_mark("matrix", "room1", "msg1")
            .unwrap();

        assert!(result, "Second call should return true (duplicate)");
    }

    #[test]
    fn check_and_mark_different_source_id_not_collide() {
        let (store, _temp_dir) = make_test_store();
        let deduplicator = Deduplicator::new(store);

        // Mark msg1 as seen
        deduplicator
            .check_and_mark("matrix", "room1", "msg1")
            .unwrap();

        // msg2 should not be duplicate
        let result = deduplicator
            .check_and_mark("matrix", "room1", "msg2")
            .unwrap();

        assert!(!result, "Different message ID should not be duplicate");
    }

    #[test]
    fn check_and_mark_different_channel_not_collide() {
        let (store, _temp_dir) = make_test_store();
        let deduplicator = Deduplicator::new(store);

        // Mark message in room1 as seen
        deduplicator
            .check_and_mark("matrix", "room1", "msg1")
            .unwrap();

        // Same message ID in different channel should not be duplicate
        let result = deduplicator
            .check_and_mark("matrix", "room2", "msg1")
            .unwrap();

        assert!(!result, "Different channel should not be duplicate");
    }

    #[test]
    fn check_and_mark_different_service_not_collide() {
        let (store, _temp_dir) = make_test_store();
        let deduplicator = Deduplicator::new(store);

        // Mark message in matrix as seen
        deduplicator
            .check_and_mark("matrix", "room1", "msg1")
            .unwrap();

        // Same message ID in different service should not be duplicate
        let result = deduplicator
            .check_and_mark("discord", "channel1", "msg1")
            .unwrap();

        assert!(!result, "Different service should not be duplicate");
    }
}
