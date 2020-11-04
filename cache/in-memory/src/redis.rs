use super::{
    CachedEmoji, CachedGuild, ColdStorageCurrentUser, ColdStorageEmoji, ColdStorageMember,
    ColdStorageRole, ColdStorageTextChannel, ColdStorageUser, Config, GuildItem, InMemoryCache,
};

use darkredis::ConnectionPool;
use futures::future;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    error::Error,
    ops::Deref,
    sync::Arc,
    time::Instant,
};
use twilight_gateway::shard::ResumeSession;
use twilight_model::{
    channel::GuildChannel,
    guild::Role,
    id::{ChannelId, EmojiId, GuildId, RoleId, UserId},
};

const STORE_DURATION: u32 = 180; // seconds

#[derive(Deserialize, Serialize, Debug)]
pub struct ColdRebootData {
    pub resume_data: HashMap<u64, (String, u64)>,
    pub shard_count: u64,
    pub total_shards: u64,
    pub guild_chunks: usize,
    pub user_chunks: usize,
    pub member_chunks: usize,
    pub channel_chunks: usize,
    pub role_chunks: usize,
    pub emoji_chunks: usize,
    pub unavailable_guild_chunks: usize,
}

impl InMemoryCache {
    pub async fn from_redis(
        redis: &ConnectionPool,
        total_shards: u64,
        shards_per_cluster: u64,
        config: Config,
    ) -> (Self, Option<HashMap<u64, ResumeSession>>) {
        let cache = Self::new_with_config(config);
        let mut connection = redis.get().await;
        let key = "cb_cluster_data";
        if let Some(data) = connection.get(key).await.ok().flatten() {
            let cold_cache: ColdRebootData = serde_json::from_slice(&data).unwrap();
            connection.del(key).await.unwrap();
            if cold_cache.total_shards == total_shards
                && cold_cache.shard_count == shards_per_cluster
            {
                let map: HashMap<_, _> = cold_cache
                    .resume_data
                    .iter()
                    .map(|(id, data)| {
                        (
                            *id,
                            ResumeSession {
                                session_id: data.0.to_owned(),
                                sequence: data.1,
                            },
                        )
                    })
                    .collect();
                let start = Instant::now();
                if let Err((cause, why)) = cache.restore_cold_resume(redis, cold_cache).await {
                    error!("Cold resume defrosting failed ({}): {}", cause, why);
                    cache.clear();
                } else {
                    let end = Instant::now();
                    debug!(
                        "Cold resume defrosting completed in {}ms",
                        (end - start).as_millis()
                    );
                }
                return (cache, Some(map));
            }
        }
        (cache, None)
    }

    // ###################
    // ## Defrost cache ##
    // ###################

    async fn restore_cold_resume(
        &self,
        redis: &ConnectionPool,
        reboot_data: ColdRebootData,
    ) -> Result<(), (&'static str, Box<dyn Error>)> {
        // --- Guilds ---
        let guild_defrosters: Vec<_> = (0..reboot_data.guild_chunks)
            .map(|i| self.defrost_guilds(redis, i))
            .collect();
        future::try_join_all(guild_defrosters)
            .await
            .map_err(|e| ("guilds", e))?;
        // --- Users ---
        let user_defrosters: Vec<_> = (0..reboot_data.user_chunks)
            .map(|i| self.defrost_users(redis, i))
            .collect();
        future::try_join_all(user_defrosters)
            .await
            .map_err(|e| ("users", e))?;
        // --- Members ---
        let member_defrosters: Vec<_> = (0..reboot_data.member_chunks)
            .map(|i| self.defrost_members(redis, i))
            .collect();
        future::try_join_all(member_defrosters)
            .await
            .map_err(|e| ("members", e))?;
        // --- Channels ---
        let channel_defrosters: Vec<_> = (0..reboot_data.channel_chunks)
            .map(|i| self.defrost_channels(redis, i))
            .collect();
        future::try_join_all(channel_defrosters)
            .await
            .map_err(|e| ("channels", e))?;
        // --- Emojis ---
        let emoji_defrosters: Vec<_> = (0..reboot_data.emoji_chunks)
            .map(|i| self.defrost_emojis(redis, i))
            .collect();
        future::try_join_all(emoji_defrosters)
            .await
            .map_err(|e| ("emojis", e))?;
        // --- Roles ---
        let role_defrosters: Vec<_> = (0..reboot_data.role_chunks)
            .map(|i| self.defrost_roles(redis, i))
            .collect();
        future::try_join_all(role_defrosters)
            .await
            .map_err(|e| ("roles", e))?;
        // --- Unavailable guilds ---
        let unavailable_guilds_defrosters: Vec<_> = (0..reboot_data.unavailable_guild_chunks)
            .map(|i| self.defrost_unavailable_guilds(redis, i))
            .collect();
        future::try_join_all(unavailable_guilds_defrosters)
            .await
            .map_err(|e| ("unavailable guilds", e))?;
        // --- CurrentUser ---
        self.defrost_current_user(redis)
            .await
            .map_err(|e| ("current_user", e))?;
        debug!(
            "Cache defrosting complete:\n\
            {} guilds | {} channels_guild | {} users\n\
            {} members | {} emojis | {} roles\n\
            {} guild_channels | {} guild_emojis | {} guilds_members\n\
            {} guild_roles | {} unavailable_guilds",
            self.0.guilds.len(),
            self.0.channels_guild.len(),
            self.0.users.len(),
            self.0.members.len(),
            self.0.emojis.len(),
            self.0.roles.len(),
            self.0
                .guild_channels
                .iter()
                .map(|guard| guard.value().len())
                .sum::<usize>(),
            self.0
                .guild_emojis
                .iter()
                .map(|guard| guard.value().len())
                .sum::<usize>(),
            self.0
                .guild_members
                .iter()
                .map(|guard| guard.value().len())
                .sum::<usize>(),
            self.0
                .guild_roles
                .iter()
                .map(|guard| guard.value().len())
                .sum::<usize>(),
            self.0.unavailable_guilds.len(),
        );
        Ok(())
    }

    async fn defrost_guilds(
        &self,
        redis: &ConnectionPool,
        index: usize,
    ) -> Result<(), Box<dyn Error>> {
        let key = format!("cb_cluster_guild_chunk_{}", index);
        let mut connection = redis.get().await;
        let data = connection.get(&key).await?.unwrap();
        let guilds: Vec<CachedGuild> = serde_json::from_slice(&data)?;
        connection.del(key).await?;
        debug!("Worker {} found {} guilds to defrost", index, guilds.len());
        for guild in guilds {
            self.0.guilds.insert(guild.id, Arc::new(guild));
        }
        Ok(())
    }

    async fn defrost_users(
        &self,
        redis: &ConnectionPool,
        index: usize,
    ) -> Result<(), Box<dyn Error>> {
        let key = format!("cb_cluster_user_chunk_{}", index);
        let mut connection = redis.get().await;
        let data = connection.get(&key).await?.unwrap();
        let users: Vec<ColdStorageUser> = serde_json::from_slice(&data)?;
        connection.del(key).await?;
        debug!("Worker {} found {} users to defrost", index, users.len());
        for user in users {
            let (user, guilds) = user.into();
            self.0.users.insert(user.id, (Arc::new(user), guilds));
        }
        Ok(())
    }

    async fn defrost_members(
        &self,
        redis: &ConnectionPool,
        index: usize,
    ) -> Result<(), Box<dyn Error>> {
        let key = format!("cb_cluster_member_chunk_{}", index);
        let mut connection = redis.get().await;
        let data = connection.get(&key).await?.unwrap();
        let members: Vec<ColdStorageMember> = serde_json::from_slice(&data)?;
        connection.del(key).await?;
        debug!(
            "Worker {} found {} members to defrost",
            index,
            members.len()
        );
        for member in members {
            self.0
                .guild_members
                .entry(member.guild_id)
                .or_insert_with(HashSet::new)
                .insert(member.user_id);
            let user = match self.0.users.get(&member.user_id) {
                Some(guard) => Arc::clone(&guard.value().0),
                None => continue,
            };
            self.0.members.insert(
                (member.guild_id, member.user_id),
                Arc::new(member.into_cached_member(user)),
            );
        }
        Ok(())
    }

    async fn defrost_channels(
        &self,
        redis: &ConnectionPool,
        index: usize,
    ) -> Result<(), Box<dyn Error>> {
        let key = format!("cb_cluster_channel_chunk_{}", index);
        let mut connection = redis.get().await;
        let data = connection.get(&key).await?.unwrap();
        let channels: Vec<ColdStorageTextChannel> = serde_json::from_slice(&data)?;
        connection.del(key).await?;
        debug!(
            "Worker {} found {} textchannels to defrost",
            index,
            channels.len()
        );
        for channel in channels {
            self.0
                .guild_channels
                .entry(channel.guild_id.unwrap())
                .or_insert_with(HashSet::new)
                .insert(channel.id);
            self.0.channels_guild.insert(channel.id, channel.into());
        }
        Ok(())
    }

    async fn defrost_emojis(
        &self,
        redis: &ConnectionPool,
        index: usize,
    ) -> Result<(), Box<dyn Error>> {
        let key = format!("cb_cluster_emoji_chunk_{}", index);
        let mut connection = redis.get().await;
        let data = connection.get(&key).await?.unwrap();
        let emojis: Vec<ColdStorageEmoji> = serde_json::from_slice(&data)?;
        connection.del(key).await?;
        debug!("Worker {} found {} emojis to defrost", index, emojis.len());
        for emoji in emojis {
            let emoji: GuildItem<CachedEmoji> = emoji.into();
            self.0
                .guild_emojis
                .entry(emoji.guild_id)
                .or_insert_with(HashSet::new)
                .insert(emoji.data.id);
            self.0.emojis.insert(emoji.data.id, emoji);
        }
        Ok(())
    }

    async fn defrost_roles(
        &self,
        redis: &ConnectionPool,
        index: usize,
    ) -> Result<(), Box<dyn Error>> {
        let key = format!("cb_cluster_role_chunk_{}", index);
        let mut connection = redis.get().await;
        let data = connection.get(&key).await?.unwrap();
        let roles: Vec<ColdStorageRole> = serde_json::from_slice(&data)?;
        connection.del(key).await?;
        debug!("Worker {} found {} role to defrost", index, roles.len());
        for role in roles {
            let role: GuildItem<Role> = role.into();
            self.0
                .guild_roles
                .entry(role.guild_id)
                .or_insert_with(HashSet::new)
                .insert(role.data.id);
            self.0.roles.insert(role.data.id, role);
        }
        Ok(())
    }

    async fn defrost_unavailable_guilds(
        &self,
        redis: &ConnectionPool,
        index: usize,
    ) -> Result<(), Box<dyn Error>> {
        let key = format!("cb_cluster_unavailable_guilds_chunk_{}", index);
        let mut connection = redis.get().await;
        let data = connection.get(&key).await?.unwrap();
        let guilds: Vec<GuildId> = serde_json::from_slice(&data)?;
        connection.del(key).await?;
        debug!(
            "Worker {} found {} unavailable guilds to defrost",
            index,
            guilds.len()
        );
        for guild in guilds {
            self.0.unavailable_guilds.insert(guild);
        }
        Ok(())
    }

    async fn defrost_current_user(&self, redis: &ConnectionPool) -> Result<(), Box<dyn Error>> {
        let key = "cb_cluster_current_user";
        let mut connection = redis.get().await;
        let data = connection.get(key).await?.unwrap();
        let user: ColdStorageCurrentUser = serde_json::from_slice(&data)?;
        connection.del(key).await?;
        *self.0.current_user.lock().unwrap() = Some(Arc::new(user.into()));
        debug!("Worker found current user to defrost");
        Ok(())
    }

    // ##################
    // ## Freeze cache ##
    // ##################

    pub async fn prepare_cold_resume(
        &self,
        redis: &ConnectionPool,
        resume_data: HashMap<u64, ResumeSession>,
        total_shards: u64,
        shards_per_cluster: u64,
    ) {
        let start = Instant::now();
        // --- Guilds ---
        let guild_chunks = self.0.guilds.len() / 100_000 + 1;
        let mut guild_work_orders = vec![Vec::with_capacity(500); guild_chunks];
        for (i, guard) in self.0.guilds.iter().enumerate() {
            guild_work_orders[i % guild_chunks].push(*guard.key());
        }
        debug!("Freezing {} guilds", self.0.guilds.len());
        let guild_tasks: Vec<_> = guild_work_orders
            .into_iter()
            .enumerate()
            .map(|(i, order)| self._prepare_cold_resume_guild(redis, order, i))
            .collect();
        future::join_all(guild_tasks).await;
        // --- Users ---
        let user_chunks = self.0.users.len() / 100_000 + 1;
        let mut user_work_orders = vec![Vec::with_capacity(50_000); user_chunks];
        for (i, guard) in self.0.users.iter().enumerate() {
            user_work_orders[i % user_chunks].push(*guard.key());
        }
        debug!("Freezing {} users", self.0.users.len());
        let user_tasks: Vec<_> = user_work_orders
            .into_iter()
            .enumerate()
            .map(|(i, chunk)| self._prepare_cold_resume_user(redis, chunk, i))
            .collect();
        future::join_all(user_tasks).await;
        // --- Members ---
        let member_chunks = self.0.members.len() / 100_000 + 1;
        let mut member_work_orders = vec![Vec::with_capacity(50_000); member_chunks];
        for (i, guard) in self.0.members.iter().enumerate() {
            member_work_orders[i % member_chunks].push(*guard.key());
        }
        debug!("Freezing {} members", self.0.members.len());
        let member_tasks: Vec<_> = member_work_orders
            .into_iter()
            .enumerate()
            .map(|(i, chunk)| self._prepare_cold_resume_member(redis, chunk, i))
            .collect();
        future::join_all(member_tasks).await;
        // --- Channels ---
        let channels_len = self
            .0
            .channels_guild
            .iter()
            .filter(|guard| matches!(guard.value().data.deref(), GuildChannel::Text(_)))
            .count();
        let channel_chunks = channels_len / 100_000 + 1;
        let mut channel_work_orders = vec![Vec::with_capacity(50_000); channel_chunks];
        let iter = self
            .0
            .channels_guild
            .iter()
            .filter(|guard| matches!(guard.value().data.deref(), GuildChannel::Text(_)));
        for (i, guard) in iter.enumerate() {
            channel_work_orders[i % channel_chunks].push(*guard.key());
        }
        debug!("Freezing {} channels", channels_len);
        let channel_tasks: Vec<_> = channel_work_orders
            .into_iter()
            .enumerate()
            .map(|(i, chunk)| self._prepare_cold_resume_channel(redis, chunk, i))
            .collect();
        future::join_all(channel_tasks).await;
        // --- Emojis ---
        let emoji_chunks = self.0.emojis.len() / 100_000 + 1;
        let mut emoji_work_orders = vec![Vec::with_capacity(5_000); emoji_chunks];
        for (i, guard) in self.0.emojis.iter().enumerate() {
            emoji_work_orders[i % emoji_chunks].push(*guard.key());
        }
        debug!("Freezing {} emojis", self.0.emojis.len());
        let emoji_tasks: Vec<_> = emoji_work_orders
            .into_iter()
            .enumerate()
            .map(|(i, chunk)| self._prepare_cold_resume_emoji(redis, chunk, i))
            .collect();
        future::join_all(emoji_tasks).await;
        // --- Roles ---
        let role_chunks = self.0.roles.len() / 100_000 + 1;
        let mut role_work_orders = vec![Vec::with_capacity(5_000); role_chunks];
        for (i, guard) in self.0.roles.iter().enumerate() {
            role_work_orders[i % role_chunks].push(*guard.key());
        }
        debug!("Freezing {} roles", self.0.roles.len());
        let role_tasks: Vec<_> = role_work_orders
            .into_iter()
            .enumerate()
            .map(|(i, chunk)| self._prepare_cold_resume_role(redis, chunk, i))
            .collect();
        future::join_all(role_tasks).await;
        // --- Unavailable guilds ---
        let unavailable_guild_chunks = if self.0.unavailable_guilds.len() > 0 {
            let unavailable_guild_chunks = self.0.unavailable_guilds.len() / 100_000 + 1;
            let mut unavailable_guild_work_orders =
                vec![Vec::with_capacity(5_000); unavailable_guild_chunks];
            for (i, guard) in self.0.unavailable_guilds.iter().enumerate() {
                unavailable_guild_work_orders[i % unavailable_guild_chunks].push(*guard.key());
            }
            debug!(
                "Freezing {} unavailable_guild",
                self.0.unavailable_guilds.len()
            );
            let unavailable_guild_tasks: Vec<_> = unavailable_guild_work_orders
                .into_iter()
                .enumerate()
                .map(|(i, chunk)| self._prepare_cold_resume_unavailable_guild(redis, chunk, i))
                .collect();
            future::join_all(unavailable_guild_tasks).await;
            unavailable_guild_chunks
        } else {
            0
        };
        // --- CurrentUser ---
        debug!("Freezing current user");
        self._prepare_cold_resume_current_user(redis).await;

        // ------

        // Prepare resume data
        let map: HashMap<_, _> = resume_data
            .into_iter()
            .map(|(shard_id, info)| (shard_id, (info.session_id, info.sequence)))
            .collect();
        let data = ColdRebootData {
            resume_data: map,
            total_shards,
            guild_chunks,
            shard_count: shards_per_cluster,
            user_chunks,
            member_chunks,
            channel_chunks,
            emoji_chunks,
            role_chunks,
            unavailable_guild_chunks,
        };
        let mut connection = redis.get().await;
        let data_result = connection
            .set_and_expire_seconds(
                "cb_cluster_data",
                &serde_json::to_value(data).unwrap().to_string().into_bytes(),
                STORE_DURATION,
            )
            .await;
        if let Err(why) = data_result {
            warn!("Error while storing cluster data onto redis: {}", why);
        }
        let end = Instant::now();
        info!(
            "Cold resume preparations completed in {}ms",
            (end - start).as_millis()
        );
    }

    async fn _prepare_cold_resume_guild(
        &self,
        redis: &ConnectionPool,
        orders: Vec<GuildId>,
        index: usize,
    ) {
        debug!(
            "Guild dumper {} started freezing {} guilds",
            index,
            orders.len()
        );
        let mut connection = redis.get().await;
        let to_dump: Vec<_> = orders
            .into_iter()
            .filter_map(|key| self.0.guilds.remove(&key))
            .map(|(_, g)| g)
            .collect();
        let serialized = serde_json::to_string(&to_dump).unwrap();
        let dump_task = connection
            .set_and_expire_seconds(
                format!("cb_cluster_guild_chunk_{}", index),
                serialized,
                STORE_DURATION,
            )
            .await;
        if let Err(why) = dump_task {
            debug!(
                "Error while setting redis' `cb_cluster_guild_chunk_{}`: {}",
                index, why
            );
        }
    }

    async fn _prepare_cold_resume_user(
        &self,
        redis: &ConnectionPool,
        chunk: Vec<UserId>,
        index: usize,
    ) -> Result<(), Box<dyn Error>> {
        debug!("Worker {} freezing {} users", index, chunk.len());
        let mut connection = redis.get().await;
        let users: Vec<_> = chunk
            .into_iter()
            .filter_map(|key| self.0.users.remove(&key))
            .map(|(_, (user, guilds))| ColdStorageUser {
                avatar: user.avatar.to_owned(),
                bot: user.bot,
                discriminator: user.discriminator.to_owned(),
                email: user.email.to_owned(),
                flags: user.flags,
                id: user.id,
                locale: user.locale.to_owned(),
                mfa_enabled: user.mfa_enabled,
                name: user.name.to_owned(),
                premium_type: user.premium_type,
                public_flags: user.public_flags,
                system: user.system,
                verified: user.verified,
                guilds,
            })
            .collect();
        let serialized = serde_json::to_string(&users).unwrap();
        let worker_task = connection
            .set_and_expire_seconds(
                format!("cb_cluster_user_chunk_{}", index),
                serialized,
                STORE_DURATION,
            )
            .await;
        if let Err(why) = worker_task {
            debug!(
                "Error while setting redis' `cb_cluster_user_chunk_{}`: {}",
                index, why
            );
        }
        Ok(())
    }

    async fn _prepare_cold_resume_member(
        &self,
        redis: &ConnectionPool,
        orders: Vec<(GuildId, UserId)>,
        index: usize,
    ) {
        debug!(
            "Guild dumper {} started freezing {} members",
            index,
            orders.len()
        );
        let mut connection = redis.get().await;
        let to_dump: Vec<_> = orders
            .into_iter()
            .filter_map(|key| self.0.members.remove(&key))
            .map(|(_, g)| g)
            .map(ColdStorageMember::from)
            .collect();
        let serialized = serde_json::to_string(&to_dump).unwrap();
        let dump_task = connection
            .set_and_expire_seconds(
                format!("cb_cluster_member_chunk_{}", index),
                serialized,
                STORE_DURATION,
            )
            .await;
        if let Err(why) = dump_task {
            debug!(
                "Error while setting redis' `cb_cluster_member_chunk_{}`: {}",
                index, why
            );
        }
    }

    async fn _prepare_cold_resume_channel(
        &self,
        redis: &ConnectionPool,
        orders: Vec<ChannelId>,
        index: usize,
    ) {
        debug!(
            "Guild dumper {} started freezing {} channels",
            index,
            orders.len()
        );
        let mut connection = redis.get().await;
        let to_dump: Vec<_> = orders
            .into_iter()
            .filter_map(|key| self.0.channels_guild.remove(&key))
            .filter_map(|(_, g)| match g.data.deref() {
                GuildChannel::Text(channel) => Some(ColdStorageTextChannel {
                    guild_id: Some(g.guild_id),
                    id: channel.id,
                    kind: channel.kind,
                    last_message_id: channel.last_message_id,
                    last_pin_timestamp: channel.last_pin_timestamp.to_owned(),
                    name: channel.name.to_owned(),
                    nsfw: channel.nsfw,
                    permission_overwrites: channel.permission_overwrites.to_owned(),
                    parent_id: channel.parent_id,
                    position: channel.position,
                    rate_limit_per_user: channel.rate_limit_per_user,
                    topic: channel.topic.to_owned(),
                }),
                _ => None,
            })
            .collect();
        let serialized = serde_json::to_string(&to_dump).unwrap();
        let dump_task = connection
            .set_and_expire_seconds(
                format!("cb_cluster_channel_chunk_{}", index),
                serialized,
                STORE_DURATION,
            )
            .await;
        if let Err(why) = dump_task {
            debug!(
                "Error while setting redis' `cb_cluster_channel_chunk_{}`: {}",
                index, why
            );
        }
    }

    async fn _prepare_cold_resume_emoji(
        &self,
        redis: &ConnectionPool,
        orders: Vec<EmojiId>,
        index: usize,
    ) {
        debug!(
            "Guild dumper {} started freezing {} emojis",
            index,
            orders.len()
        );
        let mut connection = redis.get().await;
        let to_dump: Vec<_> = orders
            .into_iter()
            .filter_map(|key| self.0.emojis.remove(&key))
            .map(|(_, g)| ColdStorageEmoji {
                guild_id: g.guild_id,
                id: g.data.id,
                animated: g.data.animated,
                name: g.data.name.to_owned(),
                require_colons: g.data.require_colons,
                roles: g.data.roles.to_owned(),
                available: g.data.available,
            })
            .collect();
        let serialized = serde_json::to_string(&to_dump).unwrap();
        let dump_task = connection
            .set_and_expire_seconds(
                format!("cb_cluster_emoji_chunk_{}", index),
                serialized,
                STORE_DURATION,
            )
            .await;
        if let Err(why) = dump_task {
            debug!(
                "Error while setting redis' `cb_cluster_emoji_chunk_{}`: {}",
                index, why
            );
        }
    }

    async fn _prepare_cold_resume_role(
        &self,
        redis: &ConnectionPool,
        orders: Vec<RoleId>,
        index: usize,
    ) {
        debug!(
            "Guild dumper {} started freezing {} roles",
            index,
            orders.len()
        );
        let mut connection = redis.get().await;
        let to_dump: Vec<_> = orders
            .into_iter()
            .filter_map(|key| self.0.roles.remove(&key))
            .map(|(_, g)| ColdStorageRole {
                guild_id: g.guild_id,
                color: g.data.color,
                hoist: g.data.hoist,
                id: g.data.id,
                managed: g.data.managed,
                mentionable: g.data.mentionable,
                name: g.data.name.to_owned(),
                permissions: g.data.permissions,
                position: g.data.position,
            })
            .collect();
        let serialized = serde_json::to_string(&to_dump).unwrap();
        let dump_task = connection
            .set_and_expire_seconds(
                format!("cb_cluster_role_chunk_{}", index),
                serialized,
                STORE_DURATION,
            )
            .await;
        if let Err(why) = dump_task {
            debug!(
                "Error while setting redis' `cb_cluster_role_chunk_{}`: {}",
                index, why
            );
        }
    }

    async fn _prepare_cold_resume_unavailable_guild(
        &self,
        redis: &ConnectionPool,
        orders: Vec<GuildId>,
        index: usize,
    ) {
        debug!(
            "Guild dumper {} started freezing {} unavailable guilds",
            index,
            orders.len()
        );
        let mut connection = redis.get().await;
        let to_dump: Vec<_> = orders
            .into_iter()
            .filter_map(|key| self.0.unavailable_guilds.remove(&key))
            .collect();
        let serialized = serde_json::to_string(&to_dump).unwrap();
        let dump_task = connection
            .set_and_expire_seconds(
                format!("cb_cluster_unavailable_guilds_chunk_{}", index),
                serialized,
                STORE_DURATION,
            )
            .await;
        if let Err(why) = dump_task {
            debug!(
                "Error while setting redis' `cb_cluster_unavailable_guilds_chunk_{}`: {}",
                index, why
            );
        }
    }

    async fn _prepare_cold_resume_current_user(&self, redis: &ConnectionPool) {
        if let Some(user) = self.0.current_user.lock().unwrap().deref() {
            let mut connection = redis.get().await;
            let user = ColdStorageCurrentUser {
                avatar: user.avatar.to_owned(),
                discriminator: user.discriminator.to_owned(),
                flags: user.flags,
                id: user.id,
                locale: user.locale.to_owned(),
                mfa_enabled: user.mfa_enabled,
                name: user.name.to_owned(),
                premium_type: user.premium_type,
                public_flags: user.public_flags,
                verified: user.verified,
            };
            let serialized = serde_json::to_string(&user).unwrap();
            let dump_task = connection
                .set_and_expire_seconds("cb_cluster_current_user", serialized, STORE_DURATION)
                .await;
            if let Err(why) = dump_task {
                debug!(
                    "Error while setting redis' `cb_cluster_current_user`: {}",
                    why
                );
            }
        }
    }
}
