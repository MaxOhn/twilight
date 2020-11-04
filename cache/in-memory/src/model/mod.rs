//! Models built for utilizing efficient caching.

mod cold_current_user;
mod cold_role;
mod cold_textchannel;
mod cold_user;
mod emoji;
mod guild;
mod member;
mod message;

pub use self::{
    emoji::CachedEmoji, guild::CachedGuild, member::CachedMember, message::CachedMessage,
};

pub(crate) use self::{
    cold_current_user::ColdStorageCurrentUser, cold_role::ColdStorageRole,
    cold_textchannel::ColdStorageTextChannel, cold_user::ColdStorageUser, emoji::ColdStorageEmoji,
    member::ColdStorageMember,
};

#[cfg(tests)]
mod tests {
    #[test]
    fn test_reexports() {
        use super::{CachedEmoji, CachedGuild, CachedMember, CachedPresence, CachedVoiceState};
    }
}
