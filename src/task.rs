use bson::{doc, from_bson, Bson};

use crate::script::{
    task::{Event, Every, Stop, Task},
    time::{DateTime, Duration, Time},
};
use crate::storage::{Error, Result, Storage};

impl Storage {
    pub fn create_task(
        &mut self,
        name: &str,
        start: DateTime,
        every: Every,
        stop: Stop,
        deadline: Time,
    ) -> Result<i32> {
        let id = self.get_obj_id();
        self.objs
            .insert_one(doc! { "_id": id, "name": name, "type": "task", "start": start.to_utc(), "every": every.to_doc(), "stop": stop.to_doc(), "deadline": deadline.to_secs() }, None)
            .unwrap();
        self.create_log("task.create", doc! { "id": id })?;
        Ok(id)
    }

    pub fn get_task(&mut self, id: i32) -> Result<Task> {
        let task = self
            .objs
            .find_one(doc! { "_id": id }, None)
            .unwrap()
            .ok_or_else(|| Error::InvalidObjID(id))?;
        if task.get_str("type") != Ok("task") {
            return Err(Error::ObjNotTask(id));
        }
        let start = task.get_datetime("start").unwrap().clone().into();
        let every = Every::from_doc(task.get_document("every").unwrap().clone());
        let stop = Stop::from_doc(task.get_document("stop").unwrap().clone());
        let deadline = Time::from_secs(task.get_i32("deadline").unwrap() as u32);
        let object = from_bson(Bson::Document(task)).unwrap();
        Ok(Task {
            start,
            every,
            stop,
            deadline,
            object,
        })
    }

    pub fn create_event(
        &mut self,
        name: &str,
        start: DateTime,
        every: Every,
        stop: Stop,
        event_start: Time,
        duration: Duration,
    ) -> Result<i32> {
        let id = self.get_obj_id();
        self.objs
            .insert_one(doc! { "_id": id, "name": name, "type": "event", "start": start.to_utc(), "every": every.to_doc(), "stop": stop.to_doc(), "event_start": event_start.to_secs(), "duration": duration.to_parts().0 }, None)
            .unwrap();
        self.create_log("event.create", doc! { "id": id })?;
        Ok(id)
    }

    pub fn get_event(&mut self, id: i32) -> Result<Event> {
        let task = self
            .objs
            .find_one(doc! { "_id": id }, None)
            .unwrap()
            .ok_or_else(|| Error::InvalidObjID(id))?;
        if task.get_str("type") != Ok("event") {
            return Err(Error::ObjNotEvent(id));
        }
        let start = task.get_datetime("start").unwrap().clone().into();
        let every = Every::from_doc(task.get_document("every").unwrap().clone());
        let stop = Stop::from_doc(task.get_document("stop").unwrap().clone());
        let event_start = Time::from_secs(task.get_i32("event_start").unwrap() as u32);
        let duration = Duration::seconds(task.get_i64("duration").unwrap());
        let object = from_bson(Bson::Document(task)).unwrap();
        Ok(Event {
            start,
            every,
            stop,
            event_start,
            duration,
            object,
        })
    }
}
