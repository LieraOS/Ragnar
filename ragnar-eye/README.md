# Ragnar Eye

Fast GTK4 image viewer for the Ragnar desktop with a classic
three-pane layout: directory tree on the left, the viewed
image in the center, directory thumbnails on the right — built
for speed and broad format coverage.

[Deutsche Version](README_de.md)

## Features

- **Three-pane layout** — filesystem tree on the left, the viewed
  image in the center, thumbnails of the current directory on the
  right. Clicking a thumbnail shows the image; selecting a
  directory repopulates the strip.
- **Free zoom** — wheel zoom clamped at 25600%, drag pan, fit
  modes (F / 0 = fit window, 1 = 100%), nearest-neighbour above
  400% so pixels stay crisp.
- **Fast navigation** — Left/Right/PgUp/PgDn/Space walk the
  directory; neighbours are pre-decoded in the background so
  flipping feels instant. Async decoding keeps the UI responsive
  on 60 MB+ files.
- **Formats** — PNG, JPEG, GIF, WebP, TIFF, BMP, TGA, ICO/CUR,
  PNM, QOI, DDS, EXR, HDR, farbfeld, APNG via the `image` crate;
  PSD/PSB via `psd`; KRA/ORA as ZIP containers; camera RAW
  (CR2/CR3/NEF/ARW/DNG and more) via libraw; HEIC/HEIF/AVIF via
  libheif; JPEG XL via libjxl; DjVu via libdjvulibre; FITS via
  cfitsio; EPS/PS/AI via ghostscript; plus ported decoders for
  PFM/PHM, Sun RAS, MTV, XBM, XPM.
- **Light filters** — grayscale, sepia, invert, auto-enhance
  (histogram stretch), red-eye reduction; "Original" restores the
  untouched decode. Viewer, not an editor.
- **Thumbnail disk cache** — keyed on path+mtime+size under
  `~/.cache/ragnar-eye/thumbs/`.
- Optional codec libraries are loaded at runtime (`dlopen`), so a
  missing library disables only that format — the viewer still
  builds and runs.

## Usage

```
ragnar-eye image.cr2        # open a file
ragnar-eye                  # open current directory
```

Shortcuts: `←/→` or `PgUp/PgDn` navigate, `Space` next, `+/-`
zoom, `0`/`F` fit window, `1` actual size.

## Build

```
cargo build --release
```

Runtime codec libraries (optional, per-format): `libraw25`,
`libheif1`, `libjxl0.11`, `libavif16`, `libdjvulibre21`,
`libcfitsio10`, `ghostscript`.

## License

GPL-3.0-or-later — © Jörg Schröder, LieraOS Project
