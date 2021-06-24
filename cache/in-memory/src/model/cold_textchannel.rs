use super::is_false;
use crate::GuildItem;

use serde::{Deserialize, Serialize};
use twilight_model::{
    channel::{permission_overwrite::PermissionOverwrite, ChannelType, GuildChannel, TextChannel},
    id::{ChannelId, GuildId, MessageId},
};

#[derive(Deserialize, Serialize)]
pub(crate) struct ColdStorageTextChannel {
    #[serde(default, rename = "a", skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<GuildId>,
    #[serde(rename = "b")]
    pub id: ChannelId,
    #[serde(rename = "c")]
    pub kind: ChannelType,
    #[serde(default, rename = "d", skip_serializing_if = "Option::is_none")]
    pub last_message_id: Option<MessageId>,
    #[serde(default, rename = "e", skip_serializing_if = "Option::is_none")]
    pub last_pin_timestamp: Option<String>,
    #[serde(rename = "f")]
    pub name: String,
    #[serde(default, rename = "g", skip_serializing_if = "is_false")]
    pub nsfw: bool,
    #[serde(default, rename = "h", skip_serializing_if = "Vec::is_empty")]
    pub permission_overwrites: Vec<PermissionOverwrite>,
    #[serde(default, rename = "i", skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<ChannelId>,
    #[serde(rename = "j")]
    pub position: i64,
    #[serde(default, rename = "k", skip_serializing_if = "Option::is_none")]
    pub rate_limit_per_user: Option<u64>,
    #[serde(default, rename = "l", skip_serializing_if = "Option::is_none")]
    pub topic: Option<String>,
}

impl Into<GuildItem<GuildChannel>> for ColdStorageTextChannel {
    fn into(self) -> GuildItem<GuildChannel> {
        let channel = GuildChannel::Text(TextChannel {
            guild_id: self.guild_id,
            id: self.id,
            kind: self.kind,
            last_message_id: self.last_message_id,
            last_pin_timestamp: self.last_pin_timestamp,
            name: self.name,
            nsfw: self.nsfw,
            permission_overwrites: self.permission_overwrites,
            parent_id: self.parent_id,
            position: self.position,
            rate_limit_per_user: self.rate_limit_per_user,
            topic: self.topic,
        });
        GuildItem {
            data: channel,
            guild_id: self.guild_id.unwrap(),
        }
    }
}
