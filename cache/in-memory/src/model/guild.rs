use serde::{Deserialize, Serialize};
use twilight_model::{
    guild::Permissions,
    id::{ChannelId, GuildId, UserId},
};

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct CachedGuild {
    #[serde(rename = "a")]
    pub id: GuildId,
    #[serde(rename = "b")]
    pub icon: Option<String>,
    #[serde(rename = "c", default, skip_serializing_if = "Option::is_none")]
    pub max_members: Option<u64>,
    #[serde(rename = "d", default, skip_serializing_if = "Option::is_none")]
    pub member_count: Option<u64>,
    #[serde(rename = "e")]
    pub name: String,
    pub owner_id: UserId,
    #[serde(rename = "g", default, skip_serializing_if = "Option::is_none")]
    pub permissions: Option<Permissions>,
    #[serde(rename = "h", default, skip_serializing_if = "Option::is_none")]
    pub system_channel_id: Option<ChannelId>,
    #[serde(rename = "i")]
    pub unavailable: bool,
}
