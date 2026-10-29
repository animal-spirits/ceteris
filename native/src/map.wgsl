// Fills coloured per feature from a texture (ADR 0010); feature ids go to a second target for
// picking. Instance k draws the world shifted by (first + k) in x, so the map wraps.

struct View {
    centre: vec2f,
    k: vec2f,
    first: f32,
}

@group(0) @binding(0) var<uniform> view: View;
@group(0) @binding(1) var colours: texture_2d<f32>;

const TEX_WIDTH: u32 = 1024u;

struct VsOut {
    @builtin(position) pos: vec4f,
    @location(0) colour: vec4f,
    @location(1) @interpolate(flat) id: u32,
}

@vertex
fn vs(@location(0) xy: vec2f, @location(1) feature: u32, @builtin(instance_index) copy: u32) -> VsOut {
    let d = xy + vec2f(view.first + f32(copy), 0.0) - view.centre;
    var out: VsOut;
    out.pos = vec4f(d * view.k, 0.0, 1.0);
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
    return FsOut(in.colour, in.id);
}
