// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
//! Process table — everything is read straight from procfs so the
//! 1-second refresh costs no subprocesses at all. CPU% is derived from
//! jiffies deltas between two ticks, normalised to one core like `top`.

use std::collections::HashMap;
use std::fs;

/// One row of the process list.
#[derive(Clone, Debug)]
pub struct Process {
    pub pid: i32,
    pub ppid: i32,
    pub name: String,
    pub cmdline: String,
    pub user: String,
    pub state: char,
    pub cpu: f32, // percent of one core
    pub rss_kb: u64,
    pub threads: i32,
    pub nice: i32,
}

/// Raw counters kept between ticks for the cpu% delta.
#[derive(Clone, Copy)]
struct Raw {
    total_jiffies: u64,
    ppid: i32,
    state: char,
    threads: i32,
    nice: i32,
    rss_kb: u64,
}

/// Parse `comm` away: field 2 is wrapped in parens and may itself
/// contain spaces or parens, so everything is taken from after the
/// last ')'. Field index `n` of the stat line maps to `n - 3` here.
fn read_stat(pid: i32) -> Option<Raw> {
    let s = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let rest = s.rsplit_once(") ")?.1;
    let f: Vec<&str> = rest.split_whitespace().collect();
    if f.len() < 22 {
        return None;
    }
    Some(Raw {
        state: f[0].chars().next().unwrap_or('?'),
        ppid: f[1].parse().unwrap_or(0),
        total_jiffies: f[11].parse::<u64>().unwrap_or(0) + f[12].parse::<u64>().unwrap_or(0),
        nice: f[16].parse().unwrap_or(0),
        threads: f[17].parse().unwrap_or(1),
        rss_kb: f[21].parse::<u64>().unwrap_or(0) * 4096 / 1024, // pages → kB
    })
}

/// Owner name for a pid, resolved via the uid in /proc/pid/status.
fn uid_of(pid: i32) -> Option<u32> {
    let s = fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    s.lines()
        .find_map(|l| l.strip_prefix("Uid:"))
        .and_then(|v| v.split_whitespace().next()?.parse().ok())
}

/// uid → login map, built once from /etc/passwd.
fn passwd_map() -> HashMap<u32, String> {
    let mut m = HashMap::new();
    if let Ok(s) = fs::read_to_string("/etc/passwd") {
        for l in s.lines() {
            let mut p = l.split(':');
            if let (Some(n), Some(Ok(u))) = (p.next(), p.nth(1).map(|v| v.parse::<u32>())) {
                m.insert(u, n.to_string());
            }
        }
    }
    m
}

/// Jiffies spent by a pid in /proc/uptime time — the wall-clock
/// lifetime is starttime-based, useful for the details dialog.
pub fn proc_start_secs(pid: i32, boot_secs: u64) -> f64 {
    fs::read_to_string(format!("/proc/{pid}/stat"))
        .ok()
        .and_then(|s| {
            let rest = s.rsplit_once(") ")?.1;
            let f: Vec<&str> = rest.split_whitespace().collect();
            let start_ticks: u64 = f.get(19)?.parse().ok()?;
            Some(boot_secs as f64 + start_ticks as f64 / 100.0)
        })
        .unwrap_or(0.0)
}

/// Full command line of a pid, NUL-joined, empty if kernel thread.
pub fn cmdline(pid: i32) -> String {
    fs::read(format!("/proc/{pid}/cmdline"))
        .ok()
        .map(|b| {
            b.split(|&c| c == 0)
                .filter(|s| !s.is_empty())
                .map(|s| String::from_utf8_lossy(s).into_owned())
                .collect::<Vec<_>>()
                .join(" ")
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_default()
}

/// Comm name of a pid (fallback display name).
fn comm(pid: i32) -> String {
    fs::read_to_string(format!("/proc/{pid}/comm"))
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "?".into())
}

/// Snapshot of every process plus the state needed for cpu% deltas.
pub struct ProcessTable {
    pub rows: Vec<Process>,
    prev: HashMap<i32, u64>,
    prev_total: u64,
    ncpu: f32,
    users: HashMap<u32, String>,
    // Per-pid caches: uid and cmdline change rarely — reading
    // /proc/pid/status and cmdline for ~500 pids every second was the
    // main cost of the refresh tick.
    uid_cache: HashMap<i32, u32>,
    cmd_cache: HashMap<i32, String>,
    name_cache: HashMap<i32, String>,
}

impl ProcessTable {
    /// Build an empty table; first `refresh` populates it with cpu%=0.
    pub fn new() -> Self {
        Self {
            rows: Vec::new(),
            prev: HashMap::new(),
            prev_total: 0,
            ncpu: num_cpus() as f32,
            users: passwd_map(),
            uid_cache: HashMap::new(),
            cmd_cache: HashMap::new(),
            name_cache: HashMap::new(),
        }
    }

    /// Re-read /proc and recompute the whole list. O(n) over pids.
    pub fn refresh(&mut self) {
        let total = total_jiffies();
        let d_total = total.saturating_sub(self.prev_total) as f64;
        let mut next: HashMap<i32, u64> = HashMap::new();
        let mut rows = Vec::new();
        if let Ok(rd) = fs::read_dir("/proc") {
            for e in rd.flatten() {
                let pid: i32 = match e.file_name().to_string_lossy().parse() {
                    Ok(p) => p,
                    Err(_) => continue,
                };
                let Some(r) = read_stat(pid) else { continue };
                let prev = self.prev.get(&pid).copied().unwrap_or(r.total_jiffies);
                let cpu = if d_total > 0.0 {
                    (r.total_jiffies - prev) as f32 / d_total as f32 * self.ncpu * 100.0
                } else {
                    0.0
                };
                next.insert(pid, r.total_jiffies);
                let uid = *self
                    .uid_cache
                    .entry(pid)
                    .or_insert_with(|| uid_of(pid).unwrap_or(0));
                let name = self.name_cache.entry(pid).or_insert_with(|| comm(pid)).clone();
                let cmd = self
                    .cmd_cache
                    .entry(pid)
                    .or_insert_with(|| cmdline(pid))
                    .clone();
                rows.push(Process {
                    pid,
                    ppid: r.ppid,
                    state: r.state,
                    cpu,
                    user: self.users.get(&uid).cloned().unwrap_or_else(|| uid.to_string()),
                    cmdline: cmd,
                    rss_kb: r.rss_kb,
                    threads: r.threads,
                    nice: r.nice,
                    name,
                });
            }
        }
        rows.sort_by(|a, b| {
            b.cpu
                .partial_cmp(&a.cpu)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        self.uid_cache.retain(|pid, _| next.contains_key(pid));
        self.cmd_cache.retain(|pid, _| next.contains_key(pid));
        self.name_cache.retain(|pid, _| next.contains_key(pid));
        self.rows = rows;
        self.prev = next;
        self.prev_total = total;
    }
}

/// Number of online cores from /proc/stat cpuN lines.
pub fn num_cpus() -> usize {
    fs::read_to_string("/proc/stat")
        .map(|s| {
            s.lines()
                .filter(|l| l.chars().nth(3).is_some_and(|c| c.is_ascii_digit()))
                .count()
                .max(1)
        })
        .unwrap_or(1)
}

/// Total jiffies across all cpus — denominator for process cpu%.
fn total_jiffies() -> u64 {
    fs::read_to_string("/proc/stat")
        .ok()
        .and_then(|s| {
            s.lines()
                .next()?
                .split_whitespace()
                .skip(1)
                .map(|v| v.parse::<u64>().unwrap_or(0))
                .sum::<u64>()
                .into()
        })
        .unwrap_or(0)
}

/// Seconds since boot — btime from /proc/stat → unix epoch at boot.
pub fn boot_seconds() -> u64 {
    fs::read_to_string("/proc/stat")
        .ok()
        .and_then(|s| {
            s.lines()
                .find_map(|l| l.strip_prefix("btime "))
                .and_then(|v| v.trim().parse().ok())
        })
        .unwrap_or(0)
}
