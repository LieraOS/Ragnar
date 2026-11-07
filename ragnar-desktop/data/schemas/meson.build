# GSettings schemas required by live consumers after the
# src/panel and src/wm source trees were removed.

gschema_files = files(
    'org.lieraos.ragnar.panel.gschema.xml',
    'org.lieraos.ragnar.wm.gschema.xml',
)

install_data(
    gschema_files,
    install_dir: join_paths(datadir, 'glib-2.0', 'schemas'),
)
