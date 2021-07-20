use super::is_false;

use serde::{Deserialize, Serialize};
use twilight_model::{
    channel::{
        embed::Embed,
        message::{
            sticker::MessageSticker, Message, MessageActivity, MessageApplication, MessageFlags,
            MessageReaction, MessageReference, MessageType, Sticker,
        },
        Attachment, ChannelMention,
    },
    guild::PartialMember,
    id::{ChannelId, GuildId, MessageId, RoleId, UserId, WebhookId},
};

/// Represents a cached [`Message`].
///
/// [`Message`]: twilight_model::channel::Message
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CachedMessage {
    /// ID of the message.
    #[serde(rename = "a")]
    pub id: MessageId,
    /// For rich presence chat embeds, the activity object.
    #[serde(default, rename = "b", skip_serializing_if = "Option::is_none")]
    pub activity: Option<MessageActivity>,
    /// For interaction responses, the ID of the interaction's application.
    #[serde(default, rename = "c", skip_serializing_if = "Option::is_none")]
    pub application: Option<MessageApplication>,
    /// Attached files.
    #[serde(default, rename = "d", skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<Attachment>,
    /// ID of the message author.
    ///
    /// If the author is a webhook, this is its ID.
    #[serde(rename = "e")]
    pub author: UserId,
    /// ID of the channel the message was sent in.
    #[serde(rename = "f")]
    pub channel_id: ChannelId,
    /// Content of the message.
    #[serde(default, rename = "g", skip_serializing_if = "String::is_empty")]
    pub content: String,
    /// ISO 8601 timestamp of the date the message was last edited.
    #[serde(default, rename = "h", skip_serializing_if = "Option::is_none")]
    pub edited_timestamp: Option<String>,
    /// Embeds attached to the message.
    #[serde(default, rename = "i", skip_serializing_if = "Vec::is_empty")]
    pub embeds: Vec<Embed>,
    /// Message flags.
    #[serde(default, rename = "j", skip_serializing_if = "Option::is_none")]
    pub flags: Option<MessageFlags>,
    /// ID of the guild the message was sent in, if there is one.
    #[serde(default, rename = "k", skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<GuildId>,
    /// Type of the message.
    #[serde(rename = "l")]
    pub kind: MessageType,
    /// Member data for the author, if there is any.
    #[serde(default, rename = "m", skip_serializing_if = "Option::is_none")]
    pub member: Option<PartialMember>,
    /// Channels mentioned in the content.
    #[serde(default, rename = "n", skip_serializing_if = "Vec::is_empty")]
    pub mention_channels: Vec<ChannelMention>,
    /// Whether or not '@everyone' or '@here' is mentioned in the content.
    #[serde(default, rename = "o", skip_serializing_if = "is_false")]
    pub mention_everyone: bool,
    /// Roles mentioned in the content.
    #[serde(default, rename = "p", skip_serializing_if = "Vec::is_empty")]
    pub mention_roles: Vec<RoleId>,
    /// Users mentioned in the content.
    #[serde(default, rename = "q", skip_serializing_if = "Vec::is_empty")]
    pub mentions: Vec<UserId>,
    /// Whether or not the message is pinned.
    #[serde(default, rename = "r", skip_serializing_if = "is_false")]
    pub pinned: bool,
    /// Reactions to the message.
    #[serde(default, rename = "s", skip_serializing_if = "Vec::is_empty")]
    pub reactions: Vec<MessageReaction>,
    /// Message reference.
    #[serde(default, rename = "t", skip_serializing_if = "Option::is_none")]
    pub reference: Option<MessageReference>,
    #[allow(missing_docs)]
    #[deprecated(since = "0.5.2", note = "use `sticker_items`")]
    #[serde(default, rename = "u", skip_serializing_if = "Vec::is_empty")]
    pub stickers: Vec<Sticker>,
    /// Stickers within the message.
    pub sticker_items: Vec<MessageSticker>,
    /// ISO 8601 timestamp of the date the message was sent.
    #[serde(rename = "v")]
    pub timestamp: String,
    /// Whether the message is text-to-speech.
    #[serde(default, rename = "w", skip_serializing_if = "is_false")]
    pub tts: bool,
    /// For messages sent by webhooks, the webhook ID.
    #[serde(default, rename = "x", skip_serializing_if = "Option::is_none")]
    pub webhook_id: Option<WebhookId>,
}

impl From<Message> for CachedMessage {
    fn from(msg: Message) -> Self {
        #[allow(deprecated)]
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
            stickers: Vec::new(),
            sticker_items: msg.sticker_items,
            timestamp: msg.timestamp,
            tts: msg.tts,
            webhook_id: msg.webhook_id,
        }
    }
}
