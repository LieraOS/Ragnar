//! Per-network iwd profile editing. iwd stores each known network as
//! /var/lib/iwd/<name>.psk|.open|.8021x — root-owned, so all access
//! goes through the privileged helper's `iwd-profile` verb. The file
//! is INI-ish: [Settings] AutoConnect/Hidden, [Security] Passphrase,
//! [IPv4] Address/Netmask/Gateway/DNS for per-network static config.

use anyhow::{Context, Result};
use std::io::Write;
use std::process::{Command, Stdio};

/// One parsed iwd network profile.
#[derive(Debug, Default, Clone)]
pub struct Profile {
    /// The backing file name (e.g. `MyNet.psk`), None = unsaved.
    pub file: Option<String>,
    /// INI sections in order: (group, [(key, value)]).
    pub groups: Vec<(String, Vec<(String, String)>)>,
}

impl Profile {
    /// Read (or set) a key inside a [group].
    pub fn get(&self, group: &str, key: &str) -> Option<&str> {
        self.groups
            .iter()
            .find(|(g, _)| g == group)
            .and_then(|(_, kvs)| kvs.iter().find(|(k, _)| k == key))
            .map(|(_, v)| v.as_str())
    }

    pub fn set(&mut self, group: &str, key: &str, value: &str) {
        if let Some((_, kvs)) = self.groups.iter_mut().find(|(g, _)| g == group) {
            if let Some(kv) = kvs.iter_mut().find(|(k, _)| *k == key) {
                kv.1 = value.to_string();
            } else {
                kvs.push((key.to_string(), value.to_string()));
            }
        } else {
            self.groups.push((
                group.to_string(),
                vec![(key.to_string(), value.to_string())],
            ));
        }
    }

    /// Remove a key — needed by the merge-save so toggled-off
    /// options actually disappear instead of lingering.
    pub fn remove(&mut self, group: &str, key: &str) {
        if let Some((_, kvs)) = self.groups.iter_mut().find(|(g, _)| g == group) {
            kvs.retain(|(k, _)| k != key);
        }
    }

    /// Serialize back to iwd's INI format.
    pub fn to_ini(&self) -> String {
        let mut out = String::new();
        for (g, kvs) in &self.groups {
            out.push_str(&format!("[{g}]\n"));
            for (k, v) in kvs {
                out.push_str(&format!("{k}={v}\n"));
            }
            out.push('\n');
        }
        out
    }
}

/// Parse an INI profile (comments/blank lines skipped).
pub fn parse(text: &str) -> Profile {
    let mut p = Profile::default();
    let mut cur: Option<usize> = None;
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with('[') && t.ends_with(']') {
            p.groups.push((t[1..t.len() - 1].to_string(), Vec::new()));
            cur = Some(p.groups.len() - 1);
        } else if let Some((k, v)) = t.split_once('=') {
            if let Some(i) = cur {
                p.groups[i]
                    .1
                    .push((k.trim().to_string(), v.trim().to_string()));
            }
        }
    }
    p
}

fn helper(args: &[&str], stdin: Option<&str>) -> Result<String> {
    let prog = if std::path::Path::new(crate::backend::ufw::HELPER_PATH).is_file() {
        crate::backend::ufw::HELPER_PATH.to_string()
    } else {
        return Err(anyhow::anyhow!("helper not installed"));
    };
    let mut cmd = Command::new("pkexec");
    cmd.arg(&prog).arg("iwd-profile").args(args);
    if stdin.is_some() {
        cmd.stdin(Stdio::piped());
    }
    let mut child = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
    if let Some(text) = stdin {
        child.stdin.take().unwrap().write_all(text.as_bytes())?;
    }
    let out = child.wait_with_output()?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        anyhow::bail!(
            "iwd-profile failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )
    }
}

/// Map an SSID to its profile file name as iwd names it:
/// `<ssid>.psk` for WPA, `.open` for open networks, `.8021x` for
/// enterprise. The exact suffix comes from the iwd listing.
pub fn file_for(ssid: &str) -> Result<Option<String>> {
    let list = helper(&["list"], None).context("iwd-profile list")?;
    for line in list.lines() {
        let f = line.trim();
        if let Some(stem) = f
            .strip_suffix(".psk")
            .or_else(|| f.strip_suffix(".open"))
            .or_else(|| f.strip_suffix(".8021x"))
        {
            if stem == ssid {
                return Ok(Some(f.to_string()));
            }
        }
    }
    Ok(None)
}

/// Load a network's profile; returns an empty one when none exists.
pub fn load(ssid: &str) -> Result<Profile> {
    match file_for(ssid)? {
        Some(f) => {
            let text = helper(&["read", &f], None).context("iwd-profile read")?;
            let mut p = parse(&text);
            p.file = Some(f);
            Ok(p)
        }
        None => Ok(Profile::default()),
    }
}

/// Persist a profile — writes to `file` or creates `<ssid>.psk`
/// when the profile is new.
pub fn save(ssid: &str, p: &Profile) -> Result<()> {
    let file = p.file.clone().unwrap_or_else(|| format!("{}.psk", ssid));
    helper(&["write", &file], Some(&p.to_ini())).context("iwd-profile write")?;
    Ok(())
}
