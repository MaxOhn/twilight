use serde::Serialize;
use twilight_model::{
    channel::{
        embed::Embed,
        message::{Message, MessageReaction, MessageReference},
        Attachment, ChannelMention,
    },
    guild::PartialMember,
    id::{ChannelId, GuildId, MessageId, RoleId, UserId},
};

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CachedMessage {
    pub id: MessageId,
    pub attachments: Vec<Attachment>,
    pub author: UserId,
    pub channel_id: ChannelId,
    pub content: String,
    pub embeds: Vec<Embed>,
    pub guild_id: Option<GuildId>,
    pub member: Option<PartialMember>,
    pub mention_channels: Vec<ChannelMention>,
    pub mention_everyone: bool,
    pub mention_roles: Vec<RoleId>,
    pub mentions: Vec<UserId>,
    pub reactions: Vec<MessageReaction>,
    pub reference: Option<MessageReference>,
    pub timestamp: String,
}

impl From<Message> for CachedMessage {
    fn from(msg: Message) -> Self {
        Self {
            id: msg.id,
            attachments: msg.attachments,
            author: msg.author.id,
            channel_id: msg.channel_id,
            content: msg.content,
            embeds: msg.embeds,
            guild_id: msg.guild_id,
            member: msg.member,
            mention_channels: msg.mention_channels,
            mention_everyone: msg.mention_everyone,
            mention_roles: msg.mention_roles,
            mentions: msg.mentions.keys().copied().collect(),
            reactions: msg.reactions,
            reference: msg.reference,
            timestamp: msg.timestamp,
        }
    }
}
