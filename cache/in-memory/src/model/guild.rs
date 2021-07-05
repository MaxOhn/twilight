use super::is_false;

use serde::{Deserialize, Serialize};
use twilight_model::{
    guild::{
        DefaultMessageNotificationLevel, ExplicitContentFilter, MfaLevel, NSFWLevel, Permissions,
        PremiumTier, SystemChannelFlags, VerificationLevel,
    },
    id::{ApplicationId, ChannelId, GuildId, UserId},
};

/// Represents a cached [`Guild`].
///
/// [`Guild`]: twilight_model::guild::Guild
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CachedGuild {
    /// ID of the guild.
    #[serde(rename = "a")]
    pub id: GuildId,
    /// ID of the AFK channel.
    #[serde(default, rename = "b", skip_serializing_if = "Option::is_none")]
    pub afk_channel_id: Option<ChannelId>,
    /// AFK timeout in seconds.
    #[serde(rename = "c")]
    pub afk_timeout: u64,
    /// For bot created guilds, the ID of the creating application.
    #[serde(default, rename = "d", skip_serializing_if = "Option::is_none")]
    pub application_id: Option<ApplicationId>,
    /// Banner hash.
    ///
    /// See [Discord Docs/Image Formatting].
    ///
    /// [Discord Docs/Image Formatting]: https://discord.com/developers/docs/reference#image-formatting
    #[serde(default, rename = "e", skip_serializing_if = "Option::is_none")]
    pub banner: Option<String>,
    /// Default message notification level.
    #[serde(rename = "f")]
    pub default_message_notifications: DefaultMessageNotificationLevel,
    /// For Community guilds, the description.
    #[serde(default, rename = "g", skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// For discoverable guilds, the discovery splash hash.
    ///
    /// See [Discord Docs/Image Formatting].
    ///
    /// [Discord Docs/Image Formatting]: https://discord.com/developers/docs/reference#image-formatting
    #[serde(default, rename = "h", skip_serializing_if = "Option::is_none")]
    pub discovery_splash: Option<String>,
    /// Explicit content filter level.
    #[serde(rename = "i")]
    pub explicit_content_filter: ExplicitContentFilter,
    /// Enabled [guild features].
    ///
    /// [guild features]: https://discord.com/developers/docs/resources/guild#guild-object-guild-features
    #[serde(default, rename = "j", skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<String>,
    /// Icon hash.
    ///
    /// See [Discord Docs/Image Formatting].
    ///
    /// [Discord Docs/Image Formatting]: https://discord.com/developers/docs/reference#image-formatting
    #[serde(default, rename = "l", skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// ISO 8601 timestamp of the user's join date.
    #[serde(default, rename = "m", skip_serializing_if = "Option::is_none")]
    pub joined_at: Option<String>,
    /// Whether this guild is "large".
    #[serde(default, rename = "n", skip_serializing_if = "is_false")]
    pub large: bool,
    /// Maximum members.
    #[serde(default, rename = "o", skip_serializing_if = "Option::is_none")]
    pub max_members: Option<u64>,
    /// Maximum presences.
    #[serde(default, rename = "p", skip_serializing_if = "Option::is_none")]
    pub max_presences: Option<u64>,
    /// Total number of members in the guild.
    #[serde(default, rename = "q", skip_serializing_if = "Option::is_none")]
    pub member_count: Option<u64>,
    /// Required MFA level.
    #[serde(rename = "r")]
    pub mfa_level: MfaLevel,
    /// Name of the guild.
    #[serde(rename = "s")]
    pub name: String,
    /// NSFW level.
    #[serde(rename = "t")]
    pub nsfw_level: NSFWLevel,
    /// Whether the current user is the owner of the guild.
    #[serde(default, rename = "u", skip_serializing_if = "Option::is_none")]
    pub owner: Option<bool>,
    /// ID of the guild's owner.
    #[serde(rename = "v")]
    pub owner_id: UserId,
    /// Total permissions for the current user in the guild, excluding overwrites.
    #[serde(default, rename = "w", skip_serializing_if = "Option::is_none")]
    pub permissions: Option<Permissions>,
    /// Preferred locale for Community guilds.
    ///
    /// Used in server discovery and notices from Discord. Defaults to "en-US".
    #[serde(rename = "x")]
    pub preferred_locale: String,
    /// Number of boosts this guild currently has.
    #[serde(default, rename = "y", skip_serializing_if = "Option::is_none")]
    pub premium_subscription_count: Option<u64>,
    /// Server boost level.
    #[serde(rename = "z")]
    pub premium_tier: PremiumTier,
    /// For Community guilds, the ID of the rules channel.
    #[serde(default, rename = "0", skip_serializing_if = "Option::is_none")]
    pub rules_channel_id: Option<ChannelId>,
    /// Splash hash.
    ///
    /// See [Discord Docs/Image Formatting].
    ///
    /// [Discord Docs/Image Formatting]: https://discord.com/developers/docs/reference#image-formatting
    #[serde(default, rename = "1", skip_serializing_if = "Option::is_none")]
    pub splash: Option<String>,
    /// ID of the channel where notices are posted.
    ///
    /// Example notices include welcome messages and boost events.
    #[serde(default, rename = "2", skip_serializing_if = "Option::is_none")]
    pub system_channel_id: Option<ChannelId>,
    /// System channel flags.
    #[serde(rename = "3")]
    pub system_channel_flags: SystemChannelFlags,
    /// Whether the guild is unavailable due to an outage.
    #[serde(default, rename = "4", skip_serializing_if = "is_false")]
    pub unavailable: bool,
    /// Vanity URL code.
    #[serde(rename = "5")]
    pub verification_level: VerificationLevel,
    /// ID of the channel that a widget generates an invite to.
    #[serde(default, rename = "6", skip_serializing_if = "Option::is_none")]
    pub vanity_url_code: Option<String>,
    /// Required verification level.
    #[serde(default, rename = "7", skip_serializing_if = "Option::is_none")]
    pub widget_channel_id: Option<ChannelId>,
    /// Whether the widget is enabled.
    #[serde(default, rename = "8", skip_serializing_if = "Option::is_none")]
    pub widget_enabled: Option<bool>,
}
