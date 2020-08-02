use std::collections::HashMap;

use chrono::{DateTime, Utc};

#[derive(Debug, Serialize, Deserialize)]
pub struct Log {
    name: String,
    #[serde(rename(serialize = "type", deserialize = "type"))]
    typ: String,
    time: DateTime<Utc>,
    desc: Option<String>,
    obj: Option<ObjectRef>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Object {
    name: String,
    typ: String,
    desc: String,
    deps: Vec<ObjectRef>,
    subs: Vec<ObjectRef>,
    refs: Vec<ObjectRef>,
    attrs: HashMap<String, String>,
}

pub type ObjectRef = i32;

#[cfg(feature = "use-redis")]
pub mod storage_redis;
#[cfg(feature = "use-redis")]
pub use storage_redis::*;

#[cfg(feature = "mongodb")]
pub mod storage_mongodb;
#[cfg(feature = "mongodb")]
pub use storage_mongodb::*;
