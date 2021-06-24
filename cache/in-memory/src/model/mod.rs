//! Models built for utilizing efficient caching.

mod cold_role;
mod cold_textchannel;
mod cold_user;
mod emoji;
mod guild;
mod member;
mod message;
mod presence;
mod voice_state;

pub(crate) use self::{
    cold_role::ColdStorageRole, cold_textchannel::ColdStorageTextChannel,
    cold_user::ColdStorageUser,
};

pub use self::{
    emoji::CachedEmoji, guild::CachedGuild, member::CachedMember, message::CachedMessage,
    presence::CachedPresence, voice_state::CachedVoiceState,
};

#[inline]
fn is_false(b: &bool) -> bool {
    !b
}

#[cfg(tests)]
mod tests {
    #[test]
    fn test_reexports() {
        use super::{CachedEmoji, CachedGuild, CachedMember, CachedPresence, CachedVoiceState};
    }
}
