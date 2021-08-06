use chrono::{FixedOffset, Local, Utc};
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(
    features = "scripting",
    derive(Trace, VmType, Userdata),
    gluon_userdata(clone),
    gluon(vm_type = "time.DateTime"),
    gluon_trace(skip)
)]
#[serde(transparent)]
pub struct DateTime(pub chrono::DateTime<FixedOffset>);

impl From<chrono::DateTime<Utc>> for DateTime {
    fn from(t: chrono::DateTime<Utc>) -> DateTime {
        DateTime(t.into())
    }
}

impl From<chrono::DateTime<Local>> for DateTime {
    fn from(t: chrono::DateTime<Local>) -> DateTime {
        DateTime(t.into())
    }
}

impl DateTime {
    pub fn now() -> Self {
        Self(Local::now().into())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(
    features = "scripting",
    derive(Trace, VmType, Userdata),
    gluon_userdata(clone),
    gluon(vm_type = "time.Duration"),
    gluon_trace(skip)
)]
pub struct Duration(pub chrono::Duration);

impl From<chrono::Duration> for Duration {
    fn from(d: chrono::Duration) -> Duration {
        Duration(d)
    }
}

// We only care about seconds, so we can just discard the nanos
impl Serialize for Duration {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(self.0.num_seconds())
    }
}

impl<'de> Deserialize<'de> for Duration {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct DurationVisitor;
        use std::fmt;
        impl<'de> de::Visitor<'de> for DurationVisitor {
            type Value = Duration;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("Duration")
            }

            fn visit_u64<E: de::Error>(self, value: u64) -> Result<Self::Value, E> {
                Ok(Duration(chrono::Duration::seconds(value as i64)))
            }
        }
        deserializer.deserialize_u64(DurationVisitor)
    }
}

#[derive(Clone, Debug)]
#[cfg_attr(
    features = "scripting",
    derive(Trace, VmType, Userdata),
    gluon_userdata(clone),
    gluon(vm_type = "time.TimeZone"),
    gluon_trace(skip)
)]
pub struct TimeZone(pub chrono::FixedOffset);
