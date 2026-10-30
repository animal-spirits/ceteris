// Fills coloured per feature from a texture (ADR 0010); feature ids go to a second target for
// picking and borders. Flat: instance k draws the world shifted by (first + k) in x, so the map
// wraps. Globe: an orthographic sphere seen from above the centre; the far side is discarded.

struct View {
    centre: vec2f,
    k: vec2f,
    first: f32,
    globe: u32,
    mix: f32,
    sea: vec4f,
    paper: vec4f,
    ink: vec4f,
    county_borders: u32,
    // Province ids (index + 1; 0 for none).
    selected: u32,
    origin: u32,
    hover: u32,
}

@group(0) @binding(0) var<uniform> view: View;
@group(0) @binding(1) var colours: texture_2d<f32>;
@group(0) @binding(2) var next_colours: texture_2d<f32>;

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
    let texel = vec2u(feature % TEX_WIDTH, feature / TEX_WIDTH);
    out.colour = mix(textureLoad(colours, texel, 0), textureLoad(next_colours, texel, 0), view.mix);
    out.id = feature + 1u;
    return out;
}

struct FsOut {
    @location(0) colour: vec4f,
    // Province ids in x, county ids in y; each pipeline's write mask keeps its own channel.
    @location(1) id: vec2u,
}

@fragment
fn fs(in: VsOut) -> FsOut {
    if in.facing < 0.0 {
        discard;
    }
    return FsOut(in.colour, vec2u(in.id));
}

// Full-screen passes: a triangle that covers the viewport.

struct ScreenOut {
    @builtin(position) pos: vec4f,
    @location(0) ndc: vec2f,
}

@vertex
fn vs_screen(@builtin(vertex_index) i: u32) -> ScreenOut {
    let ndc = vec2f(f32(i & 1u) * 4.0 - 1.0, f32(i >> 1u) * 4.0 - 1.0);
    return ScreenOut(vec4f(ndc, 0.0, 1.0), ndc);
}

// Background: on the globe, sea inside the disc and paper outside; flat, all sea.
@fragment
fn fs_bg(in: ScreenOut) -> FsOut {
    let r = in.ndc / view.k;
    let outside = view.globe == 1u && dot(r, r) > 1.0;
    return FsOut(select(view.sea, view.paper, outside), vec2u(0u));
}

// Borders from the id target: where neighbouring pixels differ in country (both sides, so two
// pixels wide), province or county (one side, one pixel). Sea is country 0. The selected and
// origin provinces get a two-pixel outline inside their edge; the hovered one is lightened.

@group(1) @binding(0) var ids: texture_2d<u32>;
@group(1) @binding(1) var countries: texture_2d<u32>;

fn country(province: u32) -> u32 {
    if province == 0u {
        return 0u;
    }
    let i = province - 1u;
    return textureLoad(countries, vec2u(i % TEX_WIDTH, i / TEX_WIDTH), 0).r + 1u;
}

fn id_at(p: vec2i) -> vec2u {
    let size = vec2i(textureDimensions(ids));
    return textureLoad(ids, clamp(p, vec2i(0), size - 1), 0).xy;
}

@fragment
fn fs_borders(in: ScreenOut) -> @location(0) vec4f {
    let p = vec2i(in.pos.xy);
    let here = id_at(p);
    let own = country(here.x);
    let offsets = array(vec2i(1, 0), vec2i(0, 1), vec2i(-1, 0), vec2i(0, -1));
    if here.x != 0u && (here.x == view.selected || here.x == view.origin) {
        for (var k = 0; k < 8; k++) {
            if id_at(p + offsets[k % 4] * (1 + k / 4)).x != here.x {
                return select(view.ink, view.paper, here.x == view.origin);
            }
        }
    }
    for (var k = 0; k < 4; k++) {
        if country(id_at(p + offsets[k]).x) != own {
            return vec4f(view.ink.rgb, 0.85);
        }
    }
    for (var k = 0; k < 2; k++) {
        let there = id_at(p + offsets[k]);
        if there.x != here.x {
            return vec4f(view.ink.rgb, 0.35);
        }
        if view.county_borders == 1u && there.y != here.y && there.y != 0u && here.y != 0u {
            return vec4f(view.ink.rgb, 0.18);
        }
    }
    if here.x != 0u && here.x == view.hover {
        return vec4f(view.paper.rgb, 0.3);
    }
    discard;
}
