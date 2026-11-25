# ragnar-livewall — TODO

## Open

- [ ] Field-test the aurora overlay on the production desktop
      (visual tuning: veil density, speed, colour mix)
- [ ] Verify auto-pause triggers on a real fullscreen window
- [ ] Verify aurora performance/look on AMD and Intel GPUs —
      so far only validated on NVIDIA (OpenGL ES 3.2)
- [ ] Extend the effects config: lamp-glow rects with flicker
      modes — same uniform mechanism aurora/stars/mist already use
- [ ] Move the hardcoded aurora-fjord lamp flicker (CSS overlays)
      into the shader as configurable glow spots
- [ ] Pause on lock screen and on battery power
- [ ] Ship the default wallpaper path from the installed
      ragnar-backgrounds package instead of a hardcoded path
- [ ] .deb packaging + live-build integration after approval
- [ ] Autostart entry for the Ragnar session (once enabled by
      default)

## Done

- [x] Background-layer surface per monitor (zwlr_layer_shell)
- [x] GLSL fragment-shader aurora over the wallpaper, 15 fps
      (GtkGLArea, OpenGL ES / desktop GL dual dialect)
- [x] Cabin lamp flicker overlays on the stock artwork
- [x] Fullscreen auto-pause via zwlr_foreign_toplevel_v1
- [x] `--image` argument for testing arbitrary pictures
- [x] Mirrored, dimmed aurora in the water band + luminance mask
      so dark clouds don't show the glow
- [x] `ApplicationFlags::NON_UNIQUE` — daemon-spawned children
      must never single-instance-forward and exit
- [x] Integration: ragnar-wallpaper spawns livewall for all
      monitors while `wallpaper-animated` is `true`
- [x] Configurable effects per image via livewall-effects.ini —
      uniform-driven shader blocks; aurora is a toggleable effect
      with its own region/intensity, stars and mist take up to four
      `;`-separated regions each; snow, rain, fireflies and cloud
      shadows joined in 0.1.2 — same uniform mechanism
- [x] Shader sources as installed files (include_str! == deb
      payload) — shared with the ragnar-settings GL preview
