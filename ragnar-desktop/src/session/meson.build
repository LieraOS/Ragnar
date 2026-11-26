# Main session scripts

# We bash-quote already.
session_data = configuration_data()
session_data.set('PACKAGE_VERSION', package_version)
session_data.set('libexecdir', libexecdir)
session_data.set('libexecdirroot', libexecdirroot)

# gnome-settings-daemon components
gsd_components_excluded = get_option('gsd-components-excluded')
gsd_components = []
foreach component: get_option('gsd-components')
    if component not in gsd_components_excluded
        gsd_components += component
    endif
endforeach

# gsd components are unwanted: volume/MPRIS/power/lock run through
# PipeWire/logind/own services, so neither session entries nor
# autostart files are built from them.
ragnar_session_324_components = []

# Always in Ragnar
# Note: org.lieraos.ragnar.Panel was removed — the Rust
# ragnar-taskbar (own package, autostarted via /etc/xdg/autostart)
# is the panel now; the alten Fork panel must not load anymore.
ragnar_components = [
    # Note: org.lieraos.ragnar.Panel was removed — the Rust
    # ragnar-taskbar (own package, autostarted via /etc/xdg/autostart)
    # is the panel now; the alten Fork panel must not load anymore.
    # org.lieraos.ragnar.Daemon is gone — the daemon.s last job
    # (session registration) is no longer needed: power-dialog
    # registers itself and nothing else talked to the daemon's bus.
    'org.lieraos.ragnar.PowerDialog',
]

session_components = ragnar_components + ragnar_session_324_components

# Set the session list
session_data.set('SESSION_COMPONENTS', ';'.join(session_components))
# Set the prefix
session_data.set('prefix', join_paths(get_option('prefix')))
# Set bindir to ragnar-desktop
session_data.set('bindir', bindir)
# Set datadir to ragnar-desktop
session_data.set('datadir', datadir)

# Set bindir to labwc
labwc_bindir = get_option('labwc-bindir')
if labwc_bindir == ''
    labwc_bindir = bindir
endif
session_data.set('labwc_bindir', labwc_bindir)

# Set bindir to magpie-wm
#magpie_bindir = get_option('magpie-bindir')
#if magpie_bindir == ''
#    magpie_bindir = bindir
#endif
#session_data.set('magpie_bindir', magpie_bindir)

# Set libexecdir to ragnar-session
ragnar_session_libexecdir = get_option('ragnar-session-libexecdir')
if ragnar_session_libexecdir == ''
    ragnar_session_libexecdir = libexecdirroot
endif
session_data.set('ragnar_session_libexecdir', ragnar_session_libexecdir)

# Set libexecdir to gnome-settings-daemon
# Unfortunately this info is not provided by its pkgconfig file
gsd_libexecdir = get_option('gsd-libexecdir')
if gsd_libexecdir == ''
    gsd_libexecdir = libexecdirroot
endif
session_data.set('gsd_libexecdir', gsd_libexecdir)

# Set the libexecdir for xdg-desktop-portal
xdp_libexecdir = get_option('xdp-libexecdir')
if xdp_libexecdir == ''
    xdp_libexecdir = libexecdirroot
endif
session_data.set('xdp_libexecdir', xdp_libexecdir)

# Start oo7-daemon with the session when oo7 support is enabled
if oo7 == true
    session_data.set('oo7_daemon', '  ' + join_paths(oo7_libexecdir, 'oo7-daemon') + ' &')
else
    session_data.set('oo7_daemon', '')
endif

# magpie is here while its merge is commented out, to keep its translations
ragnar_pot_templates += files(
    'ragnar-desktop.session.desktop.in.in',
    'ragnar-desktop.desktop.in.in',
    'ragnar-desktop-magpie.desktop.in.in',
    'ragnar-desktop-powerdialog.desktop.in.in',
    'ragnar-desktop-screenshotdialog.desktop.in.in',
)

# Write the ragnar-desktop.session.in file from the .in.in
session_conf = configure_file(
    input: 'ragnar-desktop.session.desktop.in.in',
    output: 'ragnar-desktop.session.in',
    configuration: session_data,
)

# Now merge the translations of the .desktop.in to a .desktop
i18n.merge_file(
    input : session_conf,
    output : 'org.lieraos.ragnar.Desktop.session',
    type : 'desktop',
    po_dir : podir,
    install : true,
    install_dir : join_paths(datadir, 'gnome-session', 'sessions'),
)

# Write /usr/bin/startragnarlabwc script
configure_file(
    input: 'startragnarlabwc.in',
    output: 'startragnarlabwc',
    configuration: session_data,
    install_dir: join_paths(get_option('prefix'), get_option('bindir')),
)

# Write /usr/bin/ragnar-desktop script
configure_file(
    input: 'ragnar-desktop.in',
    output: 'ragnar-desktop',
    configuration: session_data,
    install_dir: join_paths(get_option('prefix'), get_option('bindir')),
)

# Write /usr/bin/startragnarodin script — the Ragnar session
# bootstrap on the Odin compositor, selectable via the
# RAGNAR_SESSION_COMPOSITOR hook. Session-layer integration
# lives here; Odin itself stays ragnar-free.
configure_file(
    input: 'startragnarodin.in',
    output: 'startragnarodin',
    configuration: session_data,
    install_dir: join_paths(get_option('prefix'), get_option('bindir')),
)

# Configure the default wayland-session file
waylandsession_conf = configure_file(
    input: 'ragnar-desktop.desktop.in.in',
    output: 'ragnar-desktop.desktop.in',
    configuration: session_data,
)

# Now merge the translations of the .desktop.in to a .desktop
i18n.merge_file(
    input : waylandsession_conf,
    output : 'ragnar-desktop.desktop',
    type : 'desktop',
    po_dir : podir,
    install : true,
    install_dir : join_paths(datadir, 'wayland-sessions'),
)

# Configure the experimental Odin wayland-session file
odin_waylandsession_conf = configure_file(
    input: 'ragnar-odin.desktop.in.in',
    output: 'ragnar-odin.desktop.in',
    configuration: session_data,
)

# Now merge the translations of the .desktop.in to a .desktop
i18n.merge_file(
    input : odin_waylandsession_conf,
    output : 'ragnar-odin.desktop',
    type : 'desktop',
    po_dir : podir,
    install : true,
    install_dir : join_paths(datadir, 'wayland-sessions'),
)

# Configure the magpie wayland-session file
#waylandmagpiesession_conf = configure_file(
#    input: 'ragnar-desktop-magpie.desktop.in.in',
#    output: 'ragnar-desktop-magpie.desktop.in',
#    configuration: session_data,
#)

# Now merge the translations of the .desktop.in to a .desktop
#i18n.merge_file(
#    input : waylandmagpiesession_conf,
#    output : 'ragnar-desktop-magpie.desktop',
#    type : 'desktop',
#    po_dir : podir,
#    install : true,
#    install_dir : join_paths(datadir, 'wayland-sessions'),
#)

# Merge + install power dialog
session_power_dialog_desktop = configure_file(
    input: 'ragnar-desktop-powerdialog.desktop.in.in',
    output: 'ragnar-desktop-powerdialog.desktop.in',
    configuration: session_data,
)

i18n.merge_file(
    input : session_power_dialog_desktop,
    output : 'org.lieraos.ragnar.PowerDialog.desktop',
    type : 'desktop',
    po_dir : podir,
    install : true,
    install_dir : xdg_appdir,
)


# root display access for pkexec-launched apps (gparted & co.) under XWayland
install_data(
    'org.lieraos.ragnar.XhostRoot.desktop',
    install_dir: xdg_appdir,
)


# install gnome-settings-daemon desktop files to ensure they autostart

foreach component: ragnar_session_324_components
    configure_file(
        input: join_paths('settingsdaemon', component + '.desktop.in'),
        output: '@BASENAME@',
        configuration: session_data,
        install_dir: xdg_appdir,
    )
endforeach

install_data(
    'org.lieraos.ragnar.settings-daemon.plugins.media-keys.gschema.xml',
    install_dir: join_paths(datadir, 'glib-2.0', 'schemas'),
)
