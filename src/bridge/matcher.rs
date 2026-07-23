use crate::config::{ChannelConfig, Config};
use std::collections::HashMap;

pub struct BridgeMatcher;

impl BridgeMatcher {
    pub fn find_targets<'a>(
        &self,
        source_service: &str,
        source_channel: &str,
        config: &'a Config,
    ) -> Vec<&'a ChannelConfig> {
        let mut targets = Vec::new();

        for (_bridge_name, channels) in &config.bridges {
            let source_exists = channels
                .iter()
                .any(|ch| ch.service == source_service && ch.channel == source_channel);

            if source_exists {
                for channel in channels {
                    let same_service = channel.service == source_service;
                    let same_channel = channel.channel == source_channel;

                    if !(same_service && same_channel) {
                        targets.push(channel);
                    }
                }
            }
        }

        targets
    }
}

fn make_channel_config(service: &str, channel: &str) -> ChannelConfig {
    ChannelConfig {
        service: service.to_string(),
        channel: channel.to_string(),
        display_names: true,
        enable_media: true,
        bridge_own_messages: false,
        aliases: HashMap::new(),
    }
}

fn make_config(bridges: HashMap<String, Vec<ChannelConfig>>) -> Config {
    Config {
        services: HashMap::new(),
        bridges,
        media: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_targets_single_bridge_returns_other_channels() {
        let mut channels = vec![
            make_channel_config("matrix", "room1"),
            make_channel_config("discord", "channel1"),
            make_channel_config("whatsapp", "group1"),
        ];
        let mut bridges = HashMap::new();
        bridges.insert("bridge1".to_string(), channels);

        let config = make_config(bridges);
        let matcher = BridgeMatcher;

        let targets = matcher.find_targets("matrix", "room1", &config);

        assert_eq!(targets.len(), 2);
        let services: Vec<_> = targets.iter().map(|c| c.service.as_str()).collect();
        assert!(services.contains(&"discord"));
        assert!(services.contains(&"whatsapp"));
    }

    #[test]
    fn find_targets_excludes_source_channel() {
        let mut channels = vec![
            make_channel_config("matrix", "room1"),
            make_channel_config("matrix", "room2"),
            make_channel_config("discord", "channel1"),
        ];
        let mut bridges = HashMap::new();
        bridges.insert("bridge1".to_string(), channels);

        let config = make_config(bridges);
        let matcher = BridgeMatcher;

        let targets = matcher.find_targets("matrix", "room1", &config);

        assert_eq!(targets.len(), 2);
        let channels: Vec<_> = targets.iter().map(|c| c.channel.as_str()).collect();
        assert!(channels.contains(&"room2"));
        assert!(channels.contains(&"channel1"));
        assert!(!channels.contains(&"room1"));
    }

    #[test]
    fn find_targets_multiple_bridges_returns_matching_bridge() {
        let mut channels1 = vec![
            make_channel_config("matrix", "room1"),
            make_channel_config("discord", "channel1"),
        ];
        let mut channels2 = vec![
            make_channel_config("matrix", "room2"),
            make_channel_config("whatsapp", "group1"),
        ];
        let mut bridges = HashMap::new();
        bridges.insert("bridge1".to_string(), channels1);
        bridges.insert("bridge2".to_string(), channels2);

        let config = make_config(bridges);
        let matcher = BridgeMatcher;

        let targets = matcher.find_targets("matrix", "room1", &config);

        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].service, "discord");
        assert_eq!(targets[0].channel, "channel1");
    }

    #[test]
    fn find_targets_no_matching_bridge_returns_empty() {
        let mut channels = vec![
            make_channel_config("matrix", "room1"),
            make_channel_config("discord", "channel1"),
        ];
        let mut bridges = HashMap::new();
        bridges.insert("bridge1".to_string(), channels);

        let config = make_config(bridges);
        let matcher = BridgeMatcher;

        let targets = matcher.find_targets("matrix", "room999", &config);

        assert!(targets.is_empty());
    }

    #[test]
    fn find_targets_source_not_in_any_bridge_returns_empty() {
        let mut channels = vec![
            make_channel_config("matrix", "room1"),
            make_channel_config("discord", "channel1"),
        ];
        let mut bridges = HashMap::new();
        bridges.insert("bridge1".to_string(), channels);

        let config = make_config(bridges);
        let matcher = BridgeMatcher;

        let targets = matcher.find_targets("slack", "channel1", &config);

        assert!(targets.is_empty());
    }

    #[test]
    fn find_targets_returns_empty_for_empty_config() {
        let config = Config {
            services: HashMap::new(),
            bridges: HashMap::new(),
            media: None,
        };
        let matcher = BridgeMatcher;

        let targets = matcher.find_targets("matrix", "room1", &config);

        assert!(targets.is_empty());
    }

    #[test]
    fn find_targets_preserves_channel_config() {
        let mut channels = vec![
            make_channel_config("matrix", "room1"),
            make_channel_config("discord", "channel1"),
        ];
        channels[1].display_names = false;
        channels[1].enable_media = false;
        channels[1].bridge_own_messages = true;

        let mut aliases = HashMap::new();
        aliases.insert("user1".to_string(), "Alice".to_string());
        channels[1].aliases = aliases;

        let mut bridges = HashMap::new();
        bridges.insert("bridge1".to_string(), channels);

        let config = make_config(bridges);
        let matcher = BridgeMatcher;

        let targets = matcher.find_targets("matrix", "room1", &config);

        assert_eq!(targets.len(), 1);
        let target = &targets[0];
        assert_eq!(target.service, "discord");
        assert_eq!(target.channel, "channel1");
        assert_eq!(target.display_names, false);
        assert_eq!(target.enable_media, false);
        assert_eq!(target.bridge_own_messages, true);
        assert_eq!(target.aliases.get("user1"), Some(&"Alice".to_string()));
    }
}
