use crate::GuildItem;

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use twilight_model::{
    guild::{Permissions, Role},
    id::{GuildId, RoleId},
};

#[derive(Deserialize, Serialize)]
pub(crate) struct ColdStorageRole {
    #[serde(rename = "a")]
    pub color: u32,
    #[serde(rename = "b")]
    pub hoist: bool,
    #[serde(rename = "c")]
    pub id: RoleId,
    #[serde(rename = "d")]
    pub managed: bool,
    #[serde(rename = "e")]
    pub mentionable: bool,
    #[serde(rename = "f")]
    pub name: String,
    #[serde(rename = "g")]
    pub permissions: Permissions,
    #[serde(rename = "h")]
    pub position: i64,
    #[serde(rename = "i")]
    pub guild_id: GuildId,
}

impl Into<GuildItem<Role>> for ColdStorageRole {
    fn into(self) -> GuildItem<Role> {
        let role = Role {
            color: self.color,
            hoist: self.hoist,
            id: self.id,
            managed: self.managed,
            mentionable: self.mentionable,
            name: self.name,
            permissions: self.permissions,
            position: self.position,
            tags: None,
        };
        GuildItem {
            data: Arc::new(role),
            guild_id: self.guild_id,
        }
    }
}
