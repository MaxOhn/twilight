use crate::{
    CachedGuild, CachedMember, ColdStorageRole, ColdStorageTextChannel, ColdStorageUser, Config,
    GuildItem, InMemoryCache,
};

use deadpool_redis::{
    redis::{AsyncCommands, RedisError},
    Pool, PoolError,
};
use futures::{
    future::{Either, TryFutureExt},
    stream::{FuturesUnordered, StreamExt, TryStreamExt},
};
use serde::{Deserialize, Serialize};
use serde_cbor::Error as SerdeError;
use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fmt,
    time::Instant,
    u64,
};
use twilight_model::{
    channel::GuildChannel,
    guild::Role,
    id::{ChannelId, GuildId, RoleId, UserId},
};

type ResumeSession = (String, u64);

const STORE_DURATION: usize = 240; // seconds

const DATA_KEY: &str = "cb_data";
const GUILD_KEY_PREFIX: &str = "cb_guild_chunk";
const USER_KEY_PREFIX: &str = "cb_user_chunk";
const MEMBER_KEY_PREFIX: &str = "cb_member_chunk";
const CHANNEL_KEY_PREFIX: &str = "cb_channel_chunk";
const ROLE_KEY_PREFIX: &str = "cb_role_chunk";
const CURRENT_USER_KEY: &str = "cb_current_user";

#[derive(Deserialize, Serialize, Debug)]
pub struct ColdRebootData {
    pub resume_data: HashMap<u64, ResumeSession>,
    pub guild_chunks: usize,
    pub user_chunks: usize,
    pub member_chunks: usize,
    pub channel_chunks: usize,
    pub role_chunks: usize,
}

pub type RedisCacheResult<T> = Result<T, RedisCacheError>;

#[derive(Debug)]
pub enum RedisCacheError {
    MissingCurrentUser,
    MissingKey(String),
    Pool(PoolError),
    Redis(RedisError),
    Serde(SerdeError),
    Store(RedisError, String),
}

impl Error for RedisCacheError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::MissingCurrentUser => None,
            Self::MissingKey(_) => None,
            Self::Pool(source) => Some(source),
            Self::Redis(source) => Some(source),
            Self::Serde(source) => Some(source),
            Self::Store(source, _) => Some(source),
        }
    }
}

impl fmt::Display for RedisCacheError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingCurrentUser => f.write_str("missing current user in cache"),
            Self::MissingKey(key) => write!(f, "missing redis key `{}`", key),
            Self::Pool(_) => f.write_str("pool error"),
            Self::Redis(_) => f.write_str("redis error"),
            Self::Serde(_) => f.write_str("serde error"),
            Self::Store(_, key) => write!(f, "failed to set key `{}` into redis", key),
        }
    }
}

impl From<SerdeError> for RedisCacheError {
    fn from(e: SerdeError) -> Self {
        Self::Serde(e)
    }
}

impl From<RedisError> for RedisCacheError {
    fn from(e: RedisError) -> Self {
        Self::Redis(e)
    }
}

impl From<(RedisError, String)> for RedisCacheError {
    fn from((e, key): (RedisError, String)) -> Self {
        Self::Store(e, key)
    }
}

impl From<PoolError> for RedisCacheError {
    fn from(e: PoolError) -> Self {
        Self::Pool(e)
    }
}

impl InMemoryCache {
    /// Check if the cache was frozen into redis.
    /// If so, retrieve and use it; otherwise create an empty initial cache
    pub async fn from_redis(
        redis: &Pool,
        config: Config,
    ) -> (Self, Option<HashMap<u64, ResumeSession>>) {
        let cache = Self::new_with_config(config);

        let mut conn = match redis.get().await {
            Ok(conn) => conn,
            Err(why) => {
                warn!("Failed to get initial redis connection: {}", why);

                return (cache, None);
            }
        };

        let key = DATA_KEY;

        if let Ok(data) = conn.get::<_, Vec<u8>>(key).await {
            if data.is_empty() {
                return (cache, None);
            }

            let mut cold_cache: ColdRebootData = serde_cbor::from_slice(&data).unwrap();

            if let Err(why) = conn.del::<_, u8>(key).await {
                warn!("Failed to remove `{}` element: {}", key, why);
            }

            let mut resume_data = HashMap::new();
            std::mem::swap(&mut resume_data, &mut cold_cache.resume_data);

            let start = Instant::now();

            if let Err((cause, why)) = cache.restore_cold_resume(redis, cold_cache).await {
                error!("Cold resume defrosting failed ({}): {}", cause, why);

                if let Some(source) = why.source() {
                    error!(" - caused by: {}", source);
                }

                cache.clear();

                return (cache, None);
            } else {
                cache
                    .0
                    .metrics
                    .channels_guild
                    .add(cache.0.channels_guild.len() as i64);

                cache.0.metrics.guilds.add(cache.0.guilds.len() as i64);
                cache.0.metrics.members.add(cache.0.members.len() as i64);
                cache.0.metrics.roles.add(cache.0.roles.len() as i64);
                cache.0.metrics.users.add(cache.0.users.len() as i64);

                debug!("Cold resume defrosting completed in {:?}", start.elapsed());

                return (cache, Some(resume_data));
            }
        }

        (cache, None)
    }

    // ###################
    // ## Defrost cache ##
    // ###################

    async fn restore_cold_resume(
        &self,
        redis: &Pool,
        reboot_data: ColdRebootData,
    ) -> Result<(), (&'static str, RedisCacheError)> {
        let mut defrost_futs = FuturesUnordered::new();

        // --- Guilds ---
        let guild_defrosters = (0..reboot_data.guild_chunks)
            .map(|i| self.defrost_guilds(redis, i).map_err(|e| ("guild", e)))
            .map(Either::Left);
        defrost_futs.extend(guild_defrosters);

        // --- Users ---
        let user_defrosters = (0..reboot_data.user_chunks)
            .map(|i| self.defrost_users(redis, i).map_err(|e| ("users", e)))
            .map(Either::Left)
            .map(Either::Right);
        defrost_futs.extend(user_defrosters);

        // --- Members ---
        let member_defrosters = (0..reboot_data.member_chunks)
            .map(|i| self.defrost_members(redis, i).map_err(|e| ("members", e)))
            .map(Either::Left)
            .map(Either::Right)
            .map(Either::Right);
        defrost_futs.extend(member_defrosters);

        // --- Channels ---
        let channel_defrosters = (0..reboot_data.channel_chunks)
            .map(|i| self.defrost_channels(redis, i).map_err(|e| ("channels", e)))
            .map(Either::Left)
            .map(Either::Right)
            .map(Either::Right)
            .map(Either::Right);
        defrost_futs.extend(channel_defrosters);

        // --- Roles ---
        let role_defrosters = (0..reboot_data.role_chunks)
            .map(|i| self.defrost_roles(redis, i).map_err(|e| ("roles", e)))
            .map(Either::Left)
            .map(Either::Right)
            .map(Either::Right)
            .map(Either::Right)
            .map(Either::Right);
        defrost_futs.extend(role_defrosters);

        // --- CurrentUser ---
        let current_user_defroster = self
            .defrost_current_user(redis)
            .map_err(|e| ("current_user", e));
        let current_user_defroster = Either::Right(Either::Right(Either::Right(Either::Right(
            Either::Right(current_user_defroster),
        ))));
        defrost_futs.push(current_user_defroster);

        while defrost_futs.next().await.transpose()?.is_some() {}

        debug!(
            "Cache defrosting complete:\n\
            {} guilds | {} channels_guild | {} users\n\
            {} members | {} roles | {} guild_channels\n\
            {} guild_emojis | {} guilds_members | {} guild_roles",
            self.0.guilds.len(),
            self.0.channels_guild.len(),
            self.0.users.len(),
            self.0.members.len(),
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
        );

        Ok(())
    }

    async fn defrost_guilds(&self, redis: &Pool, index: usize) -> RedisCacheResult<()> {
        let key = format!("{}_{}", GUILD_KEY_PREFIX, index);
        let mut conn = redis.get().await?;
        let data: Vec<u8> = conn.get(&key).await?;

        if data.is_empty() {
            return Err(RedisCacheError::MissingKey(key));
        }

        let guilds: Vec<CachedGuild> = serde_cbor::from_slice(&data)?;
        conn.del(key).await?;

        debug!(
            "Guild worker {} found {} guilds to defrost",
            index,
            guilds.len()
        );

        for guild in guilds {
            self.0.guilds.insert(guild.id, guild);
        }

        Ok(())
    }

    async fn defrost_users(&self, redis: &Pool, index: usize) -> RedisCacheResult<()> {
        let key = format!("{}_{}", USER_KEY_PREFIX, index);
        let mut conn = redis.get().await?;
        let data: Vec<u8> = conn.get(&key).await?;

        if data.is_empty() {
            return Err(RedisCacheError::MissingKey(key));
        }

        let users: Vec<ColdStorageUser> = serde_cbor::from_slice(&data)?;
        conn.del(key).await?;

        debug!(
            "User worker {} found {} users to defrost",
            index,
            users.len()
        );

        for user in users {
            let (user, guilds) = user.into();
            self.0.users.insert(user.id, (user, guilds));
        }

        Ok(())
    }

    async fn defrost_members(&self, redis: &Pool, index: usize) -> RedisCacheResult<()> {
        let key = format!("{}_{}", MEMBER_KEY_PREFIX, index);
        let mut conn = redis.get().await?;
        let data: Vec<u8> = conn.get(&key).await?;

        if data.is_empty() {
            return Err(RedisCacheError::MissingKey(key));
        }

        let members: Vec<CachedMember> = serde_cbor::from_slice(&data)?;
        conn.del(key).await?;

        debug!(
            "Member worker {} found {} members to defrost",
            index,
            members.len()
        );

        for member in members {
            self.0
                .guild_members
                .entry(member.guild_id)
                .or_insert_with(HashSet::new)
                .insert(member.user_id);

            self.0
                .members
                .insert((member.guild_id, member.user_id), member);
        }

        Ok(())
    }

    async fn defrost_channels(&self, redis: &Pool, index: usize) -> RedisCacheResult<()> {
        let key = format!("{}_{}", CHANNEL_KEY_PREFIX, index);
        let mut conn = redis.get().await?;
        let data: Vec<u8> = conn.get(&key).await?;

        if data.is_empty() {
            return Err(RedisCacheError::MissingKey(key));
        }

        let channels: Vec<ColdStorageTextChannel> = serde_cbor::from_slice(&data)?;
        conn.del(key).await?;

        debug!(
            "Channel worker {} found {} textchannels to defrost",
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

    async fn defrost_roles(&self, redis: &Pool, index: usize) -> RedisCacheResult<()> {
        let key = format!("{}_{}", ROLE_KEY_PREFIX, index);
        let mut conn = redis.get().await?;
        let data: Vec<u8> = conn.get(&key).await?;

        if data.is_empty() {
            return Err(RedisCacheError::MissingKey(key));
        }

        let roles: Vec<ColdStorageRole> = serde_cbor::from_slice(&data)?;
        conn.del(key).await?;

        debug!(
            "Role worker {} found {} role to defrost",
            index,
            roles.len()
        );

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

    async fn defrost_current_user(&self, redis: &Pool) -> RedisCacheResult<()> {
        let key = CURRENT_USER_KEY;
        let mut conn = redis.get().await?;
        let data: Vec<u8> = conn.get(key).await?;

        if data.is_empty() {
            return Err(RedisCacheError::MissingKey(key.to_owned()));
        }

        let user = serde_cbor::from_slice(&data)?;
        conn.del(key).await?;

        self.0
            .current_user
            .lock()
            .expect("current user poisoned")
            .replace(user);

        debug!("Found current user to defrost");

        Ok(())
    }

    // ##################
    // ## Freeze cache ##
    // ##################

    /// Dump the cache and its discord sessions into redis
    pub async fn prepare_cold_resume(
        &self,
        redis: &Pool,
        resume_data: HashMap<u64, ResumeSession>,
    ) -> RedisCacheResult<()> {
        let start = Instant::now();
        let mut prepare_futs = FuturesUnordered::new();

        // --- Guilds ---
        let guild_chunks = self.0.guilds.len() / 25_000 + 1;
        let mut guild_work_orders = vec![Vec::with_capacity(10_000); guild_chunks];

        for (i, guard) in self.0.guilds.iter().enumerate() {
            guild_work_orders[i % guild_chunks].push(*guard.key());
        }

        debug!("Freezing {} guilds", self.0.guilds.len());

        let guild_tasks = guild_work_orders
            .into_iter()
            .enumerate()
            .map(|(i, order)| self._prepare_cold_resume_guild(redis, order, i))
            .map(Either::Left);

        prepare_futs.extend(guild_tasks);

        // --- Users ---
        let user_chunks = self.0.users.len() / 100_000 + 1;
        let mut user_work_orders = vec![Vec::with_capacity(50_000); user_chunks];

        for (i, guard) in self.0.users.iter().enumerate() {
            user_work_orders[i % user_chunks].push(*guard.key());
        }

        debug!("Freezing {} users", self.0.users.len());

        let user_tasks = user_work_orders
            .into_iter()
            .enumerate()
            .map(|(i, chunk)| self._prepare_cold_resume_user(redis, chunk, i))
            .map(Either::Left)
            .map(Either::Right);

        prepare_futs.extend(user_tasks);

        // --- Members ---
        let member_chunks = self.0.members.len() / 100_000 + 1;
        let mut member_work_orders = vec![Vec::with_capacity(50_000); member_chunks];

        for (i, guard) in self.0.members.iter().enumerate() {
            member_work_orders[i % member_chunks].push(*guard.key());
        }

        debug!("Freezing {} members", self.0.members.len());

        let member_tasks = member_work_orders
            .into_iter()
            .enumerate()
            .map(|(i, chunk)| self._prepare_cold_resume_member(redis, chunk, i))
            .map(Either::Left)
            .map(Either::Right)
            .map(Either::Right);

        prepare_futs.extend(member_tasks);

        // --- Channels ---
        let channels_len = self
            .0
            .channels_guild
            .iter()
            .filter(|guard| matches!(guard.value().data, GuildChannel::Text(_)))
            .count();

        let channel_chunks = channels_len / 100_000 + 1;
        let mut channel_work_orders = vec![Vec::with_capacity(50_000); channel_chunks];

        let iter = self
            .0
            .channels_guild
            .iter()
            .filter(|guard| matches!(guard.value().data, GuildChannel::Text(_)));

        for (i, guard) in iter.enumerate() {
            channel_work_orders[i % channel_chunks].push(*guard.key());
        }

        debug!("Freezing {} channels", channels_len);

        let channel_tasks = channel_work_orders
            .into_iter()
            .enumerate()
            .map(|(i, chunk)| self._prepare_cold_resume_channel(redis, chunk, i))
            .map(Either::Left)
            .map(Either::Right)
            .map(Either::Right)
            .map(Either::Right);

        prepare_futs.extend(channel_tasks);

        // --- Roles ---
        let role_chunks = self.0.roles.len() / 100_000 + 1;
        let mut role_work_orders = vec![Vec::with_capacity(50_000); role_chunks];

        for (i, guard) in self.0.roles.iter().enumerate() {
            role_work_orders[i % role_chunks].push(*guard.key());
        }

        debug!("Freezing {} roles", self.0.roles.len());

        let role_tasks = role_work_orders
            .into_iter()
            .enumerate()
            .map(|(i, chunk)| self._prepare_cold_resume_role(redis, chunk, i))
            .map(Either::Left)
            .map(Either::Right)
            .map(Either::Right)
            .map(Either::Right)
            .map(Either::Right);

        prepare_futs.extend(role_tasks);

        // --- CurrentUser ---
        debug!("Freezing current user");
        let current_user_task = Either::Right(Either::Right(Either::Right(Either::Right(
            Either::Right(self._prepare_cold_resume_current_user(redis)),
        ))));

        prepare_futs.push(current_user_task);

        // Run all futures
        prepare_futs.try_collect().await?;

        // ------

        // Prepare resume data
        let data = ColdRebootData {
            resume_data,
            guild_chunks,
            user_chunks,
            member_chunks,
            channel_chunks,
            role_chunks,
        };

        let bytes = serde_cbor::to_vec(&data).unwrap();
        let key = DATA_KEY;

        redis
            .get()
            .await?
            .set_ex(key, bytes, STORE_DURATION)
            .await
            .map_err(|e| (e, key.to_owned()))?;

        info!(
            "Cold resume preparations completed in {:?}",
            start.elapsed()
        );

        Ok(())
    }

    async fn _prepare_cold_resume_guild(
        &self,
        redis: &Pool,
        orders: Vec<GuildId>,
        index: usize,
    ) -> RedisCacheResult<()> {
        debug!(
            "Guild dumper {} started freezing {} guilds",
            index,
            orders.len()
        );

        let mut guilds = Vec::with_capacity(orders.len());

        let iter = orders
            .into_iter()
            .filter_map(|key| self.0.guilds.remove(&key))
            .map(|(_, g)| g);

        guilds.extend(iter);

        let serialized = serde_cbor::to_vec(&guilds).unwrap();
        let key = format!("{}_{}", GUILD_KEY_PREFIX, index);

        redis
            .get()
            .await?
            .set_ex(&key, serialized, STORE_DURATION)
            .await
            .map_err(|e| (e, key))?;

        Ok(())
    }

    async fn _prepare_cold_resume_user(
        &self,
        redis: &Pool,
        chunk: Vec<UserId>,
        index: usize,
    ) -> RedisCacheResult<()> {
        debug!("User dumper {} freezing {} users", index, chunk.len());

        let mut users = Vec::with_capacity(chunk.len());

        let iter = chunk
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
            });

        users.extend(iter);

        let serialized = serde_cbor::to_vec(&users).unwrap();
        let key = format!("{}_{}", USER_KEY_PREFIX, index);

        redis
            .get()
            .await?
            .set_ex(&key, serialized, STORE_DURATION)
            .await
            .map_err(|e| (e, key))?;

        Ok(())
    }

    async fn _prepare_cold_resume_member(
        &self,
        redis: &Pool,
        orders: Vec<(GuildId, UserId)>,
        index: usize,
    ) -> RedisCacheResult<()> {
        debug!(
            "Member dumper {} started freezing {} members",
            index,
            orders.len()
        );

        let mut members = Vec::with_capacity(orders.len());

        let iter = orders
            .into_iter()
            .filter_map(|key| self.0.members.remove(&key))
            .map(|(_, g)| g);

        members.extend(iter);

        let serialized = serde_cbor::to_vec(&members).unwrap();
        let key = format!("{}_{}", MEMBER_KEY_PREFIX, index);

        redis
            .get()
            .await?
            .set_ex(&key, serialized, STORE_DURATION)
            .await
            .map_err(|e| (e, key))?;

        Ok(())
    }

    async fn _prepare_cold_resume_channel(
        &self,
        redis: &Pool,
        orders: Vec<ChannelId>,
        index: usize,
    ) -> RedisCacheResult<()> {
        debug!(
            "Channel dumper {} started freezing {} channels",
            index,
            orders.len()
        );

        let mut channels = Vec::with_capacity(orders.len());

        let iter = orders
            .into_iter()
            .filter_map(|key| self.0.channels_guild.remove(&key))
            .filter_map(|(_, g)| match g.data {
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
            });

        channels.extend(iter);

        let serialized = serde_cbor::to_vec(&channels).unwrap();
        let key = format!("{}_{}", CHANNEL_KEY_PREFIX, index);

        redis
            .get()
            .await?
            .set_ex(&key, serialized, STORE_DURATION)
            .await
            .map_err(|e| (e, key))?;

        Ok(())
    }

    async fn _prepare_cold_resume_role(
        &self,
        redis: &Pool,
        orders: Vec<RoleId>,
        index: usize,
    ) -> RedisCacheResult<()> {
        debug!(
            "Role dumper {} started freezing {} roles",
            index,
            orders.len()
        );

        let mut roles = Vec::with_capacity(orders.len());

        let iter = orders
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
            });

        roles.extend(iter);

        let serialized = serde_cbor::to_vec(&roles).unwrap();
        let key = format!("{}_{}", ROLE_KEY_PREFIX, index);

        redis
            .get()
            .await?
            .set_ex(&key, serialized, STORE_DURATION)
            .await
            .map_err(|e| (e, key))?;

        Ok(())
    }

    async fn _prepare_cold_resume_current_user(&self, redis: &Pool) -> RedisCacheResult<()> {
        let user = self
            .current_user()
            .ok_or(RedisCacheError::MissingCurrentUser)?;

        let serialized = serde_cbor::to_vec(&user).unwrap();
        let key = CURRENT_USER_KEY;

        redis
            .get()
            .await?
            .set_ex(key, serialized, STORE_DURATION)
            .await
            .map_err(|e| (e, key.to_owned()))?;

        Ok(())
    }
}
