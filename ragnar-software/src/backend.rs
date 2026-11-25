// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
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
    /// A package just entered a lifecycle phase (PK_INFO code, name)
    /// — drives the live status line and the detail log.
    InstallPhase(u32, String),
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

/// Flatpak updates pending in user scope, as update items. The id is
/// "flatpak:<origin>:<ref>" so the install/detail paths can tell
/// them apart from PackageKit ids. Read-only query — no polkit.
fn flatpak_updates() -> Vec<packagekit::UpdateItem> {
    let Ok(out) = std::process::Command::new("flatpak")
        .args([
            "remote-ls",
            "--user",
            "--updates",
            "--columns=name,ref,version,origin",
        ])
        .output()
    else {
        return Vec::new();
    };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| {
            let c: Vec<&str> = l.split('\t').collect();
            let (name, refr, ver, origin) = (
                c.first().copied().unwrap_or(""),
                c.get(1).copied().unwrap_or(""),
                c.get(2).copied().unwrap_or(""),
                c.get(3).copied().unwrap_or("flathub"),
            );
            if refr.is_empty() {
                return None;
            }
            Some(packagekit::UpdateItem {
                id: format!("flatpak:{origin}:{refr}"),
                // name column may be empty for runtimes — fall back
                // to the id component of the ref (app/org.id/…)
                name: if name.is_empty() {
                    refr.split('/').nth(1).unwrap_or(refr).to_string()
                } else {
                    name.to_string()
                },
                version: ver.to_string(),
                severity: 0,
                summary: gettextrs::gettext("Flatpak update"),
                flatpak: true,
            })
        })
        .collect()
}

/// Spawn the update-check worker.
pub fn fetch(tx: Sender<Msg>) {
    thread::spawn(move || {
        let _ = tx.send(match packagekit::fetch_updates() {
            Ok(mut list) => {
                list.extend(flatpak_updates());
                Msg::Updates(list)
            }
            Err(e) => Msg::FetchFailed(e),
        });
    });
}

/// Spawn the install worker for the selected package ids.
pub fn install(ids: Vec<String>, tx: Sender<Msg>) {
    // Open the progress UI before the first PK call — the polkit
    // wait inside it must not leave the user staring at nothing.
    let _ = tx.send(Msg::InstallStart);
    thread::spawn(move || {
        // Split the selection: flatpak refs update through the CLI
        // (user scope, no polkit), the rest through PackageKit.
        let (fp_ids, pk_ids): (Vec<String>, Vec<String>) =
            ids.into_iter().partition(|i| i.starts_with("flatpak:"));
        let mut fp_err: Option<String> = None;
        if !fp_ids.is_empty() {
            // When both sources run, flatpak owns progress 0–50,
            // PackageKit gets scaled into 50–100.
            let span = if pk_ids.is_empty() { 100 } else { 50 } as f64;
            for (i, fid) in fp_ids.iter().enumerate() {
                // "flatpak:origin:kind/org.id/arch/branch" → ref
                let refr = fid
                    .split_once(':')
                    .and_then(|(_, r)| r.split_once(':'))
                    .map(|x| x.1)
                    .unwrap_or(fid);
                let short = refr.split('/').nth(1).unwrap_or(refr);
                // 12 = PK_INFO_UPDATING — the flatpak path reuses
                // the PackageKit phase vocabulary for the log.
                let _ = tx.send(Msg::InstallPhase(12, short.to_string()));
                let r = std::process::Command::new("flatpak")
                    .args(["update", "-y", "--noninteractive", "--user", refr])
                    .output();
                let _ = tx.send(Msg::InstallProgress(
                    ((i + 1) as f64 / fp_ids.len() as f64 * span) as u32,
                ));
                match r {
                    Ok(o) if o.status.success() => {}
                    Ok(o) => {
                        fp_err = Some(String::from_utf8_lossy(&o.stderr).trim().to_string());
                        break;
                    }
                    Err(e) => {
                        fp_err = Some(e.to_string());
                        break;
                    }
                }
            }
        }
        let tx2 = tx.clone();
        let base = if fp_ids.is_empty() { 0 } else { 50 } as f64;
        let scale = if pk_ids.is_empty() || fp_ids.is_empty() {
            1.0
        } else {
            0.5
        };
        let result = packagekit::install_updates(pk_ids, move |pct, info, name| {
            let _ = tx2.send(if name.is_empty() {
                Msg::InstallProgress((pct as f64 * scale + base) as u32)
            } else {
                Msg::InstallPhase(info, name)
            });
        });
        let _ = tx.send(Msg::InstallDone(result.and(fp_err.map_or(Ok(()), Err))));
    });
}

/// Flatpak update detail: `flatpak remote-info` on the pending ref
/// yields version, commit and download size — no changelog stream
/// exists for runtime updates, the raw info is shown as-is.
fn flatpak_detail(fid: &str) -> Result<packagekit::DetailInfo, String> {
    let (origin, refr) = fid
        .split_once(':')
        .ok_or_else(|| "bad flatpak id".to_string())?;
    let out = std::process::Command::new("flatpak")
        .args(["remote-info", "--user", origin, refr])
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(packagekit::DetailInfo {
        updates: String::new(),
        text: String::from_utf8_lossy(&out.stdout).trim().to_string(),
        issued: String::new(),
    })
}

/// Spawn the detail worker for one package id.
pub fn detail(id: String, tx: Sender<Msg>) {
    thread::spawn(move || {
        let _ = tx.send(if let Some(fid) = id.strip_prefix("flatpak:") {
            match flatpak_detail(fid) {
                Ok(d) => Msg::Detail(d),
                Err(e) => Msg::DetailFailed(e),
            }
        } else {
            match packagekit::fetch_detail(&id) {
                Ok(d) => Msg::Detail(d),
                Err(e) => Msg::DetailFailed(e),
            }
        });
    });
}

/// Spawn a dry-run resolving which extra packages the selection pulls.
/// Parses `apt-get -s` under LC_ALL=C so the marker phrase is stable
/// across locales — matching translated output broke the preview on
/// German systems (and would break on every other language too).
/// The NEW-packages block is the honest answer for updates: new
/// dependencies land there, not under "additional packages" (which
/// only lists deps of freshly requested installs).
pub fn deps_preview(ids: Vec<String>, tx: Sender<Msg>) {
    thread::spawn(move || {
        let names: Vec<String> = ids
            .iter()
            // flatpak refs are no apt packages — their runtimes
            // resolve automatically, nothing to preview
            .filter(|i| !i.starts_with("flatpak:"))
            .map(|i| i.split(';').next().unwrap_or("").to_string())
            .collect();
        let mut deps = Vec::new();
        if !names.is_empty() {
            let mut cmd = std::process::Command::new("apt-get");
            cmd.env("LC_ALL", "C")
                .arg("-s")
                .arg("install")
                .args(&names);
            if let Ok(out) = cmd.output() {
                let txt = String::from_utf8_lossy(&out.stdout);
                let mut in_block = false;
                for line in txt.lines() {
                    if line.starts_with("The following NEW packages will be installed") {
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
        }
        // The simulation lists the requested packages themselves for
        // fresh installs — they are not "additional", drop them.
        deps.retain(|d| !names.contains(d));
        deps.sort();
        deps.dedup();
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
        let result = packagekit::install_named(&name, move |pct, info, pname| {
            let _ = tx2.send(if pname.is_empty() {
                Msg::InstallProgress(pct)
            } else {
                Msg::InstallPhase(info, pname)
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
        let result = packagekit::remove_named(&name, move |pct, info, pname| {
            let _ = tx2.send(if pname.is_empty() {
                Msg::InstallProgress(pct)
            } else {
                Msg::InstallPhase(info, pname)
            });
        });
        let _ = tx.send(Msg::InstallDone(result));
    });
}

/// Install/remove a flatpak app id via the flatpak CLI.
fn flatpak_tx(name: String, remove: bool, tx: Sender<Msg>) {
    let _ = tx.send(Msg::InstallStart);
    thread::spawn(move || {
        let (args, phase) = if remove {
            (
                vec!["uninstall", "-y", name.as_str()],
                14, // PK_INFO_REMOVING
            )
        } else {
            (
                vec!["install", "-y", "--user", "flathub", name.as_str()],
                13, // PK_INFO_INSTALLING
            )
        };
        let _ = tx.send(Msg::InstallPhase(phase, name.clone()));
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
            let r = packagekit::install_file(&path, move |pct, info, n| {
                let _ = tx2.send(if n.is_empty() {
                    Msg::InstallProgress(pct)
                } else {
                    Msg::InstallPhase(info, n)
                });
            });
            let _ = tx.send(Msg::InstallDone(r));
        });
    }
}
