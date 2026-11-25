# TODO — ragnar-eye

[Deutsche Version](TODO_de.md)

## Open

- Port the remaining exotic decoders: MSP, VICAR, VIFF,
  ILBM, MIFF, MNG, XWD, WBMP, PCX, SGI RGB, JP2/J2K
  (openjpeg). Re-add their extensions to
  `supported_extensions()` once a decoder actually works.
- Animated GIF/APNG: currently first frame only; add frame
  stepping.
- Verify the DjVu/FITS/JXL/libraw/HEIF paths against real
  files — no fixtures on this machine, so these formats are
  untested.
- Scaled decoding for thumbnails (e.g. JPEG DCT scaling)
  instead of full-resolution decode plus downscale.
- Zoom-to-cursor anchoring — the wheel currently zooms around
  the centre.
- EXIF metadata in the properties dialog (camera, date
  taken).
- Single-instance behaviour: a second launch should hand the
  file to the running instance instead of opening a parallel
  process.
- Multi-page TIFF/DjVu navigation.
- Slideshow and image comparison views — decide whether they
  carry over.
- Package placement: decide whether ragnar-eye lands in the
  core image or as a recommended standalone package.
