use crate::services::Service;
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};

/// Registry for managing service lifecycles
pub struct ServiceRegistry {
    services: HashMap<String, Arc<Mutex<Box<dyn Service>>>>,
}

impl ServiceRegistry {
    pub fn new() -> Self {
        Self {
            services: HashMap::new(),
        }
    }

    /// Register a service
    pub fn register(&mut self, name: String, service: Box<dyn Service>) {
        self.services.insert(name, Arc::new(Mutex::new(service)));
    }

    /// Get a service by name
    pub fn get(&self, name: &str) -> Option<Arc<Mutex<Box<dyn Service>>>> {
        self.services.get(name).cloned()
    }

    /// Get all service names
    pub fn names(&self) -> Vec<String> {
        self.services.keys().cloned().collect()
    }

    /// Check if registry is empty
    pub fn is_empty(&self) -> bool {
        self.services.is_empty()
    }

    /// Get number of registered services
    pub fn len(&self) -> usize {
        self.services.len()
    }

    /// Connect all services
    /// Discord connection failures are logged but don't fail the entire process
    pub async fn connect_all(&mut self) -> Result<()> {
        let service_names: Vec<String> = self.services.keys().cloned().collect();

        for name in service_names {
            let service = self.services.get(&name).unwrap().clone();
            let mut svc = service.lock().await;
            match svc.connect().await {
                Ok(_) => {
                    log::info!("Successfully connected to service: {}", name);
                }
                Err(e) => {
                    log::error!("Failed to connect to service {}: {}", name, e);
                    // For Discord (not implemented), we'll skip it but continue
                    if name.to_lowercase().contains("discord") {
                        log::warn!("Skipping Discord service due to connection failure");
                        self.services.remove(&name);
                        continue;
                    }
                    return Err(e);
                }
            }

            // If this is a Matrix service, we'll need to get max upload size later
            // This is handled in main.rs after connect_all
        }

        Ok(())
    }

    /// Start all services with the event channel
    pub async fn start_all(&self, tx: mpsc::Sender<crate::services::ServiceEvent>) -> Result<()> {
        for (name, service) in &self.services {
            let mut svc = service.lock().await;
            match svc.start(tx.clone()).await {
                Ok(_) => log::info!("Started service: {}", name),
                Err(e) => log::error!("Failed to start service {}: {}", name, e),
            }
        }
        Ok(())
    }

    /// Wait for all services to be ready
    pub async fn wait_all_ready(&self) -> Result<()> {
        log::info!("Waiting for all services to be synchronized...");
        for (name, service) in &self.services {
            let svc = service.lock().await;
            if let Err(e) = svc.wait_until_ready().await {
                log::error!("Service {} failed to become ready: {}", name, e);
            }
        }
        log::info!("All services ready.");
        Ok(())
    }

    /// Disconnect all services
    pub async fn shutdown_all(&self) {
        log::info!("Shutting down all services...");
        for (name, service) in &self.services {
            let mut svc = service.lock().await;
            if let Err(e) = svc.disconnect().await {
                log::warn!("Error disconnecting service {}: {}", name, e);
            }
        }
    }

    /// Convert registry into a HashMap for use with BridgeCoordinator
    pub fn into_map(self) -> HashMap<String, Arc<Mutex<Box<dyn Service>>>> {
        self.services
    }

    /// Get a reference to the services map
    pub fn services(&self) -> &HashMap<String, Arc<Mutex<Box<dyn Service>>>> {
        &self.services
    }
}

impl Default for ServiceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::{Service, ServiceEvent, ServiceMessage};
    use anyhow::Result;
    use async_trait::async_trait;
    use std::any::Any;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use tokio::sync::mpsc;

    struct MockService {
        name: String,
        should_fail_connect: bool,
    }

    impl MockService {
        fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
                should_fail_connect: false,
            }
        }

        fn with_fail_connect(mut self, fail: bool) -> Self {
            self.should_fail_connect = fail;
            self
        }
    }

    #[async_trait]
    impl Service for MockService {
        async fn connect(&mut self) -> Result<()> {
            if self.should_fail_connect {
                return Err(anyhow::anyhow!("Simulated connection failure"));
            }
            Ok(())
        }

        async fn start(&mut self, _tx: mpsc::Sender<ServiceEvent>) -> Result<()> {
            Ok(())
        }

        async fn send_message(&self, _channel: &str, _message: &ServiceMessage) -> Result<String> {
            Ok("msg123".to_string())
        }

        async fn edit_message(
            &self,
            _channel: &str,
            _message_id: &str,
            _new_content: &str,
        ) -> Result<()> {
            Ok(())
        }

        async fn react_to_message(
            &self,
            _channel: &str,
            _message_id: &str,
            _emoji: &str,
        ) -> Result<()> {
            Ok(())
        }

        fn service_name(&self) -> &str {
            &self.name
        }

        async fn get_room_members(&self, _channel: &str) -> Result<Vec<String>> {
            Ok(vec![])
        }

        async fn disconnect(&mut self) -> Result<()> {
            Ok(())
        }

        async fn wait_until_ready(&self) -> Result<()> {
            Ok(())
        }

        fn as_any(&self) -> &dyn Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }
    }

    #[tokio::test]
    async fn registry_new_and_register() {
        let mut registry = ServiceRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);

        registry.register("test".to_string(), Box::new(MockService::new("test")));
        assert!(!registry.is_empty());
        assert_eq!(registry.len(), 1);
    }

    #[tokio::test]
    async fn registry_connect_all_success() {
        let mut registry = ServiceRegistry::new();
        registry.register("svc1".to_string(), Box::new(MockService::new("svc1")));
        registry.register("svc2".to_string(), Box::new(MockService::new("svc2")));

        let result = registry.connect_all().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn registry_connect_all_handles_discord_failure() {
        let mut registry = ServiceRegistry::new();
        registry.register("good".to_string(), Box::new(MockService::new("good")));
        registry.register(
            "discord1".to_string(),
            Box::new(MockService::new("discord1").with_fail_connect(true)),
        );

        let result = registry.connect_all().await;
        // Discord failure should be skipped, not fail entirely
        assert!(result.is_ok());
        // But the discord service should be removed
        assert!(!registry.services.contains_key("discord1"));
        assert!(registry.services.contains_key("good"));
    }

    #[tokio::test]
    async fn registry_connect_all_fails_on_non_discord_error() {
        let mut registry = ServiceRegistry::new();
        registry.register("good".to_string(), Box::new(MockService::new("good")));
        registry.register(
            "bad".to_string(),
            Box::new(MockService::new("bad").with_fail_connect(true)),
        );

        let result = registry.connect_all().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn registry_start_all() {
        let mut registry = ServiceRegistry::new();
        registry.register("svc1".to_string(), Box::new(MockService::new("svc1")));

        let (tx, _rx) = mpsc::channel(10);
        let result = registry.start_all(tx).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn registry_wait_all_ready() {
        let mut registry = ServiceRegistry::new();
        registry.register("svc1".to_string(), Box::new(MockService::new("svc1")));

        let result = registry.wait_all_ready().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn registry_shutdown_all() {
        let mut registry = ServiceRegistry::new();
        registry.register("svc1".to_string(), Box::new(MockService::new("svc1")));

        registry.shutdown_all().await;
        // Just verify no panic
    }

    #[tokio::test]
    async fn registry_into_map() {
        let mut registry = ServiceRegistry::new();
        registry.register("svc1".to_string(), Box::new(MockService::new("svc1")));

        let map = registry.into_map();
        assert_eq!(map.len(), 1);
        assert!(map.contains_key("svc1"));
    }
}
