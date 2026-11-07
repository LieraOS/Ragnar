fn main() {
    tracing_subscriber::fmt().with_env_filter("debug").init();
    let conn = zbus::blocking::Connection::system().unwrap();
    let root = zbus::blocking::Proxy::new(
        &conn,
        "fi.w1.wpa_supplicant1",
        "/fi/w1/wpa_supplicant1",
        "fi.w1.wpa_supplicant1",
    )
    .unwrap();
    let path: zbus::zvariant::OwnedObjectPath = root.call("GetInterface", &("wlp4s0",)).unwrap();
    println!("iface path: {}", path.as_str());
    let dev = zbus::blocking::Proxy::new(
        &conn,
        "fi.w1.wpa_supplicant1",
        path.as_str(),
        "fi.w1.wpa_supplicant1.Interface",
    )
    .unwrap();
    let bss: Vec<zbus::zvariant::OwnedObjectPath> = dev.get_property("BSSs").unwrap();
    println!("BSS count: {}", bss.len());
    for p in bss.iter().take(3) {
        let b = zbus::blocking::Proxy::new(
            &conn,
            "fi.w1.wpa_supplicant1",
            p.as_str(),
            "fi.w1.wpa_supplicant1.BSS",
        )
        .unwrap();
        let raw: Result<Vec<u8>, _> = b.get_property("SSID");
        println!("SSID prop: {raw:?}");
        let sig: Result<i16, _> = b.get_property("Signal");
        println!("Signal prop: {sig:?}");
    }
}
