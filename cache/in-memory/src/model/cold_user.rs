use super::is_false;

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use twilight_model::{
    id::{GuildId, UserId},
    user::{PremiumType, User, UserFlags},
};

#[derive(Deserialize, Serialize)]
pub(crate) struct ColdStorageUser {
    #[serde(default, rename = "a", skip_serializing_if = "Option::is_none")]
    pub avatar: Option<String>,
    #[serde(default, rename = "b", skip_serializing_if = "is_false")]
    pub bot: bool,
    #[serde(rename = "c")]
    pub discriminator: String,
    #[serde(default, rename = "d", skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(default, rename = "e", skip_serializing_if = "Option::is_none")]
    pub flags: Option<UserFlags>,
    #[serde(rename = "f")]
    pub id: UserId,
    #[serde(default, rename = "g", skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
    #[serde(default, rename = "h", skip_serializing_if = "Option::is_none")]
    pub mfa_enabled: Option<bool>,
    #[serde(rename = "i")]
    pub name: String,
    #[serde(default, rename = "j", skip_serializing_if = "Option::is_none")]
    pub premium_type: Option<PremiumType>,
    #[serde(default, rename = "k", skip_serializing_if = "Option::is_none")]
    pub public_flags: Option<UserFlags>,
    #[serde(default, rename = "l", skip_serializing_if = "Option::is_none")]
    pub system: Option<bool>,
    #[serde(default, rename = "m", skip_serializing_if = "Option::is_none")]
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
