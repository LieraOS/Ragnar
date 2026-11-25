#version 100
// Animated-wallpaper fragment shader, OpenGL ES 2.0 dialect.
// One pass: wallpaper texture + effect blocks (aurora, stars,
// mist), each driven by region uniforms from
// ~/.config/ragnar-settings/livewall-effects.ini.
// Regions: x, y, w, h as fractions of the image, y from TOP —
// the same space the settings editor drags in.
// Installed to /usr/share/ragnar-livewall/ — ragnar-settings
// loads this exact file for its effects preview.
precision mediump float;
uniform sampler2D u_tex;
uniform float u_time;
uniform vec2 u_res;

uniform vec4 u_aurora_rect;   // 0,0,1,1 = classic full-image band
uniform vec2 u_aurora;        // intensity, enabled
uniform vec4 u_stars_rect[4]; // up to 4 regions
uniform vec3 u_stars;         // density, speed, region count
uniform vec4 u_mist_rect[4];
uniform vec3 u_mist;          // opacity, drift, region count
uniform vec4 u_snow_rect[4];
uniform vec3 u_snow;          // density, fall speed, region count
uniform vec4 u_rain_rect[4];
uniform vec3 u_rain;          // strength, speed, region count
uniform vec4 u_fly_rect[4];
uniform vec3 u_fly;           // density, drift speed, region count
uniform vec4 u_cloud_rect[4];
uniform vec3 u_cloud;         // opacity, speed, region count
uniform float u_edit;         // 0 none, 1 glow, 2 stars,
                            // 3 mist, 4 snow, 5 rain, 6 fireflies,
                            // 7 cloud shadows

varying vec2 v_uv;

float hash(vec2 p) {
    return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453123);
}
float vnoise(vec2 p) {
    vec2 i = floor(p), f = fract(p);
    f = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash(i),               hash(i + vec2(1, 0)), f.x),
               mix(hash(i + vec2(0, 1)), hash(i + vec2(1, 1)), f.x), f.y);
}
float fbm(vec2 p) {
    float v = 0.0, a = 0.5;
    for (int i = 0; i < 5; i++) {
        v += a * vnoise(p);
        p = p * 2.03 + vec2(17.3, 9.1);
        a *= 0.55;
    }
    return v;
}

// Two drifting curtains in aurora space: p.y = 1 at the top of
// the aurora region. Returns additive glow.
vec3 aurora(vec2 p, float t) {
    vec3 glow = vec3(0.0);
    for (int k = 0; k < 2; k++) {
        float fk = float(k);
        float cx = p.x * (2.2 + fk)
                 + fbm(vec2(p.x * 3.0 + t * 0.05 + fk * 11.0, t * 0.03)) * 0.7;
        float band = 0.62 + fk * 0.16
                   + 0.10 * sin(cx * 2.4) + 0.05 * sin(cx * 5.7 + 1.7);
        float dy = p.y - band;
        float rays = fbm(vec2(cx * 16.0, dy * 1.4 - t * 0.3 + fk * 7.0));
        rays = pow(rays, 2.0);
        float shape = 1.0 - smoothstep(-0.02, 0.30, abs(dy - 0.05));
        vec3 col = mix(vec3(0.10, 0.85, 0.55), vec3(0.45, 0.15, 0.85),
                       clamp((dy + 0.10) * 4.0, 0.0, 1.0));
        glow += col * rays * shape * (0.55 - fk * 0.2);
    }
    return glow;
}

// Twinkling stars inside one region.
float stars_in(vec2 uv, vec4 rect, float t) {
    vec2 r = (uv - rect.xy) / rect.zw;
    if (r.x < 0.0 || r.y < 0.0 || r.x > 1.0 || r.y > 1.0) return 0.0;
    vec2 grid = r * vec2(48.0, 24.0);
    vec2 cell = floor(grid), f = fract(grid);
    float h = hash(cell + 17.0);
    if (h > u_stars.x) return 0.0;
    vec2 sp = vec2(hash(cell + 7.7), hash(cell + 3.3)) * 0.7 + 0.15;
    float d = length(f - sp);
    float tw = 0.5 + 0.5 * sin(t * u_stars.y * (1.0 + h * 3.0) + h * 40.0);
    float star = smoothstep(0.10, 0.02, d) * (0.25 + 0.75 * tw * tw);
    float edge = smoothstep(0.0, 0.06, r.x) * smoothstep(1.0, 0.94, r.x)
               * smoothstep(0.0, 0.06, r.y) * smoothstep(1.0, 0.94, r.y);
    return star * edge;
}

// Slow-drifting fog inside one region.
float mist_in(vec2 uv, vec4 rect, float t) {
    vec2 r = (uv - rect.xy) / rect.zw;
    if (r.x < 0.0 || r.y < 0.0 || r.x > 1.0 || r.y > 1.0) return 0.0;
    float n = fbm(vec2(r.x * 2.5 + t * 0.04 * u_mist.y,
                       r.y * 1.6 + t * 0.008 * u_mist.y));
    float body = smoothstep(0.38, 0.78, n);
    float edge = smoothstep(0.0, 0.18, r.y) * smoothstep(1.0, 0.82, r.y)
               * smoothstep(0.0, 0.10, r.x) * smoothstep(1.0, 0.90, r.x);
    return body * edge;
}


// Falling snowflakes inside one region — r.y runs top->down.
float snow_in(vec2 uv, vec4 rect, float t) {
    vec2 r = (uv - rect.xy) / rect.zw;
    if (r.x < 0.0 || r.y < 0.0 || r.x > 1.0 || r.y > 1.0) return 0.0;
    float n = 0.0;
    for (int i = 0; i < 48; i++) {
        float fi = float(i);
        if (fi >= u_snow.x * 48.0) break;
        float h1 = hash(vec2(fi, 7.13));
        float h2 = hash(vec2(fi, 3.71));
        float h3 = hash(vec2(fi, 11.9));
        float y = fract(h2 + t * u_snow.y * (0.03 + 0.05 * h3));
        float x = fract(h1 + 0.04 * sin(t * (0.4 + h3) + fi * 1.7));
        float sz = 0.004 + 0.008 * h3;
        n += smoothstep(sz, sz * 0.35, length(r - vec2(x, y)));
    }
    float edge = smoothstep(0.0, 0.05, r.x) * smoothstep(1.0, 0.95, r.x)
               * smoothstep(0.0, 0.05, r.y) * smoothstep(1.0, 0.95, r.y);
    return n * edge;
}

// Slanted rain streaks inside one region.
float rain_in(vec2 uv, vec4 rect, float t) {
    vec2 r = (uv - rect.xy) / rect.zw;
    if (r.x < 0.0 || r.y < 0.0 || r.x > 1.0 || r.y > 1.0) return 0.0;
    float n = 0.0;
    float slant = 0.07;
    float cols = 60.0;
    for (int i = 0; i < 3; i++) {
        float fi = float(i);
        float ci = floor((r.x + r.y * slant) * cols);
        float h = hash(vec2(ci, fi * 17.3 + 4.0));
        float y = fract(h + t * u_rain.y * (0.55 + 0.45 * h) + fi * 0.33);
        float dx = abs(r.x + r.y * slant - (ci + 0.5) / cols);
        float seg = step(r.y - y, 0.0) * step(-0.07, r.y - y);
        n += smoothstep(0.0022, 0.0008, dx) * seg;
    }
    return n * (0.20 + 0.80 * u_rain.x);
}

// Wandering, pulsing fireflies inside one region.
float fly_in(vec2 uv, vec4 rect, float t) {
    vec2 r = (uv - rect.xy) / rect.zw;
    if (r.x < 0.0 || r.y < 0.0 || r.x > 1.0 || r.y > 1.0) return 0.0;
    float n = 0.0;
    for (int i = 0; i < 12; i++) {
        float fi = float(i);
        if (fi >= u_fly.x * 12.0) break;
        float h1 = hash(vec2(fi, 5.3));
        float h2 = hash(vec2(fi, 9.1));
        float w = t * u_fly.y * 0.12;
        vec2 c = vec2(
            fract(h1 + w * (0.4 + 0.6 * h2) + 0.10 * sin(w * 1.3 + fi)),
            fract(h2 + w * (0.3 + 0.5 * h1) + 0.10 * cos(w * 1.1 + fi * 2.0)));
        float pulse = 0.35 + 0.65 * pow(0.5 + 0.5 * sin(t * (0.8 + h1) + h2 * 40.0), 3.0);
        n += pulse * smoothstep(0.045, 0.008, length(r - c));
    }
    return n;
}

// Large soft cloud shadows drifting across one region.
float cloud_in(vec2 uv, vec4 rect, float t) {
    vec2 r = (uv - rect.xy) / rect.zw;
    if (r.x < 0.0 || r.y < 0.0 || r.x > 1.0 || r.y > 1.0) return 0.0;
    float n = fbm(vec2(r.x * 1.8 - t * 0.05 * u_cloud.y,
                       r.y * 2.4 + t * 0.008 * u_cloud.y));
    float body = smoothstep(0.52, 0.80, n);
    float edge = smoothstep(0.0, 0.20, r.y) * smoothstep(1.0, 0.80, r.y)
               * smoothstep(0.0, 0.15, r.x) * smoothstep(1.0, 0.85, r.x);
    return body * edge;
}

// Marching-ants border around a region being edited.
float rect_border(vec2 uv, vec4 r, float t) {
    vec2 px = uv * u_res;
    vec2 r0 = r.xy * u_res, r1 = (r.xy + r.zw) * u_res;
    float inside = step(r0.x, px.x) * step(px.x, r1.x)
                 * step(r0.y, px.y) * step(px.y, r1.y);
    float bd = min(min(px.x - r0.x, r1.x - px.x),
                   min(px.y - r0.y, r1.y - px.y));
    float dash = step(0.5, fract((px.x + px.y) * 0.02 - t * 0.7));
    return inside * (1.0 - smoothstep(1.5, 3.5, bd)) * dash;
}

void main() {
    vec2 uv = vec2(v_uv.x, 1.0 - v_uv.y); // uv.y = 1 is bottom
    vec3 base = texture2D(u_tex, uv).rgb;
    float t = u_time;
    float lum = dot(base, vec3(0.30, 0.55, 0.15));
    vec3 col = base;
    vec3 hl = vec3(0.35, 0.75, 1.0);

    // Aurora: p.y = 1 at the region top; mirrored across the
    // band's lower edge (~38 % down the region) for the water.
    if (u_aurora.y > 0.5) {
        vec4 R = u_aurora_rect;
        float ap  = 1.0 - (uv.y - R.y) / R.w;
        float apm = 0.26 + (uv.y - R.y) / R.w;
        float xm  = smoothstep(R.x - 0.02, R.x + 0.02, uv.x)
                  * smoothstep(R.x + R.z + 0.02, R.x + R.z - 0.02, uv.x);
        float we  = R.y + 0.38 * R.w;
        vec3 glow = aurora(vec2(uv.x, ap), t)
                  * smoothstep(0.30, 0.55, ap)
                  * (0.35 + 0.65 * smoothstep(0.05, 0.20, lum));
        vec3 refl = aurora(vec2(uv.x, apm), t)
                  * smoothstep(we, we + 0.04 * R.w, uv.y)
                  * (1.0 - smoothstep(we + 0.10 * R.w, we + 0.45 * R.w, uv.y))
                  * 0.42;
        col += (glow + refl) * u_aurora.x * xm;
    }
    // Edit highlight lives outside the enable gates — the user
    // draws regions while the effect may still be off.
    if (u_edit == 1.0 && u_aurora_rect.z > 0.0)
        col = mix(col, hl, rect_border(uv, u_aurora_rect, t) * 0.8);

    for (int i = 0; i < 4; i++) {
        if (float(i) < u_stars.z)
            col += vec3(0.85, 0.90, 1.0)
                 * stars_in(uv, u_stars_rect[i], t)
                 * (0.4 + 0.6 * smoothstep(0.03, 0.15, lum));
        if (u_edit == 2.0 && u_stars_rect[i].z > 0.0)
            col = mix(col, hl, rect_border(uv, u_stars_rect[i], t) * 0.8);
    }

    for (int i = 0; i < 4; i++) {
        if (float(i) < u_mist.z) {
            float m = mist_in(uv, u_mist_rect[i], t) * u_mist.x;
            col = mix(col, vec3(0.72, 0.76, 0.82), m);
        }
        if (u_edit == 3.0 && u_mist_rect[i].z > 0.0)
            col = mix(col, hl, rect_border(uv, u_mist_rect[i], t) * 0.8);
    }

    for (int i = 0; i < 4; i++) {
        if (float(i) < u_snow.z)
            col += vec3(0.92, 0.94, 1.0) * snow_in(uv, u_snow_rect[i], t);
        if (u_edit == 4.0 && u_snow_rect[i].z > 0.0)
            col = mix(col, hl, rect_border(uv, u_snow_rect[i], t) * 0.8);
    }

    for (int i = 0; i < 4; i++) {
        if (float(i) < u_rain.z)
            col += vec3(0.75, 0.82, 0.95) * rain_in(uv, u_rain_rect[i], t);
        if (u_edit == 5.0 && u_rain_rect[i].z > 0.0)
            col = mix(col, hl, rect_border(uv, u_rain_rect[i], t) * 0.8);
    }

    for (int i = 0; i < 4; i++) {
        if (float(i) < u_fly.z)
            col += vec3(1.0, 0.85, 0.45) * fly_in(uv, u_fly_rect[i], t);
        if (u_edit == 6.0 && u_fly_rect[i].z > 0.0)
            col = mix(col, hl, rect_border(uv, u_fly_rect[i], t) * 0.8);
    }

    for (int i = 0; i < 4; i++) {
        if (float(i) < u_cloud.z)
            col = mix(col, col * 0.55,
                      cloud_in(uv, u_cloud_rect[i], t) * u_cloud.x);
        if (u_edit == 7.0 && u_cloud_rect[i].z > 0.0)
            col = mix(col, hl, rect_border(uv, u_cloud_rect[i], t) * 0.8);
    }

    gl_FragColor = vec4(col, 1.0);
}
