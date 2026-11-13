//! GPU use of each tree from the DRM fdinfo of its members. The probe reads the fdinfo of each
//! file descriptor that points into `/dev/dri`, and counts each DRM client once per tree, keyed
//! by driver, device and client id. Memory is the sum over clients; buffers that clients share
//! count once per client.
//!
//! The busy time of a tree is cumulative: it starts at the engine time of its clients at the first
//! sample and grows by the engine time of each client since the previous sample, so it never
//! falls when a client closes. A client that opens inside a span counts in full. A driver that
//! reports busy cycles and elapsed cycles instead of nanoseconds adds the busy share of each engine
//! times the wall time since the previous sample; its first sample adds nothing.

use std::collections::{BTreeSet, HashMap};
use std::fs;

use fakeland_types::Timestamp;

use super::process::{GpuProcessProbe, GpuProcessReading, TreeMembers};
use super::usage::{DrmClient, parse_fdinfo};

/// The identity of a DRM client.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct ClientKey {
    driver: String,
    pdev: Option<String>,
    /// The client id, or the pid and descriptor of a client without an id.
    id: (u64, u32),
}

#[derive(Clone, Debug)]
struct ClientBaseline {
    t: Timestamp,
    busy_ns: Option<u64>,
    cycles: Vec<(String, u64, u64)>,
}

#[derive(Debug, Default)]
struct TreeState {
    busy_ns_total: Option<u64>,
    clients: HashMap<ClientKey, ClientBaseline>,
}

/// Reads the DRM fdinfo of tree members.
#[derive(Debug, Default)]
pub struct FdinfoProbe {
    trees: HashMap<u32, TreeState>,
}

impl GpuProcessProbe for FdinfoProbe {
    fn name(&self) -> &'static str {
        "fdinfo"
    }

    fn read(&mut self, t: Timestamp, trees: &[TreeMembers<'_>], _: bool) -> Vec<GpuProcessReading> {
        let mut out = Vec::new();
        for tree in trees {
            let clients = read_clients(tree.pids);
            if clients.is_empty() {
                continue;
            }
            let state = self.trees.entry(tree.root).or_default();
            out.push(state.update(tree.root, t, &clients));
        }
        out
    }

    fn forget(&mut self, root: u32) {
        self.trees.remove(&root);
    }
}

impl TreeState {
    fn update(
        &mut self,
        root: u32,
        t: Timestamp,
        clients: &HashMap<ClientKey, DrmClient>,
    ) -> GpuProcessReading {
        let mut next = HashMap::new();
        let (mut mem, mut busy, mut has_busy) = (None::<u64>, 0.0f64, false);
        let mut drivers = BTreeSet::new();
        for (key, c) in clients {
            drivers.insert(c.driver.as_str());
            if let Some(m) = c.memory() {
                mem = Some(mem.unwrap_or(0) + m);
            }
            let base = ClientBaseline {
                t,
                busy_ns: c.busy_ns(),
                cycles: c
                    .cycles
                    .iter()
                    .filter_map(|(e, &n)| Some((e.clone(), n, *c.total_cycles.get(e)?)))
                    .collect(),
            };
            has_busy |= base.busy_ns.is_some() || !base.cycles.is_empty();
            busy += match self.clients.get(key) {
                Some(prev) => busy_since(prev, &base),
                None => base.busy_ns.unwrap_or(0) as f64,
            };
            next.insert(key.clone(), base);
        }
        self.clients = next;
        if has_busy {
            let total = self.busy_ns_total.unwrap_or(0) + busy.round() as u64;
            self.busy_ns_total = Some(total);
        }
        GpuProcessReading {
            root,
            mem_bytes: mem,
            busy_ns_total: self.busy_ns_total,
            util_percent: None,
            source: format!(
                "fdinfo:{}",
                drivers.into_iter().collect::<Vec<_>>().join("+")
            ),
        }
    }
}

/// Returns the busy nanoseconds of a client between two samples.
fn busy_since(prev: &ClientBaseline, next: &ClientBaseline) -> f64 {
    if let Some(ns) = next.busy_ns {
        return ns.saturating_sub(prev.busy_ns.unwrap_or(0)) as f64;
    }
    let wall = next.t.since(prev.t).as_nanos().max(0) as f64;
    next.cycles
        .iter()
        .filter_map(|(engine, busy, total)| {
            let (_, b0, t0) = prev.cycles.iter().find(|c| &c.0 == engine)?;
            let dt = total.checked_sub(*t0).filter(|d| *d > 0)?;
            let share = (busy.saturating_sub(*b0) as f64 / dt as f64).min(1.0);
            Some(share * wall)
        })
        .sum()
}

/// Reads the DRM clients of a set of processes, each client once.
fn read_clients(pids: &[u32]) -> HashMap<ClientKey, DrmClient> {
    let mut out = HashMap::new();
    for &pid in pids {
        let Ok(dir) = fs::read_dir(format!("/proc/{pid}/fd")) else {
            continue;
        };
        for e in dir.flatten() {
            let is_dri = fs::read_link(e.path()).is_ok_and(|p| p.starts_with("/dev/dri/"));
            let Some(fd) = e.file_name().to_str().and_then(|s| s.parse::<u32>().ok()) else {
                continue;
            };
            if !is_dri {
                continue;
            }
            let Some(c) = fs::read_to_string(format!("/proc/{pid}/fdinfo/{fd}"))
                .ok()
                .and_then(|t| parse_fdinfo(&t))
            else {
                continue;
            };
            let key = ClientKey {
                driver: c.driver.clone(),
                pdev: c.pdev.clone(),
                id: c.client_id.map_or((u64::from(pid), fd), |id| (id, 0)),
            };
            out.insert(key, c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use fakeland_types::Nanos;

    use super::*;

    fn client(id: u64, mem: u64, busy: Option<u64>, cycles: Option<(u64, u64)>) -> DrmClient {
        DrmClient {
            driver: "xe".into(),
            client_id: Some(id),
            resident: Some(mem),
            engines: busy
                .map(|b| BTreeMap::from([("render".into(), b)]))
                .unwrap_or_default(),
            cycles: cycles
                .map(|c| BTreeMap::from([("rcs".into(), c.0)]))
                .unwrap_or_default(),
            total_cycles: cycles
                .map(|c| BTreeMap::from([("rcs".into(), c.1)]))
                .unwrap_or_default(),
            ..DrmClient::default()
        }
    }

    fn map(clients: Vec<DrmClient>) -> HashMap<ClientKey, DrmClient> {
        clients
            .into_iter()
            .map(|c| {
                let key = ClientKey {
                    driver: c.driver.clone(),
                    pdev: None,
                    id: (c.client_id.unwrap(), 0),
                };
                (key, c)
            })
            .collect()
    }

    #[test]
    fn busy_time_accumulates_over_closed_clients() {
        let mut s = TreeState::default();
        let t0 = Timestamp(1_000_000_000);
        let r = s.update(1, t0, &map(vec![client(1, 10, Some(100), None)]));
        assert_eq!((r.mem_bytes, r.busy_ns_total), (Some(10), Some(100)));
        let t1 = t0 + Nanos::from_millis(100);
        let r = s.update(
            1,
            t1,
            &map(vec![
                client(1, 10, Some(150), None),
                client(2, 5, Some(30), None),
            ]),
        );
        assert_eq!((r.mem_bytes, r.busy_ns_total), (Some(15), Some(180)));
        // Client 1 closes; client 2 runs 20 ns more.
        let r = s.update(
            1,
            t1 + Nanos::from_millis(100),
            &map(vec![client(2, 5, Some(50), None)]),
        );
        assert_eq!((r.mem_bytes, r.busy_ns_total), (Some(5), Some(200)));
        assert_eq!(r.source, "fdinfo:xe");
    }

    #[test]
    fn cycles_turn_into_busy_time_over_the_wall_span() {
        let mut s = TreeState::default();
        let t0 = Timestamp(1_000_000_000);
        let r = s.update(1, t0, &map(vec![client(1, 0, None, Some((1000, 10_000)))]));
        assert_eq!(r.busy_ns_total, Some(0));
        // A quarter of the elapsed cycles are busy over 100 ms: 25 ms.
        let r = s.update(
            1,
            t0 + Nanos::from_millis(100),
            &map(vec![client(1, 0, None, Some((3000, 18_000)))]),
        );
        assert_eq!(r.busy_ns_total, Some(25_000_000));
    }

    #[test]
    fn reads_no_clients_of_a_missing_process() {
        assert!(read_clients(&[u32::MAX]).is_empty());
        let mut p = FdinfoProbe::default();
        let pids = [u32::MAX];
        let trees = [TreeMembers {
            root: u32::MAX,
            pids: &pids,
        }];
        assert!(p.read(Timestamp(0), &trees, true).is_empty());
    }
}
