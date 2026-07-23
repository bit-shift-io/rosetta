use crate::config::ChannelConfig;
use std::collections::HashMap;

pub struct AliasResolver;

impl AliasResolver {
    pub fn resolve(
        &self,
        sender_id: &str,
        channel_config: &ChannelConfig,
        fallback_display_name: &str,
    ) -> String {
        channel_config
            .aliases
            .get(sender_id)
            .cloned()
            .unwrap_or_else(|| fallback_display_name.to_string())
    }
}

fn make_channel_config() -> ChannelConfig {
    let mut aliases = HashMap::new();
    aliases.insert("@user1:matrix.org".to_string(), "Alice".to_string());
    aliases.insert("user2".to_string(), "Bob".to_string());

    ChannelConfig {
        service: "matrix".to_string(),
        channel: "!room1:matrix.org".to_string(),
        display_names: true,
        enable_media: true,
        bridge_own_messages: false,
        aliases,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_returns_alias_when_present() {
        let config = make_channel_config();
        let resolver = AliasResolver;

        let display_name = resolver.resolve("@user1:matrix.org", &config, "DefaultName");

        assert_eq!(display_name, "Alice");
    }

    #[test]
    fn resolve_returns_fallback_when_no_alias() {
        let config = make_channel_config();
        let resolver = AliasResolver;

        let display_name = resolver.resolve("@unknown:matrix.org", &config, "DefaultName");

        assert_eq!(display_name, "DefaultName");
    }

    #[test]
    fn resolve_with_empty_aliases_map_returns_fallback() {
        let mut config = make_channel_config();
        config.aliases.clear();
        let resolver = AliasResolver;

        let display_name = resolver.resolve("@user1:matrix.org", &config, "DefaultName");

        assert_eq!(display_name, "DefaultName");
    }

    #[test]
    fn resolve_multiple_aliases_work_independently() {
        let config = make_channel_config();
        let resolver = AliasResolver;

        let name1 = resolver.resolve("@user1:matrix.org", &config, "DefaultName");
        let name2 = resolver.resolve("user2", &config, "DefaultName");

        assert_eq!(name1, "Alice");
        assert_eq!(name2, "Bob");
    }

    #[test]
    fn resolve_case_sensitive() {
        let config = make_channel_config();
        let resolver = AliasResolver;

        let display_name = resolver.resolve("@USER1:MATRIX.ORG", &config, "DefaultName");

        assert_eq!(display_name, "DefaultName");
    }
}
