//! Backend layer: reads real network state from the system without
//! any NetworkManager dependency. Data comes from `ip -json` output
//! plus targeted sysfs lookups.

pub mod collector;
pub mod diag;
pub mod diag_deep;
pub mod diag_expert;
pub mod dns;
pub mod iwdconf;
pub mod model;
pub mod netlink;
pub mod rfkill;
pub mod routing;
pub mod speed;
pub mod system;
pub mod ufw;
pub mod vpn;
pub mod wifi;

pub use collector::collect_interfaces;
pub use dns::{
    available as dns_available, flush_caches as dns_flush_caches, query as dns_query,
    set_domains as dns_set_domains, set_feature as dns_set_feature, set_servers as dns_set_servers,
    status as dns_status, DnsGlobal, DnsLink, DnsStatus,
};
pub use iwdconf::{get as iwd_option, set as iwd_set_option};
pub use model::{LinkKind, LinkState, NetworkInterface};
pub use netlink::{
    add_virtual, del_virtual, set_dhcp, set_dns, set_ipv6, set_link, set_master, set_mtu,
    set_route_metric, set_static, BOND_MODES,
};
pub use rfkill::{set_wlan_blocked, wlan_soft_blocked};
pub use routing::{
    add_route as route_add, delete_route as route_delete, routes, rules, Route, Rule,
};
pub use system::{acquire_dhcp, default_route, dns_servers, hostname};
pub use ufw::{
    add_rule as ufw_add_rule, add_rule_advanced as ufw_add_rule_advanced,
    available as ufw_available, delete_rule as ufw_delete_rule, move_rule as ufw_move_rule,
    set_default as ufw_set_default, set_enabled as ufw_set_enabled, spec_from_parsed_pub,
    status as ufw_status, AdvRule, Policy as UfwPolicy, UfwRule, UfwStatus,
};
pub use vpn::{
    delete_profile as vpn_delete, import_profile as vpn_import, ovpn_available,
    profiles as vpn_profiles, set_autostart as vpn_set_autostart, set_up as vpn_set_up,
    wg_available, VpnKind, VpnProfile,
};
pub use wifi::{
    connect as wifi_connect, connected_ssid as wifi_connected_ssid, disconnect as wifi_disconnect,
    forget as wifi_forget, is_known as wifi_is_known, scan as wifi_scan,
    ConnectOutcome as WifiConnectOutcome, WifiNetwork,
};
