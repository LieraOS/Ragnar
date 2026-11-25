# Installation Instructions

## System Dependencies

Before building LieraAudio, you need to install the following system dependencies.

### Arch Linux / Manjaro
```bash
sudo pacman -S pipewire gtk4 base-devel pkg-config
```

### Debian / Ubuntu
```bash
sudo apt install libpipewire-0.3-dev libgtk-4-dev pkg-config build-essential
```

### Fedora
```bash
sudo dnf install pipewire-devel gtk4-devel pkg-config gcc
```

### openSUSE
```bash
sudo zypper install pipewire-devel gtk4-devel pkg-config gcc
```

## Building from Source

1. Clone the repository (or navigate to the project directory)
2. Install system dependencies (see above)
3. Build the project:
```bash
cargo build --release
```

4. Run the application:
```bash
cargo run --release
```

## Development Build

For development builds without optimizations:
```bash
cargo build
cargo run
```

## Troubleshooting

### "Package libpipewire-0.3 was not found"
This means PipeWire development libraries are not installed. See the System Dependencies section above for your distribution.

### GTK4 not found
Install GTK4 development libraries for your distribution (see above).

### pkg-config errors
Make sure pkg-config is installed and can find the required libraries:
```bash
pkg-config --modversion libpipewire-0.3
pkg-config --modversion gtk4
```
