// SPDX-License-Identifier: GPL-3.0-or-later
//! PackageKit system-bus client.
//!
//! packagekitd exposes org.freedesktop.PackageKit on the system bus.
//! CreateTransaction() hands out a fresh transaction object carrying
//! the org.freedesktop.PackageKit.Transaction interface; long-running
//! jobs (GetUpdates, UpdatePackages, InstallFiles) return immediately
//! and report progress via signals on that object.
//!
//! One dedicated listener thread per signal member, each on its own
//! connection: zbus blocking signal iterators are synchronous —
//! `receive_signal()` returns only once the match rule is active.
//! A ready-barrier waits for every listener before the transaction
//! is started, so no signal can be missed.

use std::sync::mpsc::{channel, Sender};
use std::thread;

use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::OwnedObjectPath;

const SERVICE: &str = "org.freedesktop.PackageKit";
const ROOT: &str = "/org/freedesktop/PackageKit";
const IFACE_ROOT: &str = "org.freedesktop.PackageKit";
const IFACE_TX: &str = "org.freedesktop.PackageKit.Transaction";

/// PK_INFO severities that describe a listable update — anything else
/// is a lifecycle/state marker (installed, downloading, cleanup, …).
const UPDATE_INFO: &[u32] = &[1, 2, 3, 4, 5, 6, 7, 8, 9];
/// PK_EXIT success code.
const EXIT_SUCCESS: u32 = 1;

/// One available update as reported by the Package signal.
#[derive(Debug, Clone)]
pub struct UpdateItem {
    /// Raw PackageKit id "name;version;arch;repo".
    pub id: String,
    /// Display name parsed from the id.
    pub name: String,
    /// Version parsed from the id.
    pub version: String,
    /// PK_INFO severity (3 low … 9 critical).
    pub severity: u32,
    /// Short description carried by the Package signal.
    pub summary: String,
}

/// Human-readable label for a PK_INFO severity.
pub fn severity_label(severity: u32) -> &'static str {
    match severity {
        3 => "Gering",
        4 => "Verbesserung",
        5 => "Normal",
        6 => "Fehlerbehebung",
        7 => "Wichtig",
        8 => "Sicherheit",
        9 => "Kritisch",
        _ => "",
    }
}

/// Event delivered from the signal listener threads.
enum Ev {
    Pkg(UpdateItem),
    Name(String),
    Detail(String, String, String),
    Progress(u32),
    Finished(u32),
    Error(String),
}

/// Open a fresh transaction object on `conn` → (path, call proxy).
fn transaction(conn: &Connection) -> Result<(OwnedObjectPath, Proxy<'static>), String> {
    let root = Proxy::new(conn, SERVICE, ROOT, IFACE_ROOT).map_err(|e| e.to_string())?;
    let path: OwnedObjectPath = root
        .call("CreateTransaction", &())
        .map_err(|e| e.to_string())?;
    let proxy = Proxy::new(conn, SERVICE, path.to_string(), IFACE_TX).map_err(|e| e.to_string())?;
    Ok((path, proxy))
}

/// Spawn a blocking signal listener for `member` on the transaction
/// `path`. Own connection per listener — iterators cannot share one.
/// `ready` fires once `receive_signal()` completed (rule is active).
fn spawn_listener(path: String, member: &'static str, ready: Sender<()>, events: Sender<Ev>) {
    thread::spawn(move || {
        let conn = match Connection::system() {
            Ok(c) => c,
            Err(e) => {
                let _ = events.send(Ev::Error(e.to_string()));
                return;
            }
        };
        let proxy = match Proxy::new(&conn, SERVICE, path, IFACE_TX) {
            Ok(p) => p,
            Err(e) => {
                let _ = events.send(Ev::Error(e.to_string()));
                return;
            }
        };
        let mut it = match proxy.receive_signal(member) {
            Ok(i) => i,
            Err(e) => {
                let _ = events.send(Ev::Error(e.to_string()));
                return;
            }
        };
        let _ = ready.send(());
        for sig in it.by_ref() {
            let ev = match member {
                "Package" => sig
                    .body()
                    .deserialize::<(u32, String, String)>()
                    .ok()
                    .and_then(|(raw, id, summary)| {
                        // packagekitd ORs state flags into the upper
                        // bits (e.g. 0x50005 = "available" + NORMAL) —
                        // the severity itself lives in the low 16.
                        let info = raw & 0xFFFF;
                        if matches!(info, 11..=13) {
                            // downloading/updating/installing — used
                            // during UpdatePackages for live status.
                            return Some(Ev::Name(id.split(';').next().unwrap_or(&id).to_string()));
                        }
                        if UPDATE_INFO.contains(&info) {
                            let mut p = id.splitn(4, ';');
                            Some(Ev::Pkg(UpdateItem {
                                name: p.next().unwrap_or_default().to_string(),
                                version: p.next().unwrap_or_default().to_string(),
                                severity: info,
                                summary,
                                id,
                            }))
                        } else {
                            None
                        }
                    }),
                "Percentage" => sig
                    .body()
                    .deserialize::<u32>()
                    .ok()
                    .map(|p| Ev::Progress(p.min(100))),
                "ErrorCode" => sig
                    .body()
                    .deserialize::<(u32, String)>()
                    .ok()
                    .map(|(_c, d)| Ev::Error(d)),
                "UpdateDetail" => sig
                    .body()
                    .deserialize::<(
                        String,
                        Vec<String>,
                        Vec<String>,
                        Vec<String>,
                        Vec<String>,
                        Vec<String>,
                        u32,
                        String,
                        String,
                        u32,
                        u64,
                        u64,
                    )>()
                    .ok()
                    .map(
                        |(_id, ups, _obs, _vu, _bu, _cv, _re, text, _cl, _st, iss, _up)| {
                            Ev::Detail(ups.join(" "), text, iss.to_string())
                        },
                    ),
                "Finished" => sig
                    .body()
                    .deserialize::<(u32, u32)>()
                    .ok()
                    .map(|(exit, _)| Ev::Finished(exit)),
                _ => None,
            };
            if let Some(e) = ev {
                let _ = events.send(e);
            }
        }
    });
}

/// Mark the transaction interactive so polkit is allowed to show
/// the auth dialog (without the hint, packagekitd fails auth outright).
fn set_interactive(tx: &Proxy) {
    let _ = tx.call::<_, _, ()>(
        "SetHints",
        &(vec!["interactive=true", "frontend=ragnar-software"],),
    );
}

/// Query pending updates. Blocks until Finished; runs on a worker.
pub fn fetch_updates() -> Result<Vec<UpdateItem>, String> {
    let call_conn = Connection::system().map_err(|e| e.to_string())?;
    let (path, tx) = transaction(&call_conn)?;
    let path_s = path.to_string();

    let (events, rx) = channel::<Ev>();
    let (ready_tx, ready_rx) = channel::<()>();
    for m in ["Package", "Finished", "ErrorCode"] {
        spawn_listener(path_s.clone(), m, ready_tx.clone(), events.clone());
    }
    for _ in 0..3 {
        ready_rx.recv().map_err(|_| "listener died".to_string())?;
    }
    tx.call::<_, _, ()>("GetUpdates", &(0u64,))
        .map_err(|e| e.to_string())?;

    let mut out = Vec::new();
    while let Ok(ev) = rx.recv() {
        match ev {
            Ev::Pkg(it) => out.push(it),
            Ev::Error(d) => return Err(d),
            Ev::Finished(_) => break,
            _ => {}
        }
    }
    if std::env::var("RS_DEBUG").is_ok() {
        eprintln!("[pk-debug] fetched {} updates", out.len());
    }
    Ok(out)
}

/// Install the selected update package ids. Reports overall progress
/// (0–100) through `progress`; returns Err on PackageKit failure.
pub fn install_updates(
    ids: Vec<String>,
    mut progress: impl FnMut(u32, String),
) -> Result<(), String> {
    if ids.is_empty() {
        return Ok(());
    }
    let call_conn = Connection::system().map_err(|e| e.to_string())?;
    let (path, tx) = transaction(&call_conn)?;
    let path_s = path.to_string();

    let sig_path = format!("/{}", path_s.rsplit('/').next().unwrap_or(&path_s));

    let (events, rx) = channel::<Ev>();
    let (ready_tx, ready_rx) = channel::<()>();
    for m in ["Percentage", "Package", "Finished", "ErrorCode"] {
        spawn_listener(sig_path.clone(), m, ready_tx.clone(), events.clone());
    }
    for _ in 0..4 {
        ready_rx.recv().map_err(|_| "listener died".to_string())?;
    }
    set_interactive(&tx);
    tx.call::<_, _, ()>("UpdatePackages", &(0x2u64, &ids))
        .map_err(|e| e.to_string())?;

    while let Ok(ev) = rx.recv() {
        match ev {
            Ev::Progress(p) => progress(p, String::new()),
            Ev::Name(n) => progress(0, n),
            Ev::Error(d) => return Err(d),
            Ev::Finished(exit) => {
                return if exit == EXIT_SUCCESS {
                    Ok(())
                } else {
                    Err(format!("UpdatePackages exit code {exit}"))
                };
            }
            _ => {}
        }
    }
    Ok(())
}

/// Detail data for a single update (GetUpdateDetail signal).
#[derive(Debug, Clone)]
pub struct DetailInfo {
    /// New version(s) this update ships.
    pub updates: String,
    /// Free-text changelog/update description.
    pub text: String,
    /// Issued date if provided.
    pub issued: String,
}

/// Fetch detail info for one package id. Same listener pattern.
pub fn fetch_detail(id: &str) -> Result<DetailInfo, String> {
    let call_conn = Connection::system().map_err(|e| e.to_string())?;
    let (path, tx) = transaction(&call_conn)?;
    let path_s = path.to_string();
    let sig_path = format!("/{}", path_s.rsplit('/').next().unwrap_or(&path_s));

    let (events, rx) = channel::<Ev>();
    let (ready_tx, ready_rx) = channel::<()>();
    for m in ["UpdateDetail", "Finished", "ErrorCode"] {
        spawn_listener(sig_path.clone(), m, ready_tx.clone(), events.clone());
    }
    for _ in 0..3 {
        ready_rx.recv().map_err(|_| "listener died".to_string())?;
    }
    let ids = vec![id.to_string()];
    tx.call::<_, _, ()>("GetUpdateDetail", &(ids,))
        .map_err(|e| e.to_string())?;

    let mut detail = DetailInfo {
        updates: String::new(),
        text: String::new(),
        issued: String::new(),
    };
    while let Ok(ev) = rx.recv() {
        match ev {
            Ev::Detail(u, t, i) => {
                detail.updates = u;
                detail.text = t;
                detail.issued = i;
            }
            Ev::Error(d) => return Err(d),
            Ev::Finished(_) => break,
            _ => {}
        }
    }
    Ok(detail)
}

/// Install one package by name: Resolve to a full package id first,
/// then InstallPackages on it.
pub fn install_named(name: &str, mut progress: impl FnMut(u32, String)) -> Result<(), String> {
    let call_conn = Connection::system().map_err(|e| e.to_string())?;
    let (path, tx) = transaction(&call_conn)?;
    let path_s = path.to_string();
    let sig_path = format!("/{}", path_s.rsplit('/').next().unwrap_or(&path_s));

    let (events, rx) = channel::<Ev>();
    let (ready_tx, ready_rx) = channel::<()>();
    for m in ["Package", "Finished", "ErrorCode"] {
        spawn_listener(sig_path.clone(), m, ready_tx.clone(), events.clone());
    }
    for _ in 0..3 {
        ready_rx.recv().map_err(|_| "listener died".to_string())?;
    }
    set_interactive(&tx);
    tx.call::<_, _, ()>("Resolve", &(0u64, &vec![name.to_string()]))
        .map_err(|e| e.to_string())?;
    let mut ids = Vec::new();
    while let Ok(ev) = rx.recv() {
        match ev {
            Ev::Pkg(it) => ids.push(it.id),
            Ev::Error(d) => return Err(d),
            Ev::Finished(_) => break,
            _ => {}
        }
    }
    // Resolve yields every arch (i386 + amd64 …) — InstallPackages
    // fails on foreign arches, so keep only the native one.
    let arch = match std::env::consts::ARCH {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        other => other,
    };
    ids.retain(|id| id.split(';').nth(2) == Some(arch));
    let Some(one) = ids.into_iter().next() else {
        return Err(format!("Paket '{name}' nicht gefunden"));
    };
    let ids = vec![one];
    // InstallPackages (not UpdatePackages) — new installs, not upgrades.
    let call_conn = Connection::system().map_err(|e| e.to_string())?;
    let (path, tx) = transaction(&call_conn)?;
    let sig_path = format!("/{}", path.to_string().rsplit('/').next().unwrap_or(""));
    let (events, rx) = channel::<Ev>();
    let (ready_tx, ready_rx) = channel::<()>();
    for m in ["Percentage", "Package", "Finished", "ErrorCode"] {
        spawn_listener(sig_path.clone(), m, ready_tx.clone(), events.clone());
    }
    for _ in 0..4 {
        ready_rx.recv().map_err(|_| "listener died".to_string())?;
    }
    set_interactive(&tx);
    tx.call::<_, _, ()>("InstallPackages", &(0x2u64, &ids))
        .map_err(|e| e.to_string())?;
    while let Ok(ev) = rx.recv() {
        match ev {
            Ev::Progress(p) => progress(p, String::new()),
            Ev::Name(n) => progress(0, n),
            Ev::Error(d) => return Err(d),
            Ev::Finished(exit) => {
                return if exit == EXIT_SUCCESS {
                    Ok(())
                } else {
                    Err(format!("exit {exit}"))
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// Remove one package by name: Resolve to the installed package id,
/// then RemovePackages. allow_deps=true lets PK pull dependents.
pub fn remove_named(name: &str, mut progress: impl FnMut(u32, String)) -> Result<(), String> {
    let call_conn = Connection::system().map_err(|e| e.to_string())?;
    let (path, tx) = transaction(&call_conn)?;
    let path_s = path.to_string();
    let sig_path = format!("/{}", path_s.rsplit('/').next().unwrap_or(&path_s));

    let (events, rx) = channel::<Ev>();
    let (ready_tx, ready_rx) = channel::<()>();
    for m in ["Package", "Finished", "ErrorCode"] {
        spawn_listener(sig_path.clone(), m, ready_tx.clone(), events.clone());
    }
    for _ in 0..3 {
        ready_rx.recv().map_err(|_| "listener died".to_string())?;
    }
    set_interactive(&tx);
    // Resolve everything, then keep only the installed instance —
    // the flag bit 0x40000 marks it (filter constants are unreliable).
    tx.call::<_, _, ()>("Resolve", &(0u64, &vec![name.to_string()]))
        .map_err(|e| e.to_string())?;
    let mut ids = Vec::new();
    while let Ok(ev) = rx.recv() {
        match ev {
            Ev::Pkg(it) => {
                // info 1 = PK_INFO_INSTALLED — the repo field says
                // "manual:…" on Debian, not "installed".
                if it.severity == 1 {
                    ids.push(it.id);
                }
            }
            Ev::Error(d) => return Err(d),
            Ev::Finished(_) => break,
            _ => {}
        }
    }
    if ids.is_empty() {
        return Err(format!("Paket '{name}' nicht installiert"));
    }

    // RemovePackages on a fresh transaction object.
    let (path2, tx2) = transaction(&call_conn)?;
    let sig2 = format!("/{}", path2.to_string().rsplit('/').next().unwrap_or(""));
    let (events2, rx2) = channel::<Ev>();
    let (ready2, rrx2) = channel::<()>();
    for m in ["Percentage", "Package", "Finished", "ErrorCode"] {
        spawn_listener(sig2.clone(), m, ready2.clone(), events2.clone());
    }
    for _ in 0..4 {
        rrx2.recv().map_err(|_| "listener died".to_string())?;
    }
    set_interactive(&tx2);
    tx2.call::<_, _, ()>("RemovePackages", &(0x2u64, &ids, true, false))
        .map_err(|e| e.to_string())?;
    while let Ok(ev) = rx2.recv() {
        match ev {
            Ev::Progress(p) => progress(p, String::new()),
            Ev::Name(n) => progress(0, n),
            Ev::Error(d) => return Err(d),
            Ev::Finished(exit) => {
                return if exit == EXIT_SUCCESS {
                    Ok(())
                } else {
                    Err(format!("exit {exit}"))
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// One entry in the system transaction history.
#[derive(Debug, Clone)]
pub struct HistoryEntry {
    /// "2026-11-07 09:51:29"
    pub time: String,
    /// Action label: "Installiert", "Entfernt", "Aktualisiert".
    pub action: String,
    /// Package names involved.
    pub names: String,
    /// true if the entry completed.
    pub ok: bool,
}

/// Parse /var/log/apt/history.log + flatpak history into entries
/// (newest last in file order — the UI reverses).
pub fn fetch_history() -> Vec<HistoryEntry> {
    let mut out = Vec::new();
    if let Ok(txt) = std::fs::read_to_string("/var/log/apt/history.log") {
        let mut time = String::new();
        let mut action = String::new();
        let mut names = String::new();
        for l in txt.lines() {
            if let Some(v) = l.strip_prefix("Start-Date: ") {
                time = v.trim().to_string();
                action.clear();
                names.clear();
            } else if l.starts_with("Install: ") {
                action = "Installiert".into();
                names = pkg_names(&l[9..]);
            } else if l.starts_with("Upgrade: ") {
                action = "Aktualisiert".into();
                names = pkg_names(&l[9..]);
            } else if l.starts_with("Remove: ") || l.starts_with("Purge: ") {
                action = if l.starts_with("Purge") {
                    "Gelöscht".into()
                } else {
                    "Entfernt".into()
                };
                names = pkg_names(l.split_once(':').map(|x| x.1).unwrap_or(""));
            } else if l.starts_with("End-Date:") && !names.is_empty() {
                out.push(HistoryEntry {
                    time: time.clone(),
                    action: action.clone(),
                    names: names.clone(),
                    ok: true,
                });
            }
        }
    }
    // plain `flatpak history` — tab-separated: time, action, ref,
    // version, scope, remote. Column names are not reliable.
    if let Ok(o) = std::process::Command::new("flatpak")
        .arg("history")
        .output()
    {
        for l in String::from_utf8_lossy(&o.stdout).lines() {
            let p: Vec<&str> = l.split('\t').collect();
            if p.len() >= 3 && !p[0].contains("Zeit") {
                let action = match p[1].trim() {
                    "deploy install" | "install" => "Installiert",
                    "deploy update" | "update" => "Aktualisiert",
                    "uninstall" => "Entfernt",
                    "add remote" => "Remote hinzugefügt",
                    other => other,
                };
                out.push(HistoryEntry {
                    time: p[0].trim().to_string(),
                    action: action.to_string(),
                    names: p[2].trim().to_string(),
                    ok: true,
                });
            }
        }
    }
    out
}

/// "pkg:amd64 (1.2), pkg2:amd64 (3.4)" → "pkg, pkg2".
fn pkg_names(s: &str) -> String {
    s.split(',')
        .filter_map(|p| p.trim().split(':').next().map(|x| x.trim().to_string()))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Install a local .deb file via InstallFiles. only_trusted=0 —
/// local files are inherently untrusted, pk asks the untrusted auth.
pub fn install_file(path: &str, mut progress: impl FnMut(u32, String)) -> Result<(), String> {
    let call_conn = Connection::system().map_err(|e| e.to_string())?;
    let (txpath, tx) = transaction(&call_conn)?;
    let sig_path = format!("/{}", txpath.to_string().rsplit('/').next().unwrap_or(""));
    let (events, rx) = channel::<Ev>();
    let (ready_tx, ready_rx) = channel::<()>();
    for m in ["Percentage", "Package", "Finished", "ErrorCode"] {
        spawn_listener(sig_path.clone(), m, ready_tx.clone(), events.clone());
    }
    for _ in 0..4 {
        ready_rx.recv().map_err(|_| "listener died".to_string())?;
    }
    set_interactive(&tx);
    let files = vec![path.to_string()];
    tx.call::<_, _, ()>("InstallFiles", &(0u64, &files))
        .map_err(|e| e.to_string())?;
    while let Ok(ev) = rx.recv() {
        match ev {
            Ev::Progress(p) => progress(p, String::new()),
            Ev::Name(n) => progress(0, n),
            Ev::Error(d) => return Err(d),
            Ev::Finished(exit) => {
                return if exit == EXIT_SUCCESS {
                    Ok(())
                } else {
                    Err(format!("exit {exit}"))
                }
            }
            _ => {}
        }
    }
    Ok(())
}
