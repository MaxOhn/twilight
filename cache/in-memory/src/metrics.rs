use prometheus::{opts, IntGauge, IntGaugeVec};

#[derive(Debug)]
/// Struct containing various prometheus fields to provide
/// metrics for the current state of the cache
pub struct Metrics {
    /// IntGaugeVec containing all other fields of this struct
    pub metrics: IntGaugeVec,

    /// Gauge for cached guild channels
    pub channels_guild: IntGauge,
    /// Gauge for cached private channels
    pub channels_private: IntGauge,
    /// Gauge for cached guilds
    pub guilds: IntGauge,
    /// Gauge for cached members
    pub members: IntGauge,
    /// Gauge for cached messages
    pub messages: IntGauge,
    /// Gauge for cached roles
    pub roles: IntGauge,
    /// Gauge for cached unavailable guilds
    pub unavailable_guilds: IntGauge,
    /// Gauge for cached users
    pub users: IntGauge,
}

impl Default for Metrics {
    fn default() -> Self {
        let metrics =
            IntGaugeVec::new(opts!("cached_metrics", "Cached metrics"), &["Metrics"]).unwrap();

        Self {
            channels_guild: metrics.with_label_values(&["Guild channels"]),
            channels_private: metrics.with_label_values(&["Private channels"]),
            guilds: metrics.with_label_values(&["Guilds"]),
            members: metrics.with_label_values(&["Members"]),
            messages: metrics.with_label_values(&["Messages"]),
            roles: metrics.with_label_values(&["Roles"]),
            unavailable_guilds: metrics.with_label_values(&["Unavailable guilds"]),
            users: metrics.with_label_values(&["Users"]),

            metrics,
        }
    }
}
