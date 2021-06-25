use prometheus::{opts, IntGauge, IntGaugeVec};

#[derive(Debug)]
pub struct Metrics {
    pub metrics: IntGaugeVec,

    pub channels_guild: IntGauge,
    pub channels_private: IntGauge,
    pub guilds: IntGauge,
    pub members: IntGauge,
    pub messages: IntGauge,
    pub roles: IntGauge,
    pub unavailable_guilds: IntGauge,
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
