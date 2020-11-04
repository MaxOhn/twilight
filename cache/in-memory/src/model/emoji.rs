use crate::GuildItem;

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use twilight_model::{
    guild::Emoji,
    id::{EmojiId, GuildId, RoleId},
};

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CachedEmoji {
    pub id: EmojiId,
    pub animated: bool,
    pub name: String,
    pub require_colons: bool,
    pub roles: Vec<RoleId>,
    pub available: bool,
}

impl PartialEq<Emoji> for CachedEmoji {
    fn eq(&self, other: &Emoji) -> bool {
        self.id == other.id
            && self.animated == other.animated
            && self.name == other.name
            && self.require_colons == other.require_colons
            && self.roles == other.roles
            && self.available == other.available
    }
}

#[derive(Serialize, Deserialize)]
pub(crate) struct ColdStorageEmoji {
    #[serde(rename = "a")]
    pub guild_id: GuildId,
    #[serde(rename = "b")]
    pub id: EmojiId,
    #[serde(rename = "c")]
    pub animated: bool,
    #[serde(rename = "d")]
    pub name: String,
    #[serde(rename = "e")]
    pub require_colons: bool,
    #[serde(rename = "f", default, skip_serializing_if = "Vec::is_empty")]
    pub roles: Vec<RoleId>,
    #[serde(rename = "g")]
    pub available: bool,
}

impl Into<GuildItem<CachedEmoji>> for ColdStorageEmoji {
    fn into(self) -> GuildItem<CachedEmoji> {
        let emoji = CachedEmoji {
            id: self.id,
            animated: self.animated,
            name: self.name,
            require_colons: self.require_colons,
            roles: self.roles,
            available: self.available,
        };
        GuildItem {
            data: Arc::new(emoji),
            guild_id: self.guild_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::CachedEmoji;
    use std::fmt::Debug;
    use twilight_model::{guild::Emoji, id::EmojiId};

    #[test]
    fn test_eq_emoji() {
        let emoji = Emoji {
            id: EmojiId(123),
            animated: true,
            name: "foo".to_owned(),
            managed: false,
            require_colons: true,
            roles: vec![],
            user: None,
            available: true,
        };
        let cached = CachedEmoji {
            id: EmojiId(123),
            animated: true,
            name: "foo".to_owned(),
            require_colons: true,
            roles: vec![],
            available: true,
        };

        assert_eq!(cached, emoji);
    }

    #[test]
    fn test_fields() {
        static_assertions::assert_fields!(CachedEmoji: id, animated, name, require_colons, roles);
    }

    #[test]
    fn test_impls() {
        static_assertions::assert_impl_all!(CachedEmoji: Clone, Debug, Eq, PartialEq);
    }
}
