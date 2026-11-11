// Macaron: a purple macaron as a signed distance field, ray-marched in the
// fragment shader. Two domed shells, their ruffled feet (the "pied") and the
// cream between them; it leans a little and turns with the frame clock.
struct Uniforms {
    // The turn about the vertical axis, radians (wrapped in Rust).
    spin: f32,
    // The phase of the slow bob, radians (wrapped in Rust).
    bob: f32,
    width: f32,
    height: f32,
}

@group(0) @binding(0) var<uniform> u: Uniforms;

struct Fragment {
    @builtin(position) position: vec4<f32>,
}

@vertex
fn vs(@builtin(vertex_index) i: u32) -> Fragment {
    // A triangle that covers the clip square: (-1,-1), (3,-1), (-1,3).
    var out: Fragment;
    let x = f32(i32(i & 1u) * 4 - 1);
    let y = f32(i32(i >> 1u) * 4 - 1);
    out.position = vec4<f32>(x, y, 0.0, 1.0);
    return out;
}

fn hash3(p: vec3<f32>) -> f32 {
    return fract(sin(dot(p, vec3<f32>(127.1, 311.7, 74.7))) * 43758.5453123);
}

fn noise3(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let s = f * f * (3.0 - 2.0 * f);
    let a = mix(hash3(i), hash3(i + vec3<f32>(1.0, 0.0, 0.0)), s.x);
    let b = mix(hash3(i + vec3<f32>(0.0, 1.0, 0.0)), hash3(i + vec3<f32>(1.0, 1.0, 0.0)), s.x);
    let c = mix(hash3(i + vec3<f32>(0.0, 0.0, 1.0)), hash3(i + vec3<f32>(1.0, 0.0, 1.0)), s.x);
    let d = mix(hash3(i + vec3<f32>(0.0, 1.0, 1.0)), hash3(i + vec3<f32>(1.0, 1.0, 1.0)), s.x);
    return mix(mix(a, b, s.y), mix(c, d, s.y), s.z);
}

fn smin(a: f32, b: f32, k: f32) -> f32 {
    let h = clamp(0.5 + 0.5 * (b - a) / k, 0.0, 1.0);
    return mix(b, a, h) - k * h * (1.0 - h);
}

// A cylinder about the y axis with rounded edges: radius `ra`, half height `h`, rounding `r`.
fn round_cylinder(p: vec3<f32>, ra: f32, h: f32, r: f32) -> f32 {
    let d = vec2<f32>(length(p.xz) - (ra - r), abs(p.y) - (h - r));
    return min(max(d.x, d.y), 0.0) + length(max(d, vec2<f32>(0.0, 0.0))) - r;
}

fn rot_x(p: vec3<f32>, a: f32) -> vec3<f32> {
    let c = cos(a);
    let s = sin(a);
    return vec3<f32>(p.x, c * p.y - s * p.z, s * p.y + c * p.z);
}

fn rot_y(p: vec3<f32>, a: f32) -> vec3<f32> {
    let c = cos(a);
    let s = sin(a);
    return vec3<f32>(c * p.x + s * p.z, p.y, -s * p.x + c * p.z);
}

// World space to the macaron's own: it bobs, turns about the vertical, and leans.
fn to_macaron(p: vec3<f32>) -> vec3<f32> {
    var q = p - vec3<f32>(0.0, 0.05 * sin(u.bob), 0.0);
    q = rot_y(q, -u.spin);
    q = rot_x(q, -0.42);
    q = rot_y(q, -u.spin * 0.5);
    return q;
}

// One shell with its foot, the dome towards +y; `p` is relative to the macaron's centre.
fn shell(p: vec3<f32>) -> f32 {
    let c = p - vec3<f32>(0.0, 0.32, 0.0);
    let body = round_cylinder(c, 1.0, 0.17, 0.16);
    // A flattened sphere lifts the middle into a dome.
    let radii = vec3<f32>(0.93, 0.30, 0.93);
    let e = c - vec3<f32>(0.0, 0.03, 0.0);
    let dome = (length(e / radii) - 1.0) * 0.30;
    var d = smin(body, dome, 0.08);
    // The foot: a ruffled ring under the shell's rim.
    let f = p - vec3<f32>(0.0, 0.19, 0.0);
    let ring = length(vec2<f32>(length(f.xz) - 0.915, f.y)) - 0.055;
    let ruffle = (noise3(p * 17.0) - 0.5) * 0.05 + (noise3(p * 41.0) - 0.5) * 0.018;
    d = smin(d, ring + ruffle, 0.03);
    return d;
}

// The distance to the macaron and what was hit: 0 the shells, 1 the cream.
fn map(p: vec3<f32>) -> vec2<f32> {
    let q = to_macaron(p);
    let top = shell(q);
    let bottom = shell(vec3<f32>(q.x, -q.y, q.z));
    let shells = min(top, bottom);
    let bulge = 0.012 * sin(atan2(q.z, q.x) * 9.0);
    let cream = round_cylinder(q, 0.935 + bulge, 0.15, 0.10);
    if (cream < shells) {
        return vec2<f32>(cream, 1.0);
    }
    return vec2<f32>(shells, 0.0);
}

fn normal_at(p: vec3<f32>) -> vec3<f32> {
    let e = vec2<f32>(0.0015, 0.0);
    return normalize(vec3<f32>(
        map(p + e.xyy).x - map(p - e.xyy).x,
        map(p + e.yxy).x - map(p - e.yxy).x,
        map(p + e.yyx).x - map(p - e.yyx).x,
    ));
}

fn occlusion(p: vec3<f32>, n: vec3<f32>) -> f32 {
    var o = 0.0;
    var w = 1.0;
    for (var i = 1; i <= 4; i = i + 1) {
        let h = 0.04 * f32(i);
        o = o + (h - map(p + n * h).x) * w;
        w = w * 0.6;
    }
    return clamp(1.0 - 1.6 * o, 0.0, 1.0);
}

fn background(uv: vec2<f32>) -> vec3<f32> {
    let top = vec3<f32>(0.985, 0.960, 1.000);
    let low = vec3<f32>(0.905, 0.850, 0.975);
    var col = mix(low, top, clamp(uv.y * 0.6 + 0.55, 0.0, 1.0));
    // The macaron's shadow on the table under it.
    let s = (uv - vec2<f32>(0.02, -0.29)) / vec2<f32>(0.32, 0.045);
    let shade = exp(-dot(s, s) * 1.6);
    col = mix(col, vec3<f32>(0.52, 0.40, 0.68), 0.38 * shade);
    return col;
}

@fragment
fn fs(f: Fragment) -> @location(0) vec4<f32> {
    let res = vec2<f32>(u.width, u.height);
    // Centred, y up, one unit the shorter side.
    var uv = (f.position.xy - 0.5 * res) / min(res.x, res.y);
    uv.y = -uv.y;

    let eye = vec3<f32>(0.0, 1.05, 3.9);
    let forward = normalize(vec3<f32>(0.0, 0.02, 0.0) - eye);
    let right = normalize(cross(forward, vec3<f32>(0.0, 1.0, 0.0)));
    let up = cross(right, forward);
    let ray = normalize(forward * 1.45 + right * uv.x + up * uv.y);

    var t = 2.0;
    var hit = vec2<f32>(1.0, 0.0);
    var found = false;
    for (var i = 0; i < 72; i = i + 1) {
        hit = map(eye + ray * t);
        if (hit.x < 0.0012) {
            found = true;
            break;
        }
        t = t + hit.x * 0.85;
        if (t > 6.0) {
            break;
        }
    }

    var col = background(uv);
    if (found) {
        let p = eye + ray * t;
        let n = normal_at(p);
        let light = normalize(vec3<f32>(0.45, 0.85, 0.55));
        let cream = hit.y > 0.5;
        var albedo = vec3<f32>(0.640, 0.400, 0.880);
        if (cream) {
            albedo = vec3<f32>(0.950, 0.890, 0.985);
        } else {
            // A matte, slightly uneven bake.
            let q = to_macaron(p);
            albedo = albedo * (0.94 + 0.10 * noise3(q * 9.0));
        }
        let wrap = clamp(dot(n, light) * 0.6 + 0.4, 0.0, 1.0);
        let ao = occlusion(p, n);
        let sky = 0.5 + 0.5 * n.y;
        let ambient = mix(vec3<f32>(0.26, 0.20, 0.34), vec3<f32>(0.48, 0.45, 0.56), sky);
        let half_way = normalize(light - ray);
        var gloss = pow(clamp(dot(n, half_way), 0.0, 1.0), 28.0) * 0.16;
        if (cream) {
            gloss = gloss * 1.8;
        }
        let rim = pow(1.0 - clamp(dot(n, -ray), 0.0, 1.0), 3.0) * 0.12;
        col = albedo * (ambient * ao + vec3<f32>(0.80, 0.76, 0.72) * wrap * wrap * (0.5 + 0.5 * ao));
        col = col + vec3<f32>(gloss, gloss, gloss) + vec3<f32>(0.80, 0.70, 1.00) * rim * ao;
    }
    return vec4<f32>(clamp(col, vec3<f32>(0.0, 0.0, 0.0), vec3<f32>(1.0, 1.0, 1.0)), 1.0);
}
