// SPDX-License-Identifier: GPL-3.0-or-later
//! Backend orchestration: PackageKit calls run on worker threads and
//! report back over a glib channel, so the GTK main loop never blocks.

use std::sync::mpsc::Sender;
use std::thread;

use crate::catalog_data::AppItem;
use crate::packagekit::{self, DetailInfo, UpdateItem};

/// Messages the backend workers send into the GTK loop.
#[derive(Debug)]
pub enum Msg {
    /// Update list loaded (possibly empty).
    Updates(Vec<UpdateItem>),
    /// The fetch failed — carries a user-presentable reason.
    FetchFailed(String),
    /// Overall install progress, 0–100.
    InstallProgress(u32),
    /// Install finished: Ok = success, Err = reason.
    InstallDone(Result<(), String>),
    /// Detail info for one update arrived.
    Detail(DetailInfo),
    /// Detail fetch failed.
    DetailFailed(String),
    /// A package is currently being installed (name).
    InstallName(String),
    /// App detail text arrived: (title, body).
    AppDetail(String, String),
    /// Transaction history loaded.
    History(Vec<packagekit::HistoryEntry>),
    /// History fetch failed.
    HistoryFailed(String),
    /// A privileged transaction just started (open progress UI).
    InstallStart,
    /// AppStream catalogue loaded: apps + installed-package set.
    Apps(Vec<AppItem>, std::collections::HashSet<String>),
    /// Dependency preview for the install dialog: (selected ids,
    /// additional packages apt would pull).
    Deps(Vec<String>, Vec<String>),
}

/// Spawn the update-check worker.
pub fn fetch(tx: Sender<Msg>) {
    thread::spawn(move || {
        let _ = tx.send(match packagekit::fetch_updates() {
            Ok(list) => Msg::Updates(list),
            Err(e) => Msg::FetchFailed(e),
        });
    });
}

/// Spawn the install worker for the selected package ids.
pub fn install(ids: Vec<String>, tx: Sender<Msg>) {
    thread::spawn(move || {
        let tx2 = tx.clone();
        let result = packagekit::install_updates(ids, move |pct, name| {
            let _ = tx2.send(if name.is_empty() {
                Msg::InstallProgress(pct)
            } else {
                Msg::InstallName(name)
            });
        });
        let _ = tx.send(Msg::InstallDone(result));
    });
}

/// Spawn the detail worker for one package id.
pub fn detail(id: String, tx: Sender<Msg>) {
    thread::spawn(move || {
        let _ = tx.send(match packagekit::fetch_detail(&id) {
            Ok(d) => Msg::Detail(d),
            Err(e) => Msg::DetailFailed(e),
        });
    });
}

/// Spawn a dry-run resolving which extra packages the selection pulls.
pub fn deps_preview(ids: Vec<String>, tx: Sender<Msg>) {
    thread::spawn(move || {
        let names: Vec<String> = ids
            .iter()
            .map(|i| i.split(';').next().unwrap_or("").to_string())
            .collect();
        let mut cmd = std::process::Command::new("apt-get");
        cmd.arg("-s").arg("install").args(&names);
        let mut deps = Vec::new();
        if let Ok(out) = cmd.output() {
            let txt = String::from_utf8_lossy(&out.stdout);
            let mut in_block = false;
            for line in txt.lines() {
                if line.contains("werden zusätzlich installiert")
                    || line.contains("additional packages")
                {
                    in_block = true;
                    continue;
                }
                if in_block {
                    if line.starts_with("  ") {
                        deps.extend(line.split_whitespace().map(|x| x.to_string()));
                    } else {
                        break;
                    }
                }
            }
        }
        let _ = tx.send(Msg::Deps(ids, deps));
    });
}

/// Spawn the AppStream catalogue load on a worker.
pub fn load_catalog(tx: Sender<Msg>) {
    thread::spawn(move || {
        let apps = crate::catalog_data::load_apps();
        let mut inst = crate::catalog_data::installed_set();
        inst.extend(crate::catalog_data::flatpak_installed());
        crate::catalog_data::ensure_flathub();
        let _ = tx.send(Msg::Apps(apps, inst));
    });
}

/// Spawn a single-package install by name (Resolve→InstallPackages,
/// same progress/done messages as the update flow).
pub fn install_pkg(name: String, tx: Sender<Msg>) {
    let _ = tx.send(Msg::InstallStart);
    thread::spawn(move || {
        let tx2 = tx.clone();
        let result = packagekit::install_named(&name, move |pct, pname| {
            let _ = tx2.send(if pname.is_empty() {
                Msg::InstallProgress(pct)
            } else {
                Msg::InstallName(pname)
            });
        });
        let _ = tx.send(Msg::InstallDone(result));
    });
}

/// Spawn a single-package removal by name.
pub fn uninstall_pkg(name: String, tx: Sender<Msg>) {
    let _ = tx.send(Msg::InstallStart);
    thread::spawn(move || {
        let tx2 = tx.clone();
        let result = packagekit::remove_named(&name, move |pct, pname| {
            let _ = tx2.send(if pname.is_empty() {
                Msg::InstallProgress(pct)
            } else {
                Msg::InstallName(pname)
            });
        });
        let _ = tx.send(Msg::InstallDone(result));
    });
}

/// Install/remove a flatpak app id via the flatpak CLI.
fn flatpak_tx(name: String, remove: bool, tx: Sender<Msg>) {
    let _ = tx.send(Msg::InstallStart);
    thread::spawn(move || {
        let args = if remove {
            vec!["uninstall", "-y", name.as_str()]
        } else {
            vec!["install", "-y", "--user", "flathub", name.as_str()]
        };
        let result = std::process::Command::new("flatpak")
            .args(&args)
            .output()
            .map_err(|e| e.to_string())
            .and_then(|o| {
                if o.status.success() {
                    Ok(())
                } else {
                    Err(String::from_utf8_lossy(&o.stderr).trim().to_string())
                }
            });
        let _ = tx.send(Msg::InstallDone(result));
    });
}

/// Route by source: flatpak apps → CLI, debian packages → PackageKit.
pub fn install_pkg_any(name: String, source: String, tx: Sender<Msg>) {
    if source == "flatpak" {
        flatpak_tx(name, false, tx)
    } else {
        install_pkg(name, tx)
    }
}
pub fn uninstall_pkg_any(name: String, source: String, tx: Sender<Msg>) {
    if source == "flatpak" {
        flatpak_tx(name, true, tx)
    } else {
        uninstall_pkg(name, tx)
    }
}

/// Fetch the long description of an app: apt-cache show for debs,
/// flatpak info for flatpaks. Runs on a worker.
pub fn app_detail(name: String, source: String, tx: Sender<Msg>) {
    thread::spawn(move || {
        let body = if source == "flatpak" {
            std::process::Command::new("flatpak")
                .args(["info", &name])
                .output()
                .map(|o| {
                    let t = String::from_utf8_lossy(&o.stdout);
                    // description block lives after "Beschreibung:" /
                    // "Description:" — fall back to the whole output
                    let after = t
                        .split("Beschreibung:")
                        .nth(1)
                        .or_else(|| t.split("Description:").nth(1));
                    after.unwrap_or(&t).trim().to_string()
                })
                .unwrap_or_default()
        } else {
            std::process::Command::new("apt-cache")
                .args(["show", &name])
                .output()
                .map(|o| {
                    let t = String::from_utf8_lossy(&o.stdout);
                    let mut desc = String::new();
                    let mut in_d = false;
                    for l in t.lines() {
                        if l.starts_with("Description:") {
                            in_d = true;
                            desc.push_str(l.trim_start_matches("Description:").trim());
                            desc.push('\n');
                        } else if in_d && l.starts_with(' ') {
                            desc.push_str(l.trim());
                            desc.push('\n');
                        } else if in_d {
                            break;
                        }
                    }
                    if desc.is_empty() {
                        t.lines().next().unwrap_or("").to_string()
                    } else {
                        desc
                    }
                })
                .unwrap_or_default()
        };
        let _ = tx.send(Msg::AppDetail(name, body));
    });
}

/// Spawn the transaction-history fetch (last 50 entries).
pub fn history(tx: Sender<Msg>) {
    thread::spawn(move || {
        let _ = tx.send(match Ok(packagekit::fetch_history()) {
            Ok(h) => Msg::History(h),
            Err(e) => Msg::HistoryFailed(e),
        });
    });
}

/// Install a local file: .deb → InstallFiles, .flatpakref → flatpak.
pub fn install_local(path: String, tx: Sender<Msg>) {
    let _ = tx.send(Msg::InstallStart);
    if path.ends_with(".flatpakref") || path.ends_with(".flatpakrepo") {
        thread::spawn(move || {
            let r = std::process::Command::new("flatpak")
                .args(["install", "-y", "--user", "--from", &path])
                .output()
                .map_err(|e| e.to_string())
                .and_then(|o| {
                    if o.status.success() {
                        Ok(())
                    } else {
                        Err(String::from_utf8_lossy(&o.stderr).trim().to_string())
                    }
                });
            let _ = tx.send(Msg::InstallDone(r));
        });
    } else {
        thread::spawn(move || {
            let tx2 = tx.clone();
            let r = packagekit::install_file(&path, move |pct, n| {
                let _ = tx2.send(if n.is_empty() {
                    Msg::InstallProgress(pct)
                } else {
                    Msg::InstallName(n)
                });
            });
            let _ = tx.send(Msg::InstallDone(r));
        });
    }
}
