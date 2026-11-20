// SPDX-License-Identifier: GPL-3.0-or-later
//! System-level statistics — cpu load per core, memory, network,
//! disk io and gpu load — all read from procfs/sysfs. GPU utilisation
//! uses the amdgpu sysfs busy counter; NVIDIA falls back to a single
//! nvidia-smi query per tick (the driver exposes nothing under /sys).

use std::fs;

/// Per-core busy fraction 0..1 derived from /proc/stat deltas.
#[derive(Default)]
pub struct CpuSampler {
    prev: Vec<(u64, u64)>, // (busy, total) per core
}

impl CpuSampler {
    /// Read /proc/stat, return busy% per core since the last call.
    pub fn sample(&mut self) -> Vec<f32> {
        let Ok(s) = fs::read_to_string("/proc/stat") else {
            return Vec::new();
        };
        let mut now = Vec::new();
        for l in s.lines().skip(1) {
            if !l.starts_with("cpu") || !l.chars().nth(3).is_some_and(|c| c.is_ascii_digit()) {
                break;
            }
            let v: Vec<u64> = l
                .split_whitespace()
                .skip(1)
                .map(|x| x.parse().unwrap_or(0))
                .collect();
            let idle = v.get(3).copied().unwrap_or(0) + v.get(4).copied().unwrap_or(0);
            let total: u64 = v.iter().sum();
            now.push((total - idle, total));
        }
        let out = now
            .iter()
            .zip(&self.prev)
            .map(|(n, p)| {
                let dt = n.1.saturating_sub(p.1);
                if dt == 0 {
                    0.0
                } else {
                    n.0.saturating_sub(p.0) as f32 / dt as f32
                }
            })
            .collect();
        self.prev = now;
        out
    }
}

/// Memory counters in kB straight from /proc/meminfo.
#[derive(Default, Clone)]
pub struct Mem {
    pub total: u64,
    pub avail: u64,
    pub swap_total: u64,
    pub swap_free: u64,
    pub buffers: u64,
    pub cached: u64,
}

/// Parse the meminfo key set we care about.
pub fn meminfo() -> Mem {
    let mut m = Mem::default();
    if let Ok(s) = fs::read_to_string("/proc/meminfo") {
        for l in s.lines() {
            let mut p = l.split_whitespace();
            let v: u64 = p.nth(1).and_then(|x| x.parse().ok()).unwrap_or(0);
            match &l[..l.find(':').unwrap_or(0)] {
                "MemTotal" => m.total = v,
                "MemAvailable" => m.avail = v,
                "SwapTotal" => m.swap_total = v,
                "SwapFree" => m.swap_free = v,
                "Buffers" => m.buffers = v,
                "Cached" => m.cached = v,
                _ => {}
            }
        }
    }
    m
}

/// Interface counters (rx,tx bytes) — virtual devices skipped.
pub fn net_io() -> Vec<(String, u64, u64)> {
    let Ok(s) = fs::read_to_string("/proc/net/dev") else {
        return Vec::new();
    };
    s.lines()
        .skip(2)
        .filter_map(|l| {
            let (name, v) = l.split_once(':')?;
            let name = name.trim();
            if name == "lo" {
                return None;
            }
            let f: Vec<u64> = v
                .split_whitespace()
                .map(|x| x.parse().unwrap_or(0))
                .collect();
            Some((name.to_string(), *f.first()?, *f.get(8)?))
        })
        .collect()
}

/// Whole-disk byte counters — partitions (sda1, nvme0n1p1…), loop,
/// ram, zram and device-mapper are skipped: a disk shows up once,
/// like the physical drive list in the original.
pub fn disk_io() -> Vec<(String, u64, u64)> {
    let Ok(s) = fs::read_to_string("/proc/diskstats") else {
        return Vec::new();
    };
    s.lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.split_whitespace().collect();
            let name = *f.get(2)?;
            // The kernel marks partitions with a `partition` file in
            // sysfs — whole disks don't have one.
            if fs::metadata(format!("/sys/class/block/{name}/partition")).is_ok() {
                return None;
            }
            if name.starts_with("loop")
                || name.starts_with("ram")
                || name.starts_with("zram")
                || name.starts_with("dm-")
            {
                return None;
            }
            let read = f.get(5)?.parse::<u64>().ok()? * 512;
            let written = f.get(9)?.parse::<u64>().ok()? * 512;
            Some((name.to_string(), read, written))
        })
        .collect()
}

/// One GPU entry: display name, utilisation 0..1, temperature °C.
pub struct Gpu {
    pub name: String,
    pub busy: f32,
    pub temp: Option<f32>,
}

/// Enumerate /sys/class/drm/card* devices; AMD reads gpu_busy_percent,
/// NVIDIA queries nvidia-smi (driver exposes no sysfs counter).
pub fn gpus() -> Vec<Gpu> {
    let mut out = Vec::new();
    let Ok(rd) = fs::read_dir("/sys/class/drm") else {
        return out;
    };
    for e in rd.flatten() {
        let n = e.file_name().to_string_lossy().into_owned();
        if !n.starts_with("card") || n.contains('-') {
            continue;
        }
        let dev = e.path().join("device");
        let vendor = fs::read_to_string(dev.join("vendor")).unwrap_or_default();
        let name = fs::read_to_string(dev.join("product_name"))
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|_| match vendor.trim() {
                "0x10de" => "NVIDIA".into(),
                "0x1002" => "AMD".into(),
                "0x8086" => "Intel".into(),
                _ => n.clone(),
            });
        match vendor.trim() {
            "0x1002" => {
                let busy = fs::read_to_string(dev.join("gpu_busy_percent"))
                    .ok()
                    .and_then(|v| v.trim().parse::<f32>().ok())
                    .unwrap_or(0.0)
                    / 100.0;
                let temp = hwmon_temp(&dev);
                out.push(Gpu { name, busy, temp });
            }
            "0x10de" => {
                let (busy, temp) = nvidia_sample();
                out.push(Gpu { name, busy, temp });
            }
            _ => out.push(Gpu {
                name,
                busy: 0.0,
                temp: hwmon_temp(&dev),
            }),
        }
    }
    out
}

/// First millidegree temp input of the card's hwmon directory.
fn hwmon_temp(dev: &std::path::Path) -> Option<f32> {
    let rd = fs::read_dir(dev.join("hwmon")).ok()?;
    for h in rd.flatten() {
        if let Ok(v) = fs::read_to_string(h.path().join("temp1_input")) {
            if let Ok(t) = v.trim().parse::<f32>() {
                return Some(t / 1000.0);
            }
        }
    }
    None
}

/// nvidia-smi one-shot: returns (busy 0..1, temp °C).
fn nvidia_sample() -> (f32, Option<f32>) {
    let o = std::process::Command::new("nvidia-smi")
        .args([
            "--query-gpu=utilization.gpu,temperature.gpu",
            "--format=csv,noheader,nounits",
        ])
        .output();
    let Ok(o) = o else { return (0.0, None) };
    let s = String::from_utf8_lossy(&o.stdout);
    let mut p = s.trim().split(',');
    let busy = p
        .next()
        .and_then(|v| v.trim().parse::<f32>().ok())
        .unwrap_or(0.0)
        / 100.0;
    (busy, p.next().and_then(|v| v.trim().parse::<f32>().ok()))
}

/// Seconds since boot — first field of /proc/uptime.
pub fn uptime_secs() -> f64 {
    fs::read_to_string("/proc/uptime")
        .ok()
        .and_then(|s| s.split_whitespace().next()?.parse().ok())
        .unwrap_or(0.0)
}

/// "x.xx, y.yy, z.zz" load average string.
pub fn loadavg() -> String {
    fs::read_to_string("/proc/loadavg")
        .map(|s| s.split_whitespace().take(3).collect::<Vec<_>>().join(", "))
        .unwrap_or_default()
}
