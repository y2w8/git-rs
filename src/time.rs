use std::{fmt::Display, io::Write};

use bytes::{BytesMut, buf::Writer};
use gix::date::Time as GixTime;

use chrono::{DateTime, FixedOffset, TimeZone, Utc};

#[derive(Clone)]
pub struct Time {
    gix: GixTime,
}

pub enum Recency {
    Fresh,
    Recent,
    Stale,
}

impl Display for Recency {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Fresh => write!(f, "fresh"),
            Self::Recent => write!(f, "recent"),
            Self::Stale => write!(f, "stale"),
        }
    }
}

impl Time {
    pub fn new(gix: GixTime) -> Self {
        Self { gix }
    }

    pub fn format(&self) -> String {
        let dt: DateTime<Utc> = Utc.timestamp_opt(self.gix.seconds, 0).unwrap();
        let offset = FixedOffset::east_opt(self.gix.offset).unwrap();
        let local = dt.with_timezone(&offset);
        local.format("%Y-%m-%d %H:%M:%S %z").to_string()
    }

    pub fn format_rfc(&self, writer: &mut Writer<BytesMut>) {
        let dt: DateTime<Utc> = Utc.timestamp_opt(self.gix.seconds, 0).unwrap();
        let offset = FixedOffset::east_opt(self.gix.offset).unwrap();
        let local = dt.with_timezone(&offset);
        writer
            .write_all(
                local
                    .format("%a, %d %b %Y %H:%M:%S %z")
                    .to_string()
                    .as_bytes(),
            )
            .unwrap();
    }

    pub fn format_relative(&self) -> (String, Recency) {
        format_relative(self.gix.seconds)
    }
}

pub fn format_relative(seconds: i64) -> (String, Recency) {
    let then = Utc.timestamp_opt(seconds, 0).unwrap();
    let now = Utc::now();
    let diff = now.signed_duration_since(then);

    let secs = diff.num_seconds();
    let mins = diff.num_minutes();
    let hours = diff.num_hours();
    let days = diff.num_days();

    if secs < 120 {
        (pluralize(secs.max(0), "second"), Recency::Fresh)
    } else if mins < 120 {
        (pluralize(mins, "minute"), Recency::Fresh)
    } else if hours < 48 {
        (pluralize(hours, "hour"), Recency::Fresh)
    } else if days < 14 {
        (pluralize(days, "day"), Recency::Fresh)
    } else if days < 30 {
        (pluralize(days / 7, "week"), Recency::Recent)
    } else if days < 60 {
        (pluralize(days / 30, "month"), Recency::Recent)
    } else if days < 365 {
        (pluralize(days / 30, "month"), Recency::Stale)
    } else {
        (pluralize(days / 365, "year"), Recency::Stale)
    }
}

fn pluralize(count: i64, unit: &str) -> String {
    if count == 1 {
        format!("1 {unit}")
    } else {
        format!("{count} {unit}s")
    }
}
