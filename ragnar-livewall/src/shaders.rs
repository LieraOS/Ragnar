// GLSL shader sources for the wallpaper pass, kept in both
// dialects: desktop GL (#version 130, fullscreen triangle via
// gl_VertexID) and OpenGL ES 2.0 (#version 100, VBO quad).
// Selected at runtime from the GL version string — see glwall.rs.
//
// The fragment sources live in ../glsl/ — the exact files the
// .deb installs to /usr/share/ragnar-livewall/. ragnar-settings
// loads them for its effects preview, so the dialog and the
// desktop always render identically.

pub const VS_GL: &str = "#version 130
out vec2 v_uv;
void main() {
    vec2 p = vec2(float((gl_VertexID << 1) & 2), float(gl_VertexID & 2));
    v_uv = p;
    gl_Position = vec4(p * 2.0 - 1.0, 0.0, 1.0);
}";

/// GLES2 vertex shader — fullscreen quad from a small VBO.
pub const VS_ES: &str = "#version 100
attribute vec2 a_pos;
varying vec2 v_uv;
void main() {
    v_uv = a_pos * 0.5 + 0.5;
    gl_Position = vec4(a_pos, 0.0, 1.0);
}";

/// Desktop fragment shader — wallpaper + aurora + effect blocks.
pub const FS_GL: &str = include_str!("../glsl/effects_gl.glsl");

/// GLES2 twin of FS_GL — same effect math, ES shading dialect.
pub const FS_ES: &str = include_str!("../glsl/effects_es.glsl");
