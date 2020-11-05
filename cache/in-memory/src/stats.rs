use super::InMemoryCache;

use std::collections::HashSet;

pub struct CacheStats {
    pub channels_guild: usize,
    pub channels_private: usize,
    pub emojis: usize,
    pub guilds: usize,
    pub members: usize,
    pub messages: usize,
    pub roles: usize,
    pub unavailable_guilds: usize,
    pub users: usize,
    pub biggest_guilds: Vec<CompactGuild>,
    pub most_mutuals_users: Vec<CompactUser>,
}

impl InMemoryCache {
    pub fn stats(&self, guild_amount: usize, mutuals_amount: usize) -> CacheStats {
        let messages = self
            .0
            .messages
            .iter()
            .map(|guard| guard.value().len())
            .sum();
        let mut guilds: Vec<_> = self
            .0
            .guild_members
            .iter()
            .map(|guard| (*guard.key(), guard.value().len()))
            .collect();
        guilds.sort_unstable_by(|(_, a), (_, b)| b.cmp(a));
        guilds.truncate(guild_amount);
        let biggest_guilds = guilds
            .into_iter()
            .filter_map(|(guild_id, member_count)| {
                let name = self.0.guilds.get(&guild_id)?.name.to_owned();
                Some(CompactGuild { name, member_count })
            })
            .collect();
        let mut users = HashSet::with_capacity(mutuals_amount);
        let mut lowest = (usize::MAX, None);
        let bot_user = self.current_user().map_or(0, |user| user.id.0);
        for guard in self.0.users.iter() {
            if guard.key().0 == bot_user {
                continue;
            }
            let len = guard.value().1.len();
            let user = *guard.key();
            if users.len() < mutuals_amount {
                if len < lowest.0 {
                    lowest = (len, Some(user));
                }
                users.insert((user, len));
            } else if len > lowest.0 {
                users.remove(&(lowest.1.unwrap(), lowest.0));
                users.insert((user, len));
                lowest = users
                    .iter()
                    .fold(None, |lowest, next| {
                        if next.1 < lowest.map_or(usize::MAX, |val: (usize, _)| val.0) {
                            Some((next.1, Some(next.0)))
                        } else {
                            lowest
                        }
                    })
                    .unwrap();
            }
        }
        let mut most_mutuals_users: Vec<_> = users
            .into_iter()
            .filter_map(|(user, mutual_count)| {
                let user = self.user(user)?;
                let name = format!("{}#{}", user.name, user.discriminator);
                Some(CompactUser { name, mutual_count })
            })
            .collect();
        most_mutuals_users.sort_unstable_by(|a, b| b.mutual_count.cmp(&a.mutual_count));
        CacheStats {
            channels_guild: self.0.channels_guild.len(),
            channels_private: self.0.channels_private.len(),
            emojis: self.0.emojis.len(),
            guilds: self.0.guilds.len(),
            members: self.0.members.len(),
            messages,
            roles: self.0.roles.len(),
            unavailable_guilds: self.0.unavailable_guilds.len(),
            users: self.0.users.len(),
            biggest_guilds,
            most_mutuals_users,
        }
    }
}

pub struct CompactGuild {
    pub name: String,
    pub member_count: usize,
}

pub struct CompactUser {
    pub name: String,
    pub mutual_count: usize,
}
