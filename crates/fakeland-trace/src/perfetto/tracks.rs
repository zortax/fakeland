//! Process and thread tracks, with lanes that keep overlapping slices apart.

use std::collections::HashMap;

use serde_json::Value;

use crate::perfetto::chrome::ChromeEvent;

/// The first synthetic thread id. Synthetic ids stay clear of real process ids.
const FIRST_TID: u64 = 1 << 30;

/// Named processes and threads, and the metadata events that name them.
#[derive(Default)]
pub(crate) struct Tracks {
    pub(crate) metadata: Vec<ChromeEvent>,
    processes: HashMap<u64, String>,
    threads: HashMap<(u64, String), u64>,
    lanes: HashMap<(u64, String), Vec<(u64, f64)>>,
    next_tid: u64,
}

impl Tracks {
    /// Names a process once. Later calls keep the first name.
    pub(crate) fn process(&mut self, pid: u64, name: &str, sort: i64) {
        if self.processes.contains_key(&pid) {
            return;
        }
        self.processes.insert(pid, name.to_string());
        self.metadata.push(ChromeEvent::metadata(
            "process_name",
            pid,
            0,
            "name",
            Value::from(name),
        ));
        self.metadata.push(ChromeEvent::metadata(
            "process_sort_index",
            pid,
            0,
            "sort_index",
            Value::from(sort),
        ));
    }

    /// Returns the thread with a name in a process, and names it on first use.
    pub(crate) fn thread(&mut self, pid: u64, name: &str) -> u64 {
        if let Some(tid) = self.threads.get(&(pid, name.to_string())) {
            return *tid;
        }
        let tid = FIRST_TID + self.next_tid;
        self.next_tid += 1;
        self.threads.insert((pid, name.to_string()), tid);
        self.metadata.push(ChromeEvent::metadata(
            "thread_name",
            pid,
            tid,
            "name",
            Value::from(name),
        ));
        self.metadata.push(ChromeEvent::metadata(
            "thread_sort_index",
            pid,
            tid,
            "sort_index",
            Value::from(tid - FIRST_TID),
        ));
        tid
    }

    /// Returns a thread of the lane group `name` that is free from `start`, and marks it busy
    /// until `end`. The first lane has the group name; later lanes add a number.
    pub(crate) fn lane(&mut self, pid: u64, name: &str, start: f64, end: f64) -> u64 {
        let key = (pid, name.to_string());
        let lanes = self.lanes.remove(&key).unwrap_or_default();
        let mut lanes = lanes;
        let tid = match lanes.iter_mut().find(|(_, busy)| *busy <= start) {
            Some(lane) => {
                lane.1 = end.max(start);
                lane.0
            }
            None => {
                let label = if lanes.is_empty() {
                    name.to_string()
                } else {
                    format!("{name} {}", lanes.len() + 1)
                };
                let tid = self.thread(pid, &label);
                lanes.push((tid, end.max(start)));
                tid
            }
        };
        self.lanes.insert(key, lanes);
        tid
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lanes_split_overlaps() {
        let mut t = Tracks::default();
        let a = t.lane(1, "gpu", 0.0, 10.0);
        let b = t.lane(1, "gpu", 5.0, 12.0);
        let c = t.lane(1, "gpu", 10.0, 11.0);
        let d = t.lane(1, "gpu", 12.0, 13.0);
        assert_ne!(a, b);
        assert_eq!(a, c);
        assert_eq!(d, a);
        assert_eq!(t.thread(1, "gpu"), a);
        assert_ne!(t.thread(2, "gpu"), a);
    }
}
