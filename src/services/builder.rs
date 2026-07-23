use crate::config::ServiceConfig;
use crate::gif::GifResolver;
use crate::services::traits::MandatoryService;
use crate::services::{discord::DiscordService, matrix::MatrixService, whatsapp::WhatsAppService};
use anyhow::Result;
use std::sync::Arc;

/// Builds service instances from configuration
pub struct ServiceBuilder {
    gif_resolver: Arc<GifResolver>,
}

impl ServiceBuilder {
    pub fn new(gif_resolver: Arc<GifResolver>) -> Self {
        Self { gif_resolver }
    }

    /// Build a service from its configuration
    pub fn build(
        &self,
        service_name: String,
        config: &ServiceConfig,
    ) -> Result<Box<dyn MandatoryService>> {
        match config {
            ServiceConfig::Matrix(cfg) => {
                Ok(Box::new(MatrixService::new(service_name, cfg.clone())))
            }
            ServiceConfig::WhatsApp(cfg) => {
                Ok(Box::new(WhatsAppService::new(service_name, cfg.clone())))
            }
            ServiceConfig::Discord(cfg) => Ok(Box::new(DiscordService::new(
                service_name,
                cfg.clone(),
                self.gif_resolver.clone(),
            ))),
        }
    }

    /// Build all services from a Config
    pub fn build_all(
        &self,
        config: &crate::config::Config,
    ) -> Result<Vec<(String, Box<dyn MandatoryService>)>> {
        let mut services = Vec::new();

        for (service_name, service_config) in &config.services {
            let service = self.build(service_name.clone(), service_config)?;
            services.push((service_name.clone(), service));
        }

        Ok(services)
    }

    /// Get max upload size from Matrix services and update GifResolver
    pub async fn update_gif_resolver_from_matrix(
        &self,
        services: &[(String, Box<dyn MandatoryService>)],
    ) -> Result<()> {
        for (name, service) in services {
            if let Some(matrix_svc) = service.as_any().downcast_ref::<MatrixService>() {
                if let Some(max_size) = matrix_svc.max_upload_size() {
                    log::info!(
                        "[ServiceBuilder] Matrix service '{}' max upload size: {} bytes ({:.1} MB)",
                        name,
                        max_size,
                        max_size as f64 / 1_048_576.0
                    );
                    self.gif_resolver.set_max_upload_size(max_size).await;
                }
            }
        }
        Ok(())
    }
}

impl Default for ServiceBuilder {
    fn default() -> Self {
        Self::new(Arc::new(GifResolver::new(&None, false, None)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        Config, DiscordServiceConfig, MatrixServiceConfig, MediaConfig, ServiceConfig,
        WhatsAppServiceConfig,
    };
    use crate::gif::GifResolver;
    use std::sync::Arc;

    fn make_test_config() -> Config {
        let mut services = std::collections::HashMap::new();
        services.insert(
            "matrix1".to_string(),
            ServiceConfig::Matrix(MatrixServiceConfig {
                homeserver_url: "http://localhost:8008".to_string(),
                username: "test".to_string(),
                password: "test".to_string(),
                device_id: None,
                debug: false,
                display_name: None,
            }),
        );
        services.insert(
            "whatsapp1".to_string(),
            ServiceConfig::WhatsApp(WhatsAppServiceConfig {
                session_path: Some("whatsapp.db".to_string()),
                debug: false,
                display_name: None,
            }),
        );
        services.insert(
            "discord1".to_string(),
            ServiceConfig::Discord(DiscordServiceConfig {
                bot_token: "test_token".to_string(),
                debug: false,
                display_name: None,
            }),
        );

        Config {
            services,
            bridges: std::collections::HashMap::new(),
            media: Some(MediaConfig::default()),
        }
    }

    #[test]
    fn builder_builds_matrix_service() {
        let gif_resolver = Arc::new(GifResolver::new(&None, false, None));
        let builder = ServiceBuilder::new(gif_resolver);
        let config = ServiceConfig::Matrix(MatrixServiceConfig {
            homeserver_url: "http://localhost:8008".to_string(),
            username: "test".to_string(),
            password: "test".to_string(),
            device_id: None,
            debug: false,
            display_name: None,
        });

        let service = builder.build("matrix1".to_string(), &config);
        assert!(service.is_ok());
    }

    #[test]
    fn builder_builds_whatsapp_service() {
        let gif_resolver = Arc::new(GifResolver::new(&None, false, None));
        let builder = ServiceBuilder::new(gif_resolver);
        let config = ServiceConfig::WhatsApp(WhatsAppServiceConfig {
            session_path: Some("whatsapp.db".to_string()),
            debug: false,
            display_name: None,
        });

        let service = builder.build("whatsapp1".to_string(), &config);
        assert!(service.is_ok());
    }

    #[test]
    fn builder_builds_discord_service() {
        let gif_resolver = Arc::new(GifResolver::new(&None, false, None));
        let builder = ServiceBuilder::new(gif_resolver);
        let config = ServiceConfig::Discord(DiscordServiceConfig {
            bot_token: "test_token".to_string(),
            debug: false,
            display_name: None,
        });

        let service = builder.build("discord1".to_string(), &config);
        assert!(service.is_ok());
    }

    #[tokio::test]
    async fn builder_build_all() {
        let gif_resolver = Arc::new(GifResolver::new(&None, false, None));
        let builder = ServiceBuilder::new(gif_resolver);
        let config = make_test_config();

        let services = builder.build_all(&config);
        assert!(services.is_ok());
        let services = services.unwrap();
        assert_eq!(services.len(), 3);

        let names: Vec<_> = services.iter().map(|(n, _)| n.clone()).collect();
        assert!(names.contains(&"matrix1".to_string()));
        assert!(names.contains(&"whatsapp1".to_string()));
        assert!(names.contains(&"discord1".to_string()));
    }
}
