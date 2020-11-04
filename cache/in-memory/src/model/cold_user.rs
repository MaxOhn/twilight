use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use twilight_model::{
    id::{GuildId, UserId},
    user::{PremiumType, User, UserFlags},
};

#[derive(Deserialize, Serialize)]
pub(crate) struct ColdStorageUser {
    #[serde(rename = "a", default, skip_serializing_if = "Option::is_none")]
    pub avatar: Option<String>,
    #[serde(rename = "b")]
    pub bot: bool,
    #[serde(rename = "c")]
    pub discriminator: String,
    #[serde(rename = "d", default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(rename = "e", default, skip_serializing_if = "Option::is_none")]
    pub flags: Option<UserFlags>,
    #[serde(rename = "f")]
    pub id: UserId,
    #[serde(rename = "g", default, skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
    #[serde(rename = "h", default, skip_serializing_if = "Option::is_none")]
    pub mfa_enabled: Option<bool>,
    #[serde(rename = "i")]
    pub name: String,
    #[serde(rename = "j", default, skip_serializing_if = "Option::is_none")]
    pub premium_type: Option<PremiumType>,
    #[serde(rename = "k", default, skip_serializing_if = "Option::is_none")]
    pub public_flags: Option<UserFlags>,
    #[serde(rename = "l", default, skip_serializing_if = "Option::is_none")]
    pub system: Option<bool>,
    #[serde(rename = "m", default, skip_serializing_if = "Option::is_none")]
    pub verified: Option<bool>,
    #[serde(rename = "n")]
    pub guilds: BTreeSet<GuildId>,
}

impl Into<(User, BTreeSet<GuildId>)> for ColdStorageUser {
    fn into(self) -> (User, BTreeSet<GuildId>) {
        let user = User {
            avatar: self.avatar,
            bot: self.bot,
            discriminator: self.discriminator,
            email: self.email,
            flags: self.flags,
            id: self.id,
            locale: self.locale,
            mfa_enabled: self.mfa_enabled,
            name: self.name,
            premium_type: self.premium_type,
            public_flags: self.public_flags,
            system: self.system,
            verified: self.verified,
        };
        (user, self.guilds)
    }
}
