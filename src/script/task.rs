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
    pub task_typ: String,
    /// A fixed-size FIFO cache of the daughter task ids with user configurable size
    pub cache: Vec<u32>,
}

// FIXME use `IO<T>` for returns
impl Task {
    pub fn new(name: &str, typ: &str, deadline: OptRepeated, priority: u32) -> StorageResult<u32> {
        lock_store()?.create_task(name, typ, deadline, priority, None)
    }

    pub fn get(id: u32) -> StorageResult<Task> {
        lock_store()?.get_task(id)
    }

    pub fn finish(id: u32) -> StorageResult<()> {
        lock_store()?.task_finish(id, chrono::Local::now().into())
    }

    pub fn find_current(id: u32) -> StorageResult<Option<u32>> {
        lock_store()?.find_current(id)
    }
}

#[derive(Clone, Debug, VmType, Pushable, Getable)]
pub struct Event {
    pub object: Object,
    pub start: OptRepeated,
    pub duration: Duration,
}

impl Event {
    pub fn new(name: &str, typ: &str, start: OptRepeated, duration: Duration) -> StorageResult<u32> {
        lock_store()?.create_event(name, typ, start, duration, None)
    }

    pub fn get(id: u32) -> StorageResult<Event> {
        lock_store()?.get_event(id)
    }
}
