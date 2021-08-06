//! Handles native notifications for tasks and events

use chrono::Utc;

use crate::storage::{api::*, time::DateTime};
use crate::STORE;

pub fn notify() {
    let sub_tasks = STORE.find_obj(|o: &Obj<SubTask>| o.inner.finished.is_none(), None);
    let now = Utc::now();
    let mut state = STORE.get_state().unwrap();
    let last_notified_time: Option<DateTime> = state.last_notified;
    let mut notified = false;
    for sub in sub_tasks.into_iter() {
        // TODO covert to Duration? need `PartialOrd`
        let mut notifications = sub.inner.notifications.clone();
        notifications.push(chrono::Duration::zero().into());
        notifications.sort_unstable();
        notifications.dedup();
        for diff_time in notifications.into_iter() {
            let notify_time = sub.inner.deadline.0 + diff_time.0;
            if last_notified_time.map(|l| l.0 < notify_time).unwrap_or(true) && notify_time <= now {
                notified = true;
                dbg!(diff_time, &sub);
                let body = if let Some(ref desc) = sub.desc {
                    format!("T{:+}\n\n{}", diff_time.0, desc)
                } else {
                    format!("T{:+}", diff_time.0)
                };
                notify_rust::Notification::new()
                    .appname("sched")
                    .summary(&format!("Sched: {}", &sub.name))
                    .body(&body)
                    .show()
                    .unwrap();
            }
        }
    }
    if notified {
        state.last_notified = Some(now.into());
        STORE.set_state(state).unwrap();
    }
}
