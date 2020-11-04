use serde::{Deserialize, Serialize};
use std::sync::Arc;
use twilight_model::{
    guild::{Member, PartialMember},
    id::{GuildId, RoleId, UserId},
    user::User,
};

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CachedMember {
    pub user_id: UserId,
    pub guild_id: GuildId,
    pub nick: Option<String>,
    pub roles: Vec<RoleId>,
    pub user: Arc<User>,
}

impl PartialEq<Member> for CachedMember {
    fn eq(&self, other: &Member) -> bool {
        self.nick == other.nick && self.roles == other.roles
    }
}

impl PartialEq<&PartialMember> for CachedMember {
    fn eq(&self, other: &&PartialMember) -> bool {
        self.nick == other.nick && self.roles == other.roles
    }
}

#[derive(Serialize, Deserialize)]
pub struct ColdStorageMember {
    #[serde(rename = "a")]
    pub user_id: UserId,
    #[serde(rename = "b")]
    pub guild_id: GuildId,
    #[serde(rename = "c", default, skip_serializing_if = "Option::is_none")]
    pub nick: Option<String>,
    #[serde(rename = "d", default, skip_serializing_if = "Vec::is_empty")]
    pub roles: Vec<RoleId>,
}

impl ColdStorageMember {
    pub(crate) fn into_cached_member(self, user: Arc<User>) -> CachedMember {
        CachedMember {
            user_id: self.user_id,
            guild_id: self.guild_id,
            nick: self.nick,
            roles: self.roles,
            user,
        }
    }
}

impl From<Arc<CachedMember>> for ColdStorageMember {
    fn from(member: Arc<CachedMember>) -> Self {
        Self {
            user_id: member.user_id,
            guild_id: member.guild_id,
            nick: member.nick.to_owned(),
            roles: member.roles.to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::CachedMember;
    use std::sync::Arc;
    use twilight_model::{
        guild::{Member, PartialMember},
        id::{GuildId, RoleId, UserId},
        user::User,
    };

    fn cached_member() -> CachedMember {
        CachedMember {
            guild_id: GuildId(3),
            nick: Some("member nick".to_owned()),
            roles: Vec::new(),
            user: Arc::new(user()),
            user_id: UserId(1),
        }
    }

    fn user() -> User {
        User {
            avatar: None,
            bot: false,
            discriminator: "0001".to_owned(),
            email: None,
            flags: None,
            id: UserId(1),
            locale: None,
            mfa_enabled: None,
            name: "bar".to_owned(),
            premium_type: None,
            public_flags: None,
            system: None,
            verified: None,
        }
    }

    #[test]
    fn test_eq_member() {
        let member = Member {
            deaf: false,
            guild_id: GuildId(3),
            hoisted_role: Some(RoleId(4)),
            joined_at: None,
            mute: true,
            nick: Some("member nick".to_owned()),
            premium_since: None,
            roles: Vec::new(),
            user: user(),
        };

        assert_eq!(cached_member(), member);
    }

    #[test]
    fn test_eq_partial_member() {
        let member = PartialMember {
            deaf: false,
            joined_at: None,
            mute: true,
            nick: Some("member nick".to_owned()),
            roles: Vec::new(),
        };

        assert_eq!(cached_member(), &member);
    }
}
