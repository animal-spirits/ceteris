// Fills coloured per feature from a texture (ADR 0010); feature ids go to a second target for
// picking. Flat: instance k draws the world shifted by (first + k) in x, so the map wraps.
// Globe: an orthographic sphere seen from above the centre; the far side is discarded.

struct View {
    centre: vec2f,
    k: vec2f,
    first: f32,
    globe: u32,
    sea: vec4f,
    paper: vec4f,
}

@group(0) @binding(0) var<uniform> view: View;
@group(0) @binding(1) var colours: texture_2d<f32>;

const TEX_WIDTH: u32 = 1024u;

const PI: f32 = 3.14159265358979;

struct VsOut {
    @builtin(position) pos: vec4f,
    @location(0) colour: vec4f,
    @location(1) @interpolate(flat) id: u32,
    // Distance towards the viewer on the globe; negative on the far side.
    @location(2) facing: f32,
}

fn lon_lat(m: vec2f) -> vec2f {
    let y = PI * (1.0 - 2.0 * m.y);
    return vec2f((m.x - 0.5) * 2.0 * PI, atan(0.5 * (exp(y) - exp(-y))));
}

@vertex
fn vs(@location(0) xy: vec2f, @location(1) feature: u32, @builtin(instance_index) copy: u32) -> VsOut {
    var out: VsOut;
    if view.globe == 0u {
        let d = xy + vec2f(view.first + f32(copy), 0.0) - view.centre;
        out.pos = vec4f(d * view.k, 0.0, 1.0);
        out.facing = 1.0;
    } else {
        let p = lon_lat(xy);
        let c = lon_lat(view.centre);
        let dl = p.x - c.x;
        let x = cos(p.y) * sin(dl);
        let y = sin(p.y);
        let z = cos(p.y) * cos(dl);
        let yr = y * cos(c.y) - z * sin(c.y);
        let zr = y * sin(c.y) + z * cos(c.y);
        out.pos = vec4f(vec2f(x, yr) * view.k, 0.0, 1.0);
        out.facing = zr;
    }
    out.colour = textureLoad(colours, vec2u(feature % TEX_WIDTH, feature / TEX_WIDTH), 0);
    out.id = feature + 1u;
    return out;
}

struct FsOut {
    @location(0) colour: vec4f,
    @location(1) id: u32,
}

@fragment
fn fs(in: VsOut) -> FsOut {
    if in.facing < 0.0 {
        discard;
    }
    return FsOut(in.colour, in.id);
}

// Background: on the globe, sea inside the disc and paper outside; flat, all sea.

struct BgOut {
    @builtin(position) pos: vec4f,
    @location(0) ndc: vec2f,
}

@vertex
fn vs_bg(@builtin(vertex_index) i: u32) -> BgOut {
    let ndc = vec2f(f32(i & 1u) * 4.0 - 1.0, f32(i >> 1u) * 4.0 - 1.0);
    return BgOut(vec4f(ndc, 0.0, 1.0), ndc);
}

@fragment
fn fs_bg(in: BgOut) -> FsOut {
    let r = in.ndc / view.k;
    let outside = view.globe == 1u && dot(r, r) > 1.0;
    return FsOut(select(view.sea, view.paper, outside), 0u);
}
