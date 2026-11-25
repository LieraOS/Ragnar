// Copyright (C) Jörg Schröder
// Hardware-temperature applets — CPU / GPU / RAM / motherboard.
// Reads sysfs hwmon directly (/sys/class/hwmon/*): the kernel
// exposes every vendor's sensors there — AMD k10temp, Intel
// coretemp, amdgpu, nvidia, spd51xx RAM hubs, nct/it87 boards —
// so no external tool like `sensors` is needed and it works on any
// hardware the kernel itself can see. The matched temp*_input path
// is cached per bar; missing hardware shows "—" instead of failing.
use crate::bar::State;
use gtk4::prelude::*;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

/// Which physical sensor an applet watches.
#[derive(Clone, Copy)]
pub enum Kind {
    Cpu,
    Gpu,
    Ram,
    Mb,
}

/// Scan /sys/class/hwmon for a temp*_input file belonging to `kind`.
/// Vendor matching by the chip's `name` file — the kernel's own
/// classification, so it covers every driver automatically.
/// Chip names the kernel uses for each kind — broad vendor coverage:
/// AMD/Intel/ARM CPUs, every common GPU driver, DDR3-5 SPD modules
/// and the usual super-io / ACPI board sensors.
fn chip_matches(kind: Kind, name: &str) -> bool {
    match kind {
        Kind::Cpu => matches!(
            name,
            "k10temp" | "k8temp" | "coretemp" | "zenpower" | "cpu_thermal" | "x86_pkg_temp"
        ),
        Kind::Gpu => {
            matches!(
                name,
                "amdgpu" | "nouveau" | "nvidia" | "i915" | "xe" | "radeon"
            ) || name.contains("gpu")
        }
        Kind::Ram => {
            name.starts_with("spd5") || name.starts_with("ee1004") || name.starts_with("jc42")
        }
        Kind::Mb => {
            name.starts_with("nct")
                || name.starts_with("it8")
                || name.starts_with("w83627")
                || matches!(
                    name,
                    "atk0110" | "asb100" | "acpitz" | "thinkpad" | "applesmc" | "dell_smm"
                )
        }
    }
}

/// Some boards name the input itself (temp1_label = "MB Temp") —
/// check labels too so sensor-role beats chip-name guessing.
fn label_matches(kind: Kind, label: &str) -> bool {
    let l = label.to_lowercase();
    match kind {
        Kind::Cpu => {
            l.contains("cpu") || l.contains("tctl") || l.contains("tdie") || l.contains("package")
        }
        Kind::Gpu => l.contains("gpu") || l.contains("edge") || l.contains("junction"),
        Kind::Ram => l.contains("dimm") || l.contains("ram") || l.contains("spd"),
        Kind::Mb => {
            l.contains("mb") || l.contains("mother") || l.contains("pch") || l.contains("aux")
        }
    }
}

fn find_input(kind: Kind) -> Option<PathBuf> {
    // Pass 1: hwmon chips matching by name or by temp*_label.
    let mut fallback: Option<PathBuf> = None;
    let hwmons = std::fs::read_dir("/sys/class/hwmon").ok()?;
    for dev in hwmons.flatten() {
        let dir = dev.path();
        let name = std::fs::read_to_string(dir.join("name"))
            .map(|s| s.trim().to_lowercase())
            .unwrap_or_default();
        let by_name = chip_matches(kind, &name);
        for n in 1..10 {
            let inp = dir.join(format!("temp{n}_input"));
            if !inp.exists() {
                continue;
            }
            let lbl = std::fs::read_to_string(dir.join(format!("temp{n}_label")))
                .map(|s| s.trim().to_lowercase())
                .unwrap_or_default();
            if (by_name && (lbl.is_empty() || label_matches(kind, &lbl)))
                || (!by_name && label_matches(kind, &lbl))
            {
                return Some(inp);
            }
            if by_name && fallback.is_none() {
                fallback = Some(inp);
            }
        }
    }
    if fallback.is_some() {
        return fallback;
    }
    // Pass 2: ACPI thermal zones — CPU via pkg_temp, MB via acpitz.
    let want = match kind {
        Kind::Cpu => vec!["x86_pkg_temp", "cpu", "soc"],
        Kind::Mb => vec!["acpitz", "pch", "pch_skylake"],
        _ => vec![],
    };
    if want.is_empty() {
        return None;
    }
    let zones = std::fs::read_dir("/sys/class/thermal").ok()?;
    for z in zones.flatten() {
        let ty = std::fs::read_to_string(z.path().join("type"))
            .map(|s| s.trim().to_lowercase())
            .unwrap_or_default();
        if want.iter().any(|w| ty.contains(w)) {
            let t = z.path().join("temp");
            if t.exists() {
                return Some(t);
            }
        }
    }
    None
}

/// Where a temperature comes from: a sysfs file to read, or a
/// vendor command whose first stdout number is degrees C.
enum Src {
    File(PathBuf),
    Cmd(&'static str),
}

/// Pick the source for `kind`: sysfs hwmon first — and for GPUs,
/// nvidia-smi as fallback, since proprietary Nvidia drivers (older
/// cards especially, e.g. Pascal) often register no hwmon at all.
fn find_source(kind: Kind) -> Option<Src> {
    if let Some(p) = find_input(kind) {
        return Some(Src::File(p));
    }
    if matches!(kind, Kind::Gpu) && std::path::Path::new("/usr/bin/nvidia-smi").exists() {
        return Some(Src::Cmd(
            "nvidia-smi --query-gpu=temperature.gpu --format=csv,noheader,nounits | head -1",
        ));
    }
    None
}

/// Build one sensor applet: icon-less text label `Name 42 °C`,
/// refreshed every 3 s.
pub fn build(_state: &State, kind: Kind, vertical: bool) -> gtk4::Widget {
    let name = match kind {
        Kind::Cpu => "CPU",
        Kind::Gpu => "GPU",
        Kind::Ram => "RAM",
        Kind::Mb => "MB",
    };
    let label = gtk4::Label::new(Some(&format!("{name} —")));
    label.set_tooltip_text(Some(&crate::i18n::trf("{}-Temperatur", &[name])));

    // Cache the source once; sensors rarely change.
    let src: Rc<RefCell<Option<Src>>> = Rc::new(RefCell::new(find_source(kind)));
    let name = name.to_string();
    let tick = {
        let src = src.clone();
        move |l: &gtk4::Label| {
            match src.borrow().as_ref() {
                // sysfs reads are microseconds — fine on the main loop.
                Some(Src::File(p)) => {
                    let val = std::fs::read_to_string(p)
                        .ok()
                        .and_then(|s| s.trim().parse::<i64>().ok())
                        .map(|m| m / 1000);
                    l.set_text(&match val {
                        Some(t) => format!("{name} {t} °C"),
                        None => format!("{name} —"),
                    });
                }
                // Vendor tools (nvidia-smi) are heavy binaries — run
                // them off the main loop or a slow GPU driver call
                // freezes the entire bar.
                Some(Src::Cmd(c)) => {
                    let (l2, name2, c2) = (l.clone(), name.clone(), *c);
                    gtk4::glib::spawn_future_local(async move {
                        let val = run_temp_cmd(c2).await;
                        l2.set_text(&match val {
                            Some(t) => format!("{name2} {t} °C"),
                            None => format!("{name2} —"),
                        });
                    });
                }
                None => l.set_text(&format!("{name} —")),
            }
        }
    };
    tick(&label);
    let l = label.clone();
    gtk4::glib::timeout_add_seconds_local(3, move || {
        tick(&l);
        gtk4::glib::ControlFlow::Continue
    });
    let _ = vertical;
    label.upcast()
}

/// Run a vendor temperature command off the main loop and parse its
/// first stdout number as whole degrees C. Async via gio::Subprocess —
/// a hanging vendor tool can never stall the bar.
async fn run_temp_cmd(c: &'static str) -> Option<i64> {
    use gtk4::gio;
    let p = gio::Subprocess::newv(
        &[
            std::ffi::OsStr::new("/bin/sh"),
            std::ffi::OsStr::new("-c"),
            std::ffi::OsStr::new(c),
        ],
        gio::SubprocessFlags::STDOUT_PIPE | gio::SubprocessFlags::STDERR_SILENCE,
    )
    .ok()?;
    let (stdout, _) = p.communicate_utf8_future(None).await.ok()?;
    stdout?.trim().parse::<i64>().ok()
}
