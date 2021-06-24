use super::is_false;

use serde::{Deserialize, Serialize};
use twilight_model::{
    channel::{
        embed::Embed,
        message::{
            Message, MessageActivity, MessageApplication, MessageFlags, MessageReaction,
            MessageReference, MessageType, Sticker,
        },
        Attachment, ChannelMention,
    },
    guild::PartialMember,
    id::{ChannelId, GuildId, MessageId, RoleId, UserId, WebhookId},
};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CachedMessage {
    #[serde(rename = "a")]
    pub id: MessageId,
    #[serde(default, rename = "b", skip_serializing_if = "Option::is_none")]
    pub activity: Option<MessageActivity>,
    #[serde(default, rename = "c", skip_serializing_if = "Option::is_none")]
    pub application: Option<MessageApplication>,
    #[serde(default, rename = "d", skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<Attachment>,
    #[serde(rename = "e")]
    pub author: UserId,
    #[serde(rename = "f")]
    pub channel_id: ChannelId,
    #[serde(default, rename = "g", skip_serializing_if = "String::is_empty")]
    pub content: String,
    #[serde(default, rename = "h", skip_serializing_if = "Option::is_none")]
    pub edited_timestamp: Option<String>,
    #[serde(default, rename = "i", skip_serializing_if = "Vec::is_empty")]
    pub embeds: Vec<Embed>,
    #[serde(default, rename = "j", skip_serializing_if = "Option::is_none")]
    pub flags: Option<MessageFlags>,
    #[serde(default, rename = "k", skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<GuildId>,
    #[serde(rename = "l")]
    pub kind: MessageType,
    #[serde(default, rename = "m", skip_serializing_if = "Option::is_none")]
    pub member: Option<PartialMember>,
    #[serde(default, rename = "n", skip_serializing_if = "Vec::is_empty")]
    pub mention_channels: Vec<ChannelMention>,
    #[serde(default, rename = "o", skip_serializing_if = "is_false")]
    pub mention_everyone: bool,
    #[serde(default, rename = "p", skip_serializing_if = "Vec::is_empty")]
    pub mention_roles: Vec<RoleId>,
    #[serde(default, rename = "q", skip_serializing_if = "Vec::is_empty")]
    pub mentions: Vec<UserId>,
    #[serde(default, rename = "r", skip_serializing_if = "is_false")]
    pub pinned: bool,
    #[serde(default, rename = "s", skip_serializing_if = "Vec::is_empty")]
    pub reactions: Vec<MessageReaction>,
    #[serde(default, rename = "t", skip_serializing_if = "Option::is_none")]
    pub reference: Option<MessageReference>,
    #[serde(default, rename = "u", skip_serializing_if = "Vec::is_empty")]
    pub stickers: Vec<Sticker>,
    #[serde(rename = "v")]
    pub timestamp: String,
    #[serde(default, rename = "w", skip_serializing_if = "is_false")]
    pub tts: bool,
    #[serde(default, rename = "x", skip_serializing_if = "Option::is_none")]
    pub webhook_id: Option<WebhookId>,
}

impl From<Message> for CachedMessage {
    fn from(msg: Message) -> Self {
        Self {
            id: msg.id,
            activity: msg.activity,
            application: msg.application,
            attachments: msg.attachments,
            author: msg.author.id,
            channel_id: msg.channel_id,
            content: msg.content,
            edited_timestamp: msg.edited_timestamp,
            embeds: msg.embeds,
            flags: msg.flags,
            guild_id: msg.guild_id,
            kind: msg.kind,
            member: msg.member,
            mention_channels: msg.mention_channels,
            mention_everyone: msg.mention_everyone,
            mention_roles: msg.mention_roles,
            mentions: msg.mentions.iter().map(|mention| mention.id).collect(),
            pinned: msg.pinned,
            reactions: msg.reactions,
            reference: msg.reference,
            stickers: msg.stickers,
            timestamp: msg.timestamp,
            tts: msg.tts,
            webhook_id: msg.webhook_id,
        }
    }
}
