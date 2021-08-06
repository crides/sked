use std::fmt;
use std::num::ParseIntError;
use std::str::FromStr;

use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

use crate::{
    api_logs, api_objs,
    storage::{
        time::{DateTime, Duration},
        OptRepeated,
    },
    storage_objs,
};

pub trait ApiLog: Serialize {
    const LOG_TYPE: &'static str;
}

pub trait ApiObj: Serialize {
    const OBJ_TYPE: &'static str;
}

pub type IdType = u32;
pub type ApiVec<T> = Vec<T>; //smallvec::SmallVec<[T; 16]>;
pub type ApiMap<K, V> = std::collections::BTreeMap<K, V>;
pub use serde_json::Value as AttrValue;
pub type Attrs = ApiMap<String, AttrValue>;

#[cfg_attr(features = "scripting", derive(Trace, VmType, Userdata))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LogId(pub IdType);

impl fmt::Display for LogId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "L{}", self.0)
    }
}

#[cfg_attr(features = "scripting", derive(Trace, VmType, Userdata))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ObjId(pub IdType);

impl fmt::Display for ObjId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "O{}", self.0)
    }
}

#[derive(Clone, Copy, Debug)]
pub enum EitherId {
    Obj(ObjId),
    Log(LogId),
}

impl fmt::Display for EitherId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            EitherId::Obj(id) => write!(f, "{}", id),
            EitherId::Log(id) => write!(f, "{}", id),
        }
    }
}

impl FromStr for EitherId {
    type Err = EitherIdParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let id = s[1..].parse::<IdType>().map_err(EitherIdParseError::ParseInt)?;
        match s.chars().next() {
            Some('O') => Ok(EitherId::Obj(ObjId(id))),
            Some('L') => Ok(EitherId::Log(LogId(id))),
            c @ _ => Err(EitherIdParseError::InvalidHeader(c)),
        }
    }
}

impl Serialize for EitherId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for EitherId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct EitherIdVisitor;
        impl<'de> de::Visitor<'de> for EitherIdVisitor {
            type Value = EitherId;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("EitherId")
            }

            fn visit_str<E: de::Error>(self, s: &str) -> Result<Self::Value, E> {
                s.parse().map_err(E::custom)
            }
        }
        deserializer.deserialize_str(EitherIdVisitor)
    }
}

#[derive(Clone, Debug, Error)]
pub enum EitherIdParseError {
    #[error("Unknown header `{0:?}`")]
    InvalidHeader(Option<char>),
    #[error("Can't parse int")]
    ParseInt(ParseIntError),
    #[error("{0}")]
    Message(String),
}

impl de::Error for EitherIdParseError {
    fn custom<T: std::fmt::Display>(msg: T) -> Self {
        EitherIdParseError::Message(msg.to_string())
    }
}

storage_objs! {
    obj {
        name: String,
        #[serde(default)]
        #[serde(skip_serializing_if = "Option::is_none")]
        desc: Option<String>,
        #[serde(default)]
        #[serde(skip_serializing_if = "Option::is_none")]
        attrs: Option<Attrs>
    }

    log {
        /// UTC time for when the log happened
        time: DateTime,
        #[serde(default)]
        #[serde(skip_serializing_if = "Option::is_none")]
        attrs: Option<Attrs>,
    }
}

// TODO Better name?
/// Things that can be deser'ed from nothing (`Default`) and can be ser'ed to nothing sometimes
pub trait MinimizedSerde: Default {
    fn min_able(&self) -> bool;
}

// FIXME auto gen
fn gen_ahead_default() -> usize {
    5
}
fn cache_size_default() -> usize {
    10
}

fn is_false(b: &bool) -> bool {
    !*b
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(features = "scripting", derive(Trace, VmType, Userdata))]
#[serde(rename_all = "kebab-case")]
pub enum TaskFlavor {
    /// Switch to new task right after the current deadline has passed
    Deadline,
    /// Switch to new task in the middle of 2 deadlines
    Balanced,
}

impl Default for TaskFlavor {
    fn default() -> Self {
        TaskFlavor::Deadline
    }
}

impl MinimizedSerde for TaskFlavor {
    fn min_able(&self) -> bool {
        self == &TaskFlavor::Deadline
    }
}

impl<T> MinimizedSerde for ApiVec<T> {
    fn min_able(&self) -> bool {
        self.is_empty()
    }
}

impl<K: Ord, V> MinimizedSerde for ApiMap<K, V> {
    fn min_able(&self) -> bool {
        self.is_empty()
    }
}

api_objs! {
    State "sys.state" {
        #[serde(default)] #[new(default)]
        #[serde(skip_serializing_if = "Option::is_none")]
        last_notified: Option<DateTime>,
    }

    Event "event" {
        start: OptRepeated,
        duration: Duration,
    }

    Task "task" {
        deadline: OptRepeated,
        priority: u32,
        #[serde(default)] #[new(default)]
        flavor: TaskFlavor,
        #[serde(default = "gen_ahead_default")] #[new(value = "5")]
        gen_ahead: usize,
        #[serde(default = "cache_size_default")] #[new(value = "10")]
        cache_size: usize,
        #[serde(default)] #[new(default)]
        #[serde(skip_serializing_if = "MinimizedSerde::min_able")]
        notifications: ApiVec<Duration>,
        /// A fixed-size FIFO cache of the daughter task ids with user configurable size
        cache: ApiVec<ObjId>,
    }

    SubTask "task.sub" {
        task_id: ObjId,
        deadline: DateTime,
        #[serde(default)]
        #[serde(skip_serializing_if = "MinimizedSerde::min_able")]
        notifications: ApiVec<Duration>,
        #[serde(default)] #[new(default)]
        #[serde(skip_serializing_if = "Option::is_none")]
        finished: Option<DateTime>,
    }
}

impl State {
    pub const ID: ObjId = ObjId(0);
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Diff<T> {
    New(T),
    Del(T),
    Diff(T, T),
}

api_logs! {
    CreateObj "obj.create" {
        id: ObjId,
        typ: String,
    }

    ObjSetDesc "obj.set_desc" {
        id: ObjId,
        diff: Diff<String>,
    }

    ObjSetAttr "obj.set_attr" {
        id: ObjId,
        attr: String,
        diff: Diff<AttrValue>,
    }

    TaskFinish "task.finish" {
        id: ObjId,
    }
}
