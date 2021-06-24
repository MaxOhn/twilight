use super::is_false;

use serde::{Deserialize, Serialize};
use twilight_model::{
    guild::{
        DefaultMessageNotificationLevel, ExplicitContentFilter, MfaLevel, NSFWLevel, Permissions,
        PremiumTier, SystemChannelFlags, VerificationLevel,
    },
    id::{ApplicationId, ChannelId, GuildId, UserId},
};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CachedGuild {
    #[serde(rename = "a")]
    pub id: GuildId,
    #[serde(default, rename = "b", skip_serializing_if = "Option::is_none")]
    pub afk_channel_id: Option<ChannelId>,
    #[serde(rename = "c")]
    pub afk_timeout: u64,
    #[serde(default, rename = "d", skip_serializing_if = "Option::is_none")]
    pub application_id: Option<ApplicationId>,
    #[serde(default, rename = "e", skip_serializing_if = "Option::is_none")]
    pub banner: Option<String>,
    #[serde(rename = "f")]
    pub default_message_notifications: DefaultMessageNotificationLevel,
    #[serde(default, rename = "g", skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, rename = "h", skip_serializing_if = "Option::is_none")]
    pub discovery_splash: Option<String>,
    #[serde(rename = "i")]
    pub explicit_content_filter: ExplicitContentFilter,
    #[serde(default, rename = "j", skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<String>,
    #[serde(default, rename = "l", skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, rename = "m", skip_serializing_if = "Option::is_none")]
    pub joined_at: Option<String>,
    #[serde(default, rename = "n", skip_serializing_if = "is_false")]
    pub large: bool,
    #[serde(default, rename = "o", skip_serializing_if = "Option::is_none")]
    pub max_members: Option<u64>,
    #[serde(default, rename = "p", skip_serializing_if = "Option::is_none")]
    pub max_presences: Option<u64>,
    #[serde(default, rename = "q", skip_serializing_if = "Option::is_none")]
    pub member_count: Option<u64>,
    #[serde(rename = "r")]
    pub mfa_level: MfaLevel,
    #[serde(rename = "s")]
    pub name: String,
    #[serde(rename = "t")]
    pub nsfw_level: NSFWLevel,
    #[serde(default, rename = "u", skip_serializing_if = "Option::is_none")]
    pub owner: Option<bool>,
    #[serde(rename = "v")]
    pub owner_id: UserId,
    #[serde(default, rename = "w", skip_serializing_if = "Option::is_none")]
    pub permissions: Option<Permissions>,
    #[serde(rename = "x")]
    pub preferred_locale: String,
    #[serde(default, rename = "y", skip_serializing_if = "Option::is_none")]
    pub premium_subscription_count: Option<u64>,
    #[serde(rename = "z")]
    pub premium_tier: PremiumTier,
    #[serde(default, rename = "0", skip_serializing_if = "Option::is_none")]
    pub rules_channel_id: Option<ChannelId>,
    #[serde(default, rename = "1", skip_serializing_if = "Option::is_none")]
    pub splash: Option<String>,
    #[serde(default, rename = "2", skip_serializing_if = "Option::is_none")]
    pub system_channel_id: Option<ChannelId>,
    #[serde(rename = "3")]
    pub system_channel_flags: SystemChannelFlags,
    #[serde(default, rename = "4", skip_serializing_if = "is_false")]
    pub unavailable: bool,
    #[serde(rename = "5")]
    pub verification_level: VerificationLevel,
    #[serde(default, rename = "6", skip_serializing_if = "Option::is_none")]
    pub vanity_url_code: Option<String>,
    #[serde(default, rename = "7", skip_serializing_if = "Option::is_none")]
    pub widget_channel_id: Option<ChannelId>,
    #[serde(default, rename = "8", skip_serializing_if = "Option::is_none")]
    pub widget_enabled: Option<bool>,
}
