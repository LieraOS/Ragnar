project(
    'ragnar-desktop',
    version: '1.0',
    license: [
        'GPL-2.0',
        'LGPL-2.1',
    ],
)

# ragnar-desktop is the session-glue package: shell/python launchers,
# the labwc bridge, schemas, icons and data. All desktop components
# are separate Rust crates — there is no compiled code left here.

fs = import('fs')
i18n = import('i18n')

package_version = meson.project_version()

# Paths consumed by the subdir meson.build files
prefix = get_option('prefix')
bindir = join_paths(prefix, get_option('bindir'))
libdir = join_paths(prefix, get_option('libdir'))
libexecdirroot = join_paths(prefix, get_option('libexecdir'))
libexecdir = join_paths(prefix, get_option('libexecdir'), meson.project_name())
confdir = join_paths(prefix, get_option('sysconfdir'))
datadir = join_paths(prefix, get_option('datadir'))
localedir = join_paths(prefix, get_option('localedir'))
podir = join_paths(meson.project_source_root(), 'po')

# Autostart entries always land in /etc/xdg/autostart
xdg_appdir = join_paths(get_option('sysconfdir'), 'xdg', 'autostart')

# oo7 was the Legacy Secret Service backend — replaced by
# gnome-keyring under ragnar-keyring. Consumed by src/session.
oo7 = false
with_polkit = true
with_gtk_doc = false

# src/ appends to these, po/ reads them, so subdir('po') must follow subdir('src')
ragnar_pot_targets = []
ragnar_pot_templates = []

# No .ui files may remain — files() rejects empty input, so only
# build the list when the script produced real names.
ragnar_pot_ui_stdout = run_command(
    find_program('po/listUiFiles.py'),
    meson.project_source_root(),
    check: true,
).stdout().strip()
ragnar_pot_ui = []
if ragnar_pot_ui_stdout != ''
    ragnar_pot_ui = files(ragnar_pot_ui_stdout.split('\n'))
endif

subdir('src')
subdir('po')
subdir('docs')
subdir('data')

report = [
    '    Build configuration:',
    '    ====================',
    '',
    '    prefix:                                 @0@'.format(prefix),
    '    datadir:                                @0@'.format(datadir),
    '    sysconfdir:                             @0@'.format(confdir),
    '    localedir:                              @0@'.format(localedir),
    '    libdir:                                 @0@'.format(libdir),
    '    xdg config directory:                   @0@'.format(xdg_appdir),
]

message('\n\n\n' + '\n'.join(report) + '\n\n')
