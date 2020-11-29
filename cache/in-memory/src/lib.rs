//! # twilight-cache-inmemory
//!
//! [![discord badge][]][discord link] [![github badge][]][github link] [![license badge][]][license link] ![rust badge]
//!
//! `twilight-cache-inmemory` is an in-process-memory cache for the
//! [`twilight-rs`] ecosystem. It's responsible for processing events and
//! caching things like guilds, channels, users, and voice states.
//!
//! ## Examples
//!
//! Update a cache with events that come in through the gateway:
//!
//! ```rust,no_run
//! use std::env;
//! use tokio::stream::StreamExt;
//! use twilight_cache_inmemory::InMemoryCache;
//! use twilight_gateway::{Intents, Shard};
//!
//! # #[tokio::main] async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let token = env::var("DISCORD_TOKEN")?;
//! let mut shard = Shard::new(token, Intents::GUILD_MESSAGES);
//! shard.start().await?;
//!
//! // Create a cache, caching up to 10 messages per channel:
//! let cache = InMemoryCache::builder().message_cache_size(10).build();
//!
//! let mut events = shard.events();
//!
//! while let Some(event) = events.next().await {
//!     // Update the cache with the event.
//!     cache.update(&event);
//! }
//! # Ok(()) }
//! ```
//!
//! ## License
//!
//! All first-party crates are licensed under [ISC][LICENSE.md]
//!
//! [LICENSE.md]: https://github.com/twilight-rs/twilight/blob/trunk/LICENSE.md
//! [discord badge]: https://img.shields.io/discord/745809834183753828?color=%237289DA&label=discord%20server&logo=discord&style=for-the-badge
//! [discord link]: https://discord.gg/7jj8n7D
//! [docs:discord:sharding]: https://discord.com/developers/docs/topics/gateway#sharding
//! [github badge]: https://img.shields.io/badge/github-twilight-6f42c1.svg?style=for-the-badge&logo=github
//! [github link]: https://github.com/twilight-rs/twilight
//! [license badge]: https://img.shields.io/badge/license-ISC-blue.svg?style=for-the-badge&logo=pastebin
//! [license link]: https://github.com/twilight-rs/twilight/blob/trunk/LICENSE.md
//! [rust badge]: https://img.shields.io/badge/rust-stable-93450a.svg?style=for-the-badge&logo=rust

#![deny(rust_2018_idioms, unused, warnings)]

#[macro_use]
extern crate log;

pub mod model;

mod builder;
mod config;
mod redis;
mod stats;
mod updates;

pub use self::{
    builder::InMemoryCacheBuilder,
    config::{Config, EventType},
    stats::{CacheStats, CompactGuild, CompactUser, Metrics},
    updates::UpdateCache,
};

use self::model::*;
use dashmap::{mapref::entry::Entry, DashMap, DashSet};
use std::{
    borrow::Cow,
    collections::{BTreeSet, HashSet, VecDeque},
    hash::Hash,
    sync::{atomic::Ordering::Relaxed, Arc, Mutex},
};
use twilight_model::{
    channel::{Group, GuildChannel, PrivateChannel},
    guild::{Emoji, Guild, Member, PartialMember, Role},
    id::{ChannelId, EmojiId, GuildId, MessageId, RoleId, UserId},
    user::{CurrentUser, User},
};

#[derive(Debug)]
struct GuildItem<T> {
    data: Arc<T>,
    guild_id: GuildId,
}

fn upsert_guild_item<K: Eq + Hash, V: PartialEq>(
    map: &DashMap<K, GuildItem<V>>,
    guild_id: GuildId,
    k: K,
    v: V,
) -> Arc<V> {
    match map.entry(k) {
        Entry::Occupied(e) if *e.get().data == v => Arc::clone(&e.get().data),
        Entry::Occupied(mut e) => {
            let v = Arc::new(v);
            e.insert(GuildItem {
                data: Arc::clone(&v),
                guild_id,
            });

            v
        }
        Entry::Vacant(e) => Arc::clone(
            &e.insert(GuildItem {
                data: Arc::new(v),
                guild_id,
            })
            .data,
        ),
    }
}

#[derive(Debug, Default)]
struct InMemoryCacheRef {
    config: Arc<Config>,
    channels_guild: DashMap<ChannelId, GuildItem<GuildChannel>>,
    channels_private: DashMap<UserId, Arc<PrivateChannel>>,
    // So long as the lock isn't held across await or panic points this is fine.
    current_user: Mutex<Option<Arc<CurrentUser>>>,
    emojis: DashMap<EmojiId, GuildItem<CachedEmoji>>,
    groups: DashMap<ChannelId, Arc<Group>>,
    guilds: DashMap<GuildId, Arc<CachedGuild>>,
    guild_channels: DashMap<GuildId, HashSet<ChannelId>>,
    guild_emojis: DashMap<GuildId, HashSet<EmojiId>>,
    guild_members: DashMap<GuildId, HashSet<UserId>>,
    guild_roles: DashMap<GuildId, HashSet<RoleId>>,
    members: DashMap<(GuildId, UserId), Arc<CachedMember>>,
    messages: DashMap<ChannelId, VecDeque<Arc<CachedMessage>>>,
    roles: DashMap<RoleId, GuildItem<Role>>,
    unavailable_guilds: DashSet<GuildId>,
    users: DashMap<UserId, (Arc<User>, BTreeSet<GuildId>)>,
    metrics: Arc<Metrics>,
}

/// A thread-safe, in-memory-process cache of Discord data. It can be cloned and
/// sent to other threads.
///
/// This is an implementation of a cache designed to be used by only the
/// current process.
///
/// Events will only be processed if they are properly expressed with
/// [`Intents`]; refer to function-level documentation for more details.
///
/// # Cloning
///
/// The cache internally wraps its data within an Arc. This means that the cache
/// can be cloned and passed around tasks and threads cheaply.
///
/// # Design and Performance
///
/// The defining characteristic of this cache is that returned types (such as a
/// guild or user) do not use locking for access. The internals of the cache use
/// a concurrent map for mutability and the returned types themselves are Arcs.
/// If a user is retrieved from the cache, an `Arc<User>` is returned. If a
/// reference to that user is held but the cache updates the user, the reference
/// held by you will be outdated, but still exist.
///
/// The intended use is that data is held outside the cache for only as long
/// as necessary, where the state of the value at that point time doesn't need
/// to be up-to-date. If you need to ensure you always have the most up-to-date
/// "version" of a cached resource, then you can re-retrieve it whenever you use
/// it: retrieval operations are extremely cheap.
///
/// For example, say you're deleting some of the guilds of a channel. You'll
/// probably need the guild to do that, so you retrieve it from the cache. You
/// can then use the guild to update all of the channels, because for most use
/// cases you don't need the guild to be up-to-date in real time, you only need
/// its state at that *point in time* or maybe across the lifetime of an
/// operation. If you need the guild to always be up-to-date between operations,
/// then the intent is that you keep getting it from the cache.
///
/// [`Intents`]: ../twilight_model/gateway/struct.Intents.html
#[derive(Clone, Debug, Default)]
pub struct InMemoryCache(Arc<InMemoryCacheRef>);

/// Implemented methods and types for the cache.
impl InMemoryCache {
    /// Creates a new, empty cache.
    ///
    /// # Examples
    ///
    /// Creating a new `InMemoryCache` with a custom configuration, limiting
    /// the message cache to 50 messages per channel:
    ///
    /// ```
    /// use twilight_cache_inmemory::InMemoryCache;
    ///
    /// let cache = InMemoryCache::builder().message_cache_size(50).build();
    /// ```
    pub fn new() -> Self {
        Self::default()
    }

    fn new_with_config(config: Config) -> Self {
        Self(Arc::new(InMemoryCacheRef {
            config: Arc::new(config),
            ..Default::default()
        }))
    }

    /// Create a new builder to configure and construct an in-memory cache.
    pub fn builder() -> InMemoryCacheBuilder {
        InMemoryCacheBuilder::new()
    }

    /// Returns a copy of the config cache.
    pub fn config(&self) -> Config {
        (*self.0.config).clone()
    }

    /// Update the cache with an event from the gateway.
    pub fn update(&self, value: &impl UpdateCache) {
        value.update(self);
    }

    /// Gets a channel by ID.
    ///
    /// This is an O(1) operation. This requires the [`GUILDS`] intent.
    ///
    /// [`GUILDS`]: ../twilight_model/gateway/struct.Intents.html#associatedconstant.GUILDS
    pub fn guild_channel(&self, channel_id: ChannelId) -> Option<Arc<GuildChannel>> {
        self.0
            .channels_guild
            .get(&channel_id)
            .map(|x| Arc::clone(&x.data))
    }

    /// Gets the current user.
    ///
    /// This is an O(1) operation.
    pub fn current_user(&self) -> Option<Arc<CurrentUser>> {
        self.0
            .current_user
            .lock()
            .expect("current user poisoned")
            .clone()
    }

    /// Gets an emoji by ID.
    ///
    /// This is an O(1) operation. This requires the [`GUILD_EMOJIS`] intent.
    ///
    /// [`GUILD_EMOJIS`]: ../twilight_model/gateway/struct.Intents.html#associatedconstant.GUILD_EMOJIS
    pub fn emoji(&self, emoji_id: EmojiId) -> Option<Arc<CachedEmoji>> {
        self.0.emojis.get(&emoji_id).map(|x| Arc::clone(&x.data))
    }

    /// Gets a group by ID.
    ///
    /// This is an O(1) operation.
    pub fn group(&self, channel_id: ChannelId) -> Option<Arc<Group>> {
        self.0
            .groups
            .get(&channel_id)
            .map(|r| Arc::clone(r.value()))
    }

    /// Gets a guild by ID.
    ///
    /// This is an O(1) operation. This requires the [`GUILDS`] intent.
    ///
    /// [`GUILDS`]: ../twilight_model/gateway/struct.Intents.html#associatedconstant.GUILDS
    pub fn guild(&self, guild_id: GuildId) -> Option<Arc<CachedGuild>> {
        self.0.guilds.get(&guild_id).map(|r| Arc::clone(r.value()))
    }

    /// Gets the set of channels in a guild.
    ///
    /// This is a O(m) operation, where m is the amount of channels in the
    /// guild. This requires the [`GUILDS`] intent.
    ///
    /// [`GUILDS`]: ../twilight_model/gateway/struct.Intents.html#associatedconstant.GUILDS
    pub fn guild_channels(&self, guild_id: GuildId) -> Option<HashSet<ChannelId>> {
        self.0
            .guild_channels
            .get(&guild_id)
            .map(|r| r.value().clone())
    }

    /// Gets the set of emojis in a guild.
    ///
    /// This is a O(m) operation, where m is the amount of emojis in the guild.
    /// This requires both the [`GUILDS`] and [`GUILD_EMOJIS`] intents.
    ///
    /// [`GUILDS`]: ../twilight_model/gateway/struct.Intents.html#associatedconstant.GUILDS
    /// [`GUILD_EMOJIS`]: ../twilight_model/gateway/struct.Intents.html#associatedconstant.GUILD_EMOJIS
    pub fn guild_emojis(&self, guild_id: GuildId) -> Option<HashSet<EmojiId>> {
        self.0
            .guild_emojis
            .get(&guild_id)
            .map(|r| r.value().clone())
    }

    /// Gets the set of members in a guild.
    ///
    /// This list may be incomplete if not all members have been cached.
    ///
    /// This is a O(m) operation, where m is the amount of members in the guild.
    /// This requires the [`GUILD_MEMBERS`] intent.
    ///
    /// [`GUILD_MEMBERS`]: ../twilight_model/gateway/struct.Intents.html#associatedconstant.GUILD_MEMBERS
    pub fn guild_members(&self, guild_id: GuildId) -> Option<HashSet<UserId>> {
        self.0
            .guild_members
            .get(&guild_id)
            .map(|r| r.value().clone())
    }

    /// Gets the set of roles in a guild.
    ///
    /// This is a O(m) operation, where m is the amount of roles in the guild.
    /// This requires the [`GUILDS`] intent.
    ///
    /// [`GUILDS`]: ../twilight_model/gateway/struct.Intents.html#associatedconstant.GUILDS
    pub fn guild_roles(&self, guild_id: GuildId) -> Option<HashSet<RoleId>> {
        self.0.guild_roles.get(&guild_id).map(|r| r.value().clone())
    }

    /// Gets a member by guild ID and user ID.
    ///
    /// This is an O(1) operation. This requires the [`GUILD_MEMBERS`] intent.
    ///
    /// [`GUILD_MEMBERS`]: ../twilight_model/gateway/struct.Intents.html#associatedconstant.GUILD_MEMBERS
    pub fn member(&self, guild_id: GuildId, user_id: UserId) -> Option<Arc<CachedMember>> {
        self.0
            .members
            .get(&(guild_id, user_id))
            .map(|r| Arc::clone(r.value()))
    }

    /// Gets the latest message by channel ID that returns `Some` through the given function.
    ///
    /// This is an O(n) operation. This requires one or both of the
    /// [`GUILD_MESSAGES`] or [`DIRECT_MESSAGES`] intents.
    pub fn message_extract<T>(
        &self,
        channel_id: ChannelId,
        f: impl Fn(&CachedMessage) -> Option<T>,
    ) -> Option<T> {
        let channel = self.0.messages.get(&channel_id)?;

        channel.iter().find_map(|msg| f(msg))
    }

    /// Gets the earliest message of a channel ID.
    ///
    /// This is an O(1) operation. This requires one or both of the
    /// [`GUILD_MESSAGES`] or [`DIRECT_MESSAGES`] intents.
    pub fn first_message(&self, channel_id: ChannelId) -> Option<MessageId> {
        let channel = self.0.messages.get(&channel_id)?;

        channel.iter().next_back().map(|msg| msg.id)
    }

    /// Gets the latest message of a channel ID.
    ///
    /// This is an O(1) operation. This requires one or both of the
    /// [`GUILD_MESSAGES`] or [`DIRECT_MESSAGES`] intents.
    pub fn last_message(&self, channel_id: ChannelId) -> Option<MessageId> {
        let channel = self.0.messages.get(&channel_id)?;

        channel.iter().next().map(|msg| msg.id)
    }

    /// Gets a private channel by ID.
    ///
    /// This is an O(1) operation. This requires the [`DIRECT_MESSAGES`] intent.
    ///
    /// [`DIRECT_MESSAGES`]: ../twilight_model/gateway/struct.Intents.html#associatedconstant.DIRECT_MESSAGES
    pub fn private_channel(&self, user_id: UserId) -> Option<Arc<PrivateChannel>> {
        self.0
            .channels_private
            .get(&user_id)
            .map(|r| Arc::clone(r.value()))
    }

    /// Gets a role by ID.
    ///
    /// This is an O(1) operation. This requires the [`GUILDS`] intent.
    ///
    /// [`GUILDS`]: ../twilight_model/gateway/struct.Intents.html#associatedconstant.GUILDS
    pub fn role(&self, role_id: RoleId) -> Option<Arc<Role>> {
        self.0
            .roles
            .get(&role_id)
            .map(|role| Arc::clone(&role.data))
    }

    /// Gets a user by ID.
    ///
    /// This is an O(1) operation. This requires the [`GUILD_MEMBERS`] intent.
    ///
    /// [`GUILD_MEMBERS`]: ../twilight_model/gateway/struct.Intents.html#associatedconstant.GUILD_MEMBERS
    pub fn user(&self, user_id: UserId) -> Option<Arc<User>> {
        self.0.users.get(&user_id).map(|r| Arc::clone(&r.0))
    }

    /// Clears the entire state of the Cache. This is equal to creating a new
    /// empty Cache.
    pub fn clear(&self) {
        self.0.channels_guild.clear();
        self.0
            .current_user
            .lock()
            .expect("current user poisoned")
            .take();
        self.0.emojis.clear();
        self.0.guilds.clear();
        self.0.roles.clear();
        self.0.users.clear();
    }

    fn cache_current_user(&self, mut current_user: CurrentUser) {
        let mut user = self.0.current_user.lock().expect("current user poisoned");

        if let Some(mut user) = user.as_mut() {
            if let Some(user) = Arc::get_mut(&mut user) {
                std::mem::swap(user, &mut current_user);

                return;
            }
        }

        *user = Some(Arc::new(current_user));
    }

    fn cache_guild_channels(
        &self,
        guild_id: GuildId,
        guild_channels: impl IntoIterator<Item = GuildChannel>,
    ) {
        for channel in guild_channels {
            self.cache_guild_channel(guild_id, channel);
        }
    }

    fn cache_guild_channel(
        &self,
        guild_id: GuildId,
        mut channel: GuildChannel,
    ) -> Arc<GuildChannel> {
        match channel {
            GuildChannel::Category(ref mut c) => {
                c.guild_id.replace(guild_id);
            }
            GuildChannel::Text(ref mut c) => {
                c.guild_id.replace(guild_id);
            }
            GuildChannel::Voice(ref mut c) => {
                c.guild_id.replace(guild_id);
            }
        }

        let id = channel.id();
        self.0
            .guild_channels
            .entry(guild_id)
            .or_default()
            .insert(id);

        match self.0.channels_guild.entry(id) {
            Entry::Occupied(e) if *e.get().data == channel => Arc::clone(&e.get().data),
            Entry::Occupied(mut e) => {
                let channel = Arc::new(channel);
                e.insert(GuildItem {
                    data: Arc::clone(&channel),
                    guild_id,
                });

                channel
            }
            Entry::Vacant(e) => {
                self.0.metrics.channels_guild.fetch_add(1, Relaxed);
                let item = GuildItem {
                    data: Arc::new(channel),
                    guild_id,
                };
                Arc::clone(&e.insert(item).data)
            }
        }
    }

    fn cache_emoji(&self, guild_id: GuildId, emoji: Emoji) {
        match self.0.emojis.get(&emoji.id) {
            Some(e) if *e.data == emoji => return,
            Some(_) => {}
            None => {
                self.0.metrics.emojis.fetch_add(1, Relaxed);
            }
        }

        if let Some(user) = emoji.user {
            self.cache_user(Cow::Owned(user), Some(guild_id));
        }

        let cached = Arc::new(CachedEmoji {
            id: emoji.id,
            animated: emoji.animated,
            name: emoji.name,
            require_colons: emoji.require_colons,
            roles: emoji.roles,
            available: emoji.available,
        });

        self.0.emojis.insert(
            cached.id,
            GuildItem {
                data: cached,
                guild_id,
            },
        );

        self.0
            .guild_emojis
            .entry(guild_id)
            .or_default()
            .insert(emoji.id);
    }

    fn cache_emojis(&self, guild_id: GuildId, emojis: impl IntoIterator<Item = Emoji>) {
        for emoji in emojis {
            self.cache_emoji(guild_id, emoji);
        }
    }

    fn cache_group(&self, group: Group) -> Arc<Group> {
        match self.0.groups.entry(group.id) {
            Entry::Occupied(e) if **e.get() == group => Arc::clone(e.get()),
            Entry::Occupied(mut e) => {
                let group = Arc::new(group);
                e.insert(Arc::clone(&group));

                group
            }
            Entry::Vacant(e) => {
                let group = Arc::new(group);
                e.insert(Arc::clone(&group));

                group
            }
        }
    }

    fn cache_guild(&self, guild: Guild) {
        // The map and set creation needs to occur first, so caching states and objects
        // always has a place to put them.
        self.0
            .guild_channels
            .entry(guild.id)
            .and_modify(|channels| {
                let _ = self
                    .0
                    .metrics
                    .channels_guild
                    .fetch_update(Relaxed, Relaxed, |n| Some(n.saturating_sub(channels.len())));
                channels.clear();
            })
            .or_default();
        self.0
            .guild_emojis
            .entry(guild.id)
            .and_modify(|emojis| {
                let _ = self
                    .0
                    .metrics
                    .emojis
                    .fetch_update(Relaxed, Relaxed, |n| Some(n.saturating_sub(emojis.len())));
                emojis.clear();
            })
            .or_default();
        self.0
            .guild_members
            .entry(guild.id)
            .and_modify(|members| {
                let _ = self
                    .0
                    .metrics
                    .members
                    .fetch_update(Relaxed, Relaxed, |n| Some(n.saturating_sub(members.len())));
                members.clear();
            })
            .or_default();
        self.0
            .guild_roles
            .entry(guild.id)
            .and_modify(|roles| {
                let _ = self
                    .0
                    .metrics
                    .roles
                    .fetch_update(Relaxed, Relaxed, |n| Some(n.saturating_sub(roles.len())));
                roles.clear();
            })
            .or_default();

        self.cache_guild_channels(guild.id, guild.channels.into_iter().map(|(_, v)| v));
        self.cache_emojis(guild.id, guild.emojis.into_iter().map(|(_, v)| v));
        self.cache_members(guild.id, guild.members.into_iter().map(|(_, v)| v));
        self.cache_roles(guild.id, guild.roles.into_iter().map(|(_, v)| v));

        let guild = CachedGuild {
            id: guild.id,
            icon: guild.icon,
            max_members: guild.max_members,
            member_count: guild.member_count,
            name: guild.name,
            owner_id: guild.owner_id,
            permissions: guild.permissions,
            system_channel_id: guild.system_channel_id,
            unavailable: guild.unavailable,
        };

        if self.0.unavailable_guilds.remove(&guild.id).is_some() {
            let _ = self
                .0
                .metrics
                .unavailable_guilds
                .fetch_update(Relaxed, Relaxed, |n| Some(n.saturating_sub(1)));
        }
        if self.0.guilds.insert(guild.id, Arc::new(guild)).is_none() {
            self.0.metrics.guilds.fetch_add(1, Relaxed);
        }
    }

    fn cache_member(&self, guild_id: GuildId, member: Member) {
        let member_id = member.user.id;
        let id = (guild_id, member_id);
        match self.0.members.get(&id) {
            Some(m) if **m == member => return,
            Some(_) => {}
            None => {
                self.0.metrics.members.fetch_add(1, Relaxed);
            }
        }

        let user = self.cache_user(Cow::Owned(member.user), Some(guild_id));
        let cached = Arc::new(CachedMember {
            user_id: user.id,
            guild_id,
            nick: member.nick,
            roles: member.roles,
            user,
        });
        self.0.members.insert(id, cached);
        self.0
            .guild_members
            .entry(guild_id)
            .or_default()
            .insert(member_id);
    }

    fn cache_borrowed_partial_member(
        &self,
        guild_id: GuildId,
        member: &PartialMember,
        user: Arc<User>,
    ) {
        let id = (guild_id, user.id);
        match self.0.members.get(&id) {
            Some(m) if **m == member => return,
            Some(_) => {}
            None => {
                self.0.metrics.members.fetch_add(1, Relaxed);
            }
        }

        self.0
            .guild_members
            .entry(guild_id)
            .or_default()
            .insert(user.id);

        let cached = Arc::new(CachedMember {
            guild_id,
            nick: member.nick.to_owned(),
            roles: member.roles.to_owned(),
            user_id: user.id,
            user,
        });
        self.0.members.insert(id, cached);
    }

    fn cache_members(&self, guild_id: GuildId, members: impl IntoIterator<Item = Member>) {
        for member in members {
            self.cache_member(guild_id, member);
        }
    }

    pub fn cache_private_channel(&self, private_channel: PrivateChannel) -> Arc<PrivateChannel> {
        let id = private_channel
            .recipients
            .first()
            .expect("no recipients for private channel")
            .id;

        let entry = self.0.channels_private.get(&id);
        if entry.is_none() {
            self.0.metrics.channels_private.fetch_add(1, Relaxed);
        }

        match entry {
            Some(c) if **c == private_channel => Arc::clone(c.value()),
            Some(_) | None => {
                let v = Arc::new(private_channel);
                self.0.channels_private.insert(id, Arc::clone(&v));

                v
            }
        }
    }

    fn cache_roles(&self, guild_id: GuildId, roles: impl IntoIterator<Item = Role>) {
        for role in roles {
            self.cache_role(guild_id, role);
        }
    }

    fn cache_role(&self, guild_id: GuildId, role: Role) -> Arc<Role> {
        self.0
            .guild_roles
            .entry(guild_id)
            .or_default()
            .insert(role.id);

        match self.0.roles.entry(role.id) {
            Entry::Occupied(e) if *e.get().data == role => Arc::clone(&e.get().data),
            Entry::Occupied(mut e) => {
                let role = Arc::new(role);
                e.insert(GuildItem {
                    data: Arc::clone(&role),
                    guild_id,
                });

                role
            }
            Entry::Vacant(e) => {
                self.0.metrics.roles.fetch_add(1, Relaxed);
                let item = GuildItem {
                    data: Arc::new(role),
                    guild_id,
                };
                Arc::clone(&e.insert(item).data)
            }
        }
    }

    fn cache_user(&self, user: Cow<'_, User>, guild_id: Option<GuildId>) -> Arc<User> {
        match self.0.users.get_mut(&user.id) {
            Some(mut u) if *u.0 == *user => {
                if let Some(guild_id) = guild_id {
                    u.1.insert(guild_id);
                }

                return Arc::clone(&u.value().0);
            }
            Some(_) => {}
            None => {
                self.0.metrics.users.fetch_add(1, Relaxed);
            }
        }
        let user = Arc::new(user.into_owned());
        if let Some(guild_id) = guild_id {
            let mut guild_id_set = BTreeSet::new();
            guild_id_set.insert(guild_id);
            self.0
                .users
                .insert(user.id, (Arc::clone(&user), guild_id_set));
        }

        user
    }

    fn delete_group(&self, channel_id: ChannelId) {
        self.0.groups.remove(&channel_id);
    }

    fn unavailable_guild(&self, guild_id: GuildId) {
        if self.0.unavailable_guilds.insert(guild_id) {
            self.0.metrics.unavailable_guilds.fetch_add(1, Relaxed);
        }
        if self.0.guilds.remove(&guild_id).is_some() {
            let _ = self
                .0
                .metrics
                .guilds
                .fetch_update(Relaxed, Relaxed, |n| Some(n.saturating_sub(1)));
        }
    }

    /// Delete a guild channel from the cache.
    ///
    /// The guild channel data itself and the channel entry in its guild's list
    /// of channels will be deleted.
    fn delete_guild_channel(&self, channel_id: ChannelId) {
        let guild_id = match self.0.channels_guild.remove(&channel_id) {
            Some(entry) => entry.1.guild_id,
            None => return,
        };

        if let Some(mut guild_channels) = self.0.guild_channels.get_mut(&guild_id) {
            if guild_channels.remove(&channel_id) {
                let _ = self
                    .0
                    .metrics
                    .channels_guild
                    .fetch_update(Relaxed, Relaxed, |n| Some(n.saturating_sub(1)));
            }
        }
    }

    fn delete_role(&self, role_id: RoleId) {
        let role = match self.0.roles.remove(&role_id).map(|(_, v)| v) {
            Some(role) => role,
            None => return,
        };

        if let Some(mut roles) = self.0.guild_roles.get_mut(&role.guild_id) {
            if roles.remove(&role_id) {
                let _ = self
                    .0
                    .metrics
                    .roles
                    .fetch_update(Relaxed, Relaxed, |n| Some(n.saturating_sub(1)));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::InMemoryCache;
    use std::{borrow::Cow, collections::HashMap};
    use twilight_model::{
        channel::{ChannelType, GuildChannel, TextChannel},
        gateway::payload::{MemberRemove, RoleDelete},
        guild::{
            DefaultMessageNotificationLevel, Emoji, ExplicitContentFilter, Guild, Member, MfaLevel,
            Permissions, PremiumTier, Role, SystemChannelFlags, VerificationLevel,
        },
        id::{ChannelId, EmojiId, GuildId, RoleId, UserId},
        user::{CurrentUser, User},
    };

    fn current_user(id: u64) -> CurrentUser {
        CurrentUser {
            avatar: None,
            bot: true,
            discriminator: "9876".to_owned(),
            email: None,
            id: UserId(id),
            mfa_enabled: true,
            name: "test".to_owned(),
            verified: Some(true),
            premium_type: None,
            public_flags: None,
            flags: None,
            locale: None,
        }
    }

    fn emoji(id: EmojiId, user: Option<User>) -> Emoji {
        Emoji {
            animated: false,
            available: true,
            id,
            managed: false,
            name: "test".to_owned(),
            require_colons: true,
            roles: Vec::new(),
            user,
        }
    }

    fn member(id: UserId, guild_id: GuildId) -> Member {
        Member {
            deaf: false,
            guild_id,
            hoisted_role: None,
            joined_at: None,
            mute: false,
            nick: None,
            premium_since: None,
            roles: Vec::new(),
            user: user(id),
        }
    }

    fn role(id: RoleId) -> Role {
        Role {
            color: 0,
            hoist: false,
            id,
            managed: false,
            mentionable: false,
            name: "test".to_owned(),
            permissions: Permissions::empty(),
            position: 0,
        }
    }

    fn user(id: UserId) -> User {
        User {
            avatar: None,
            bot: false,
            discriminator: "0001".to_owned(),
            email: None,
            flags: None,
            id,
            locale: None,
            mfa_enabled: None,
            name: "user".to_owned(),
            premium_type: None,
            public_flags: None,
            system: None,
            verified: None,
        }
    }

    /// Test retrieval of the current user, notably that it doesn't simply
    /// panic or do anything funny. This is the only synchronous mutex that we
    /// might have trouble with across await points if we're not careful.
    #[test]
    fn test_current_user_retrieval() {
        let cache = InMemoryCache::new();
        assert!(cache.current_user().is_none());
        cache.cache_current_user(current_user(1));
        assert!(cache.current_user().is_some());
    }

    #[test]
    fn test_guild_create_channels_have_guild_ids() {
        let mut channels = HashMap::new();
        channels.insert(
            ChannelId(111),
            GuildChannel::Text(TextChannel {
                id: ChannelId(111),
                guild_id: None,
                kind: ChannelType::GuildText,
                last_message_id: None,
                last_pin_timestamp: None,
                name: "guild channel with no guild id".to_owned(),
                nsfw: true,
                permission_overwrites: Vec::new(),
                parent_id: None,
                position: 1,
                rate_limit_per_user: None,
                topic: None,
            }),
        );

        let guild = Guild {
            id: GuildId(123),
            afk_channel_id: None,
            afk_timeout: 300,
            application_id: None,
            banner: None,
            channels,
            default_message_notifications: DefaultMessageNotificationLevel::Mentions,
            description: None,
            discovery_splash: None,
            emojis: HashMap::new(),
            explicit_content_filter: ExplicitContentFilter::AllMembers,
            features: vec![],
            icon: None,
            joined_at: Some("".to_owned()),
            large: false,
            lazy: Some(true),
            max_members: Some(50),
            max_presences: Some(100),
            member_count: Some(25),
            members: HashMap::new(),
            mfa_level: MfaLevel::Elevated,
            name: "this is a guild".to_owned(),
            owner: Some(false),
            owner_id: UserId(456),
            permissions: Some(Permissions::SEND_MESSAGES),
            preferred_locale: "en-GB".to_owned(),
            premium_subscription_count: Some(0),
            premium_tier: PremiumTier::None,
            presences: HashMap::new(),
            region: "us-east".to_owned(),
            roles: HashMap::new(),
            splash: None,
            system_channel_id: None,
            system_channel_flags: SystemChannelFlags::SUPPRESS_JOIN_NOTIFICATIONS,
            rules_channel_id: None,
            unavailable: false,
            verification_level: VerificationLevel::VeryHigh,
            voice_states: HashMap::new(),
            vanity_url_code: None,
            widget_channel_id: None,
            widget_enabled: None,
            max_video_channel_users: None,
            approximate_member_count: None,
            approximate_presence_count: None,
        };

        let cache = InMemoryCache::new();
        cache.cache_guild(guild);

        let channel = cache.guild_channel(ChannelId(111)).unwrap();

        // The channel was given to the cache without a guild ID, but because
        // it's part of a guild create, the cache can automatically attach the
        // guild ID to it. So now, the channel's guild ID is present with the
        // correct value.
        match *channel {
            GuildChannel::Text(ref c) => {
                assert_eq!(Some(GuildId(123)), c.guild_id);
            }
            _ => assert!(false, "{:?}", channel),
        }
    }

    #[test]
    fn test_syntax_update() {
        let cache = InMemoryCache::new();
        cache.update(&RoleDelete {
            guild_id: GuildId(0),
            role_id: RoleId(1),
        });
    }

    #[test]
    fn test_cache_user_guild_state() {
        let user_id = UserId(2);
        let cache = InMemoryCache::new();
        cache.cache_user(Cow::Owned(user(user_id)), Some(GuildId(1)));

        // Test the guild's ID is the only one in the user's set of guilds.
        {
            let user = cache.0.users.get(&user_id).unwrap();
            assert!(user.1.contains(&GuildId(1)));
            assert_eq!(1, user.1.len());
        }

        // Test that a second guild will cause 2 in the set.
        cache.cache_user(Cow::Owned(user(user_id)), Some(GuildId(3)));

        {
            let user = cache.0.users.get(&user_id).unwrap();
            assert!(user.1.contains(&GuildId(3)));
            assert_eq!(2, user.1.len());
        }

        // Test that removing a user from a guild will cause the ID to be
        // removed from the set, leaving the other ID.
        cache.update(&MemberRemove {
            guild_id: GuildId(3),
            user: user(user_id),
        });

        {
            let user = cache.0.users.get(&user_id).unwrap();
            assert!(!user.1.contains(&GuildId(3)));
            assert_eq!(1, user.1.len());
        }

        // Test that removing the user from its last guild removes the user's
        // entry.
        cache.update(&MemberRemove {
            guild_id: GuildId(1),
            user: user(user_id),
        });
        assert!(!cache.0.users.contains_key(&user_id));
    }

    #[test]
    fn test_cache_role() {
        let cache = InMemoryCache::new();

        // Single inserts
        {
            // The role ids for the guild with id 1
            let guild_1_role_ids = (1..=10).map(RoleId).collect::<Vec<_>>();
            // Map the role ids to a test role
            let guild_1_roles = guild_1_role_ids
                .iter()
                .copied()
                .map(role)
                .collect::<Vec<_>>();
            // Cache all the roles using cache role
            for role in guild_1_roles.clone() {
                cache.cache_role(GuildId(1), role);
            }

            // Check for the cached guild role ids
            let cached_roles = cache.guild_roles(GuildId(1)).unwrap();
            assert_eq!(cached_roles.len(), guild_1_role_ids.len());
            assert!(guild_1_role_ids.iter().all(|id| cached_roles.contains(id)));

            // Check for the cached role
            assert!(guild_1_roles
                .into_iter()
                .all(|role| *cache.role(role.id).expect("Role missing from cache") == role))
        }

        // Bulk inserts
        {
            // The role ids for the guild with id 2
            let guild_2_role_ids = (101..=110).map(RoleId).collect::<Vec<_>>();
            // Map the role ids to a test role
            let guild_2_roles = guild_2_role_ids
                .iter()
                .copied()
                .map(role)
                .collect::<Vec<_>>();
            // Cache all the roles using cache roles
            cache.cache_roles(GuildId(2), guild_2_roles.clone());

            // Check for the cached guild role ids
            let cached_roles = cache.guild_roles(GuildId(2)).unwrap();
            assert_eq!(cached_roles.len(), guild_2_role_ids.len());
            assert!(guild_2_role_ids.iter().all(|id| cached_roles.contains(id)));

            // Check for the cached role
            assert!(guild_2_roles
                .into_iter()
                .all(|role| *cache.role(role.id).expect("Role missing from cache") == role))
        }
    }

    #[test]
    fn test_cache_guild_member() {
        let cache = InMemoryCache::new();

        // Single inserts
        {
            let guild_1_user_ids = (1..=10).map(UserId).collect::<Vec<_>>();
            let guild_1_members = guild_1_user_ids
                .iter()
                .copied()
                .map(|id| member(id, GuildId(1)))
                .collect::<Vec<_>>();

            for member in guild_1_members {
                cache.cache_member(GuildId(1), member);
            }

            // Check for the cached guild members ids
            let cached_roles = cache.guild_members(GuildId(1)).unwrap();
            assert_eq!(cached_roles.len(), guild_1_user_ids.len());
            assert!(guild_1_user_ids.iter().all(|id| cached_roles.contains(id)));

            // Check for the cached members
            assert!(guild_1_user_ids
                .iter()
                .all(|id| cache.member(GuildId(1), *id).is_some()));

            // Check for the cached users
            assert!(guild_1_user_ids.iter().all(|id| cache.user(*id).is_some()));
        }

        // Bulk inserts
        {
            let guild_2_user_ids = (1..=10).map(UserId).collect::<Vec<_>>();
            let guild_2_members = guild_2_user_ids
                .iter()
                .copied()
                .map(|id| member(id, GuildId(2)))
                .collect::<Vec<_>>();
            cache.cache_members(GuildId(2), guild_2_members);

            // Check for the cached guild members ids
            let cached_roles = cache.guild_members(GuildId(1)).unwrap();
            assert_eq!(cached_roles.len(), guild_2_user_ids.len());
            assert!(guild_2_user_ids.iter().all(|id| cached_roles.contains(id)));

            // Check for the cached members
            assert!(guild_2_user_ids
                .iter()
                .copied()
                .all(|id| cache.member(GuildId(1), id).is_some()));

            // Check for the cached users
            assert!(guild_2_user_ids.iter().all(|id| cache.user(*id).is_some()));
        }
    }

    #[test]
    fn test_cache_emoji() {
        let cache = InMemoryCache::new();

        // The user to do some of the inserts
        fn user_mod(id: EmojiId) -> Option<User> {
            if id.0 % 2 == 0 {
                // Only use user for half
                Some(user(UserId(1)))
            } else {
                None
            }
        }

        // Single inserts
        {
            let guild_1_emoji_ids = (1..=10).map(EmojiId).collect::<Vec<_>>();
            let guild_1_emoji = guild_1_emoji_ids
                .iter()
                .copied()
                .map(|id| emoji(id, user_mod(id)))
                .collect::<Vec<_>>();

            for emoji in guild_1_emoji {
                cache.cache_emoji(GuildId(1), emoji);
            }

            for id in guild_1_emoji_ids.iter().cloned() {
                let global_emoji = cache.emoji(id);
                assert!(global_emoji.is_some());
            }

            // Ensure the emoji has been added to the per-guild lookup map to prevent
            // issues like #551 from returning
            let guild_emojis = cache.guild_emojis(GuildId(1));
            assert!(guild_emojis.is_some());
            let guild_emojis = guild_emojis.unwrap();

            assert_eq!(guild_1_emoji_ids.len(), guild_emojis.len());
            assert!(guild_1_emoji_ids.iter().all(|id| guild_emojis.contains(id)));
        }

        // Bulk inserts
        {
            let guild_2_emoji_ids = (11..=20).map(EmojiId).collect::<Vec<_>>();
            let guild_2_emojis = guild_2_emoji_ids
                .iter()
                .copied()
                .map(|id| emoji(id, user_mod(id)))
                .collect::<Vec<_>>();
            cache.cache_emojis(GuildId(2), guild_2_emojis);

            for id in guild_2_emoji_ids.iter().cloned() {
                let global_emoji = cache.emoji(id);
                assert!(global_emoji.is_some());
            }

            let guild_emojis = cache.guild_emojis(GuildId(2));

            assert!(guild_emojis.is_some());
            let guild_emojis = guild_emojis.unwrap();
            assert_eq!(guild_2_emoji_ids.len(), guild_emojis.len());
            assert!(guild_2_emoji_ids.iter().all(|id| guild_emojis.contains(id)));
        }
    }
}
