# Sched

A personal scheduling system for keeping track of tasks and events, habit tracking, and eventually (semi-) automatic task ordering and scheduling. The current system is based on an objects backed by an event log. The objects are meant to store relatively persistent data and state, while the log stores instant events, like when a task has been finished in addition to changes made to the objects. The log is used to help the user remember what happened in the past, but can also be used for future state recovery functionalities. It will also serve as the main source for all kinds of statistics.

This system is designed to have 2 halves. The bottom one is written in Rust which provides the storage for the objects and logs, and will provide tasks/events scheduling functionalities in the future. However the bottom half doesn't do anything without the top half, where the user uses a embedded scripting language [gluon](https://github.com/gluon-lang/gluon) to manipulate the states of the system and handles the events.

## Todos

[X] Backend object/log store.
[ ] Repeatable tasks.

The current design is to store the mother tasks (the ones that contain the time to repeat, descriptions etc.) as objects, and the daughter tasks (the actual individual tasks with their own completion status) as events in the logs when their statuses change. The main reason for this design is, the daughter tasks can be short living, and keeping them in the object pool will pollute it. Because it can be useful to search for the last several daughter tasks for a certain mother task, it's possible to store the log ids of the latest updates to some daughter tasks in the mother task, and have the number be configurable by the user individually for each task.

[X] User-facing REPL/shell

Currently a simple command-line interface has been implemented based on the APIs of [`clap`](https://github.com/clap-rs/clap), which allows the user to create custom commands. This works fine, though it would be better to use the Gluon REPL, but that'll have to wait for some issues in the Gluon REPL to be resolved.

[ ] User-facing TUI
[ ] Basic daily/weekly/monthly statistics
[ ] Simple priority system
[ ] Dependencies for tasks/events, projects containing multiple tasks
[ ] Basic task scheduling
