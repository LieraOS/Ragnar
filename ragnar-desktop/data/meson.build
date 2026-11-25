
# Live GSettings schemas previously owned by the removed src/panel
# and src/wm trees. The labwc bridge, ragnar-view, ragnar-display-svc
# and ragnar-settings still read/write these keys, so the schemas
# must ship with ragnar-core.
subdir('schemas')
subdir('backgrounds')
subdir('icons')

# Leveraged in ragnar-control-center
version_data = configuration_data()
version_data.set('version', meson.project_version())

version_file = configure_file(
    input: 'ragnar-version.xml.in',
    output: 'ragnar-version.xml',
    configuration: version_data
)

install_data(
    version_file,
    install_dir: join_paths(datadir, 'ragnar')
)

# The Secret portal is only routed to oo7 when oo7 support is enabled
portals_data = configuration_data()

if oo7 == true
    portals_data.set('secret_portal', 'org.freedesktop.impl.portal.Secret=oo7-portal')
else
    portals_data.set('secret_portal', '')
endif

configure_file(
    input: 'ragnar-portals.conf.in',
    output: 'ragnar-portals.conf',
    configuration: portals_data,
    install_dir: join_paths(datadir, 'xdg-desktop-portal')
)

# gammastep fallback config consumed by ragnar-nightlight
# (previously installed from src/daemon).
install_data('gammastep.config',
    install_dir: join_paths(datadir, 'ragnar-desktop')
)
