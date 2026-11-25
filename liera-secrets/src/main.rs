// Copyright (C) Jörg Schröder
//! liera-secrets — small CLI over the shared credential store.
//! Usage:
//!   liera-secrets store <service> <url> <user> <password>
//!   liera-secrets get <service>
//!   liera-secrets delete <service>
//!   liera-secrets list

use liera_secrets::CloudAccount;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cmd = args.get(1).map(|s| s.as_str());
    let res = match (cmd, args.len()) {
        (Some("store"), 6) => liera_secrets::store(&CloudAccount {
            service: args[2].clone(),
            url: args[3].clone(),
            user: args[4].clone(),
            password: args[5].clone(),
        })
        .map(|_| "stored".to_string()),
        (Some("get"), 3) => liera_secrets::get(&args[2]).map(|a| match a {
            Some(a) => format!("url={} user={} pass={}", a.url, a.user, a.password),
            None => "not found".into(),
        }),
        (Some("delete"), 3) => liera_secrets::delete(&args[2]).map(|_| "deleted".to_string()),
        (Some("list"), 2) => liera_secrets::list().map(|v| v.join("\n")),
        _ => Err(
            "usage: liera-secrets <store|get|delete|list> [service url user password]"
                .into(),
        ),
    };
    match res {
        Ok(s) => println!("{s}"),
        Err(e) => {
            eprintln!("liera-secrets: {e}");
            std::process::exit(1);
        }
    }
}
