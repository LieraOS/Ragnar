// Fixture tests for the rcxml text patcher — see mod.rs.
use super::*;

// End-to-end patcher checks against a fixture rc.xml in a scratch
// XDG_CONFIG_HOME. Kept in one test because the env override is
// process-global and would race across tests.

const FIXTURE: &str = "<labwc_config version=\"5\">\n\
    \t<core>\n\
    \t\t<autoEnableOutputs>yes</autoEnableOutputs>\n\
    \t</core>\n\
    \t<theme>\n\
    \t\t<dropShadows>yes</dropShadows>\n\
    \t</theme>\n\
    \t<snapping>\n\
    \t\t<range inner=\"10\" outer=\"10\">0</range>\n\
    \t</snapping>\n\
    \t<font place=\"ActiveWindow\" name=\"Sans\" weight=\"Bold\" />\n\
    \t<font place=\"InactiveWindow\" name=\"Sans\" weight=\"Normal\" />\n\
    </labwc_config>\n";

#[test]
fn patcher_roundtrip() {
    let dir = std::env::temp_dir().join(format!("rcxml-test-{}", std::process::id()));
    let cfg = dir.join("ragnar-desktop/labwc");
    std::fs::create_dir_all(&cfg).unwrap();
    std::fs::write(cfg.join("rc.xml"), FIXTURE).unwrap();
    std::env::set_var("XDG_CONFIG_HOME", &dir);

    // Existing element inside an existing block.
    set_section("theme", "dropShadows", "no");
    assert_eq!(get_section("theme", "dropShadows").as_deref(), Some("no"));

    // New element inside an existing block.
    set_section("theme", "cornerRadius", "8");
    let t = read();
    assert!(t.contains("<cornerRadius>8</cornerRadius>"));
    // Original content survives.
    assert!(t.contains("<autoEnableOutputs>yes</autoEnableOutputs>"));

    // Missing top-level section is created.
    set_section("focus", "followMouse", "yes");
    let t = read();
    assert!(t.contains("<focus>"));
    assert!(t.contains("<followMouse>yes</followMouse>"));
    // And not nested inside another section.
    assert!(t.find("<focus>").unwrap() > t.find("</snapping>").unwrap());

    // Attribute write on an existing element.
    set_attr("range", "inner", "20");
    assert_eq!(get_attr("range", "inner").as_deref(), Some("20"));

    // Attribute write creates a missing element.
    set_attr("desktops", "number", "6");
    let t = read();
    assert!(t.contains("<desktops number=\"6\""));

    // Nested block via parent: <theme><titlebar><layout>.
    set_in(
        "<titlebar>",
        "</titlebar>",
        "layout",
        ":close:",
        Some("theme"),
    );
    let t = read();
    let theme_start = t.find("<theme>").unwrap();
    let theme_end = t.find("</theme>").unwrap();
    let titlebar = t.find("<titlebar>").unwrap();
    assert!(titlebar > theme_start && titlebar < theme_end);
    assert_eq!(
        get_in("<titlebar>", "</titlebar>", "layout").as_deref(),
        Some(":close:")
    );

    // Attribute-keyed element: only the ActiveWindow font may
    // change, InactiveWindow stays untouched.
    set_attr("font place=\"ActiveWindow\"", "name", "Inter");
    let t = read();
    assert!(t.contains("<font place=\"ActiveWindow\" name=\"Inter\" weight=\"Bold\" />"));
    assert!(t.contains("<font place=\"InactiveWindow\" name=\"Sans\" weight=\"Normal\" />"));

    // Device block keyed by attribute.
    set_device("touchpad", "naturalScroll", "yes");
    assert_eq!(
        get_device("touchpad", "naturalScroll").as_deref(),
        Some("yes")
    );

    // Workspace names: positional, empty slots get the
    // generated "<prefix> N" fallback; a second write replaces
    // the whole block in place.
    set_names(&["Dev".to_string(), String::new(), "Games".to_string()]);
    let t = read();
    assert!(t.contains("<names>"));
    assert_eq!(
        get_names(),
        vec![
            "Dev".to_string(),
            "Workspace 2".to_string(),
            "Games".to_string()
        ]
    );
    set_names(&["Main".to_string()]);
    assert_eq!(get_names(), vec!["Main".to_string()]);
    assert_eq!(read().matches("<names>").count(), 1);

    let _ = std::fs::remove_dir_all(&dir);
}
