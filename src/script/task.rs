use crate::{
    script::{
        sched::{lock_store, Object},
        time::Duration,
    },
    storage::{OptRepeated, Result as StorageResult},
};

#[derive(Clone, Debug, VmType, Pushable, Getable)]
pub struct Task {
    pub object: Object,
    pub deadline: OptRepeated,
    pub priority: u32,
}

impl Task {
    pub fn new(name: &str, typ: &str, deadline: OptRepeated, priority: u32) -> StorageResult<i32> {
        lock_store()?.create_task(name, typ, deadline, priority)
    }

    pub fn get(id: i32) -> StorageResult<Task> {
        lock_store()?.get_task(id)
    }
}

#[derive(Clone, Debug, VmType, Pushable, Getable)]
pub struct Event {
    pub object: Object,
    pub start: OptRepeated,
    pub duration: Duration,
}

impl Event {
    pub fn new(name: &str, typ: &str, start: OptRepeated, duration: Duration) -> StorageResult<i32> {
        lock_store()?.create_event(name, typ, start, duration)
    }

    pub fn get(id: i32) -> StorageResult<Event> {
        lock_store()?.get_event(id)
    }
}

#[derive(Clone, Copy, Debug, VmType, Pushable, Getable)]
pub enum TaskStatus {
    Pending,
    Ready,
    Finished,
    Missed,
    Deleted,
}

impl TaskStatus {
    pub fn to_lower(self) -> String {
        use TaskStatus::*;
        match self {
            Pending => "pending".to_string(),
            Ready => "ready".to_string(),
            Finished => "finished".to_string(),
            Missed => "missed".to_string(),
            Deleted => "deleted".to_string(),
        }
    }

    pub fn from_string(s: &str) -> TaskStatus {
        use TaskStatus::*;
        match s {
            "pending" => Pending,
            "ready" => Ready,
            "finished" => Finished,
            "missed" => Missed,
            "deleted" => Deleted,
            _ => unreachable!(),
        }
    }
}
