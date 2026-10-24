// GPU fill layer (ADR 0010): triangles from the pack's `.fill` buffers are uploaded once;
// each feature's colour is read in the vertex shader from an RGBA texture indexed by feature.
// A recolour uploads that texture (4 bytes per feature) and nothing else, where MapLibre's
// feature state rebuilds per-vertex data on the CPU. Projection goes through MapLibre's own
// shader prelude, so the layer follows the map onto the globe.

import type { CustomLayerInterface, CustomRenderMethodInput, Map as MlMap } from "maplibre-gl";

const TEX_WIDTH = 1024;

interface Program {
  program: WebGLProgram;
  loc: Record<string, WebGLUniformLocation | null>;
}

interface Geometry {
  vao: WebGLVertexArrayObject;
  count: number;
  features: number;
}

function parse(buf: ArrayBuffer) {
  const head = new DataView(buf);
  const magic = String.fromCharCode(...new Uint8Array(buf, 0, 4));
  if (magic !== "CFIL" || head.getUint32(4, true) !== 1) throw new Error("not a version-1 fill buffer");
  const [v, i, f] = [head.getUint32(8, true), head.getUint32(12, true), head.getUint32(16, true)];
  const xy = new Float32Array(buf, 20, 2 * v);
  const feature = new Uint32Array(buf, 20 + 8 * v, v);
  const index = new Uint32Array(buf, 20 + 12 * v, i);
  return { xy, feature, index, features: f };
}

function compile(gl: WebGL2RenderingContext, type: number, source: string) {
  const s = gl.createShader(type)!;
  gl.shaderSource(s, source);
  gl.compileShader(s);
  if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s) ?? "shader error");
  return s;
}

export class FillLayer implements CustomLayerInterface {
  readonly type = "custom" as const;
  readonly renderingMode = "2d" as const;
  private map?: MlMap;
  private gl?: WebGL2RenderingContext;
  private geometry?: Geometry;
  private texture?: WebGLTexture;
  private colours: Uint8Array | null = null;
  private hover = -1;
  private programs = new Map<string, Program>();
  /** Resolves when the geometry is on the GPU. */
  readonly ready: Promise<void>;
  private resolveReady!: () => void;

  constructor(
    readonly id: string,
    private readonly url: string,
    private readonly minzoom = 0,
  ) {
    this.ready = new Promise((r) => (this.resolveReady = r));
  }

  onAdd(map: MlMap, gl: WebGL2RenderingContext) {
    this.map = map;
    this.gl = gl;
    fetch(this.url)
      .then((r) => {
        if (!r.ok) throw new Error(`${this.url}: ${r.status}`);
        return r.arrayBuffer();
      })
      .then((buf) => {
        this.upload(parse(buf));
        this.resolveReady();
        map.triggerRepaint();
      });
  }

  private upload({ xy, feature, index, features }: ReturnType<typeof parse>) {
    const gl = this.gl!;
    const vao = gl.createVertexArray()!;
    gl.bindVertexArray(vao);
    const pos = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, pos);
    gl.bufferData(gl.ARRAY_BUFFER, xy, gl.STATIC_DRAW);
    gl.enableVertexAttribArray(0);
    gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0);
    const fid = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, fid);
    gl.bufferData(gl.ARRAY_BUFFER, feature, gl.STATIC_DRAW);
    gl.enableVertexAttribArray(1);
    gl.vertexAttribIPointer(1, 1, gl.UNSIGNED_INT, 0, 0);
    const ibo = gl.createBuffer();
    gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, ibo);
    gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, index, gl.STATIC_DRAW);
    gl.bindVertexArray(null);
    this.geometry = { vao, count: index.length, features };
    this.texture = gl.createTexture()!;
    gl.bindTexture(gl.TEXTURE_2D, this.texture);
    gl.texStorage2D(gl.TEXTURE_2D, 1, gl.RGBA8, TEX_WIDTH, Math.ceil(features / TEX_WIDTH));
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
    if (this.colours) this.writeTexture(this.colours);
  }

  /** One premultiplied-alpha RGBA quadruple per feature. */
  setColours(rgba: Uint8Array) {
    this.colours = rgba;
    if (this.texture) this.writeTexture(rgba);
    this.map?.triggerRepaint();
  }

  setHover(feature: number | null) {
    this.hover = feature ?? -1;
    this.map?.triggerRepaint();
  }

  private writeTexture(rgba: Uint8Array) {
    const gl = this.gl!;
    const rows = Math.ceil(this.geometry!.features / TEX_WIDTH);
    const padded = new Uint8Array(TEX_WIDTH * rows * 4);
    padded.set(rgba.subarray(0, padded.length));
    gl.bindTexture(gl.TEXTURE_2D, this.texture!);
    gl.pixelStorei(gl.UNPACK_ALIGNMENT, 1);
    gl.texSubImage2D(gl.TEXTURE_2D, 0, 0, 0, TEX_WIDTH, rows, gl.RGBA, gl.UNSIGNED_BYTE, padded);
  }

  private program(shader: CustomRenderMethodInput["shaderData"]): Program {
    const cached = this.programs.get(shader.variantName);
    if (cached) return cached;
    const gl = this.gl!;
    const vertex = `#version 300 es
${shader.vertexShaderPrelude}
${shader.define}
layout(location = 0) in vec2 a_pos;
layout(location = 1) in uint a_feature;
uniform highp sampler2D u_colours;
uniform int u_hover;
uniform float u_offset;
flat out vec4 v_colour;
void main() {
  int f = int(a_feature);
  vec4 c = texelFetch(u_colours, ivec2(f % ${TEX_WIDTH}, f / ${TEX_WIDTH}), 0);
  if (f == u_hover) c.rgb = mix(c.rgb, vec3(c.a), 0.25);
  v_colour = c;
  gl_Position = projectTile(a_pos + vec2(u_offset, 0.0));
}`;
    const fragment = `#version 300 es
precision highp float;
flat in vec4 v_colour;
out vec4 fragColor;
void main() { fragColor = v_colour; }`;
    const program = gl.createProgram()!;
    gl.attachShader(program, compile(gl, gl.VERTEX_SHADER, vertex));
    gl.attachShader(program, compile(gl, gl.FRAGMENT_SHADER, fragment));
    gl.linkProgram(program);
    if (!gl.getProgramParameter(program, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(program) ?? "link error");
    const names = [
      "u_colours", "u_hover", "u_offset", "u_projection_matrix", "u_projection_fallback_matrix",
      "u_projection_tile_mercator_coords", "u_projection_clipping_plane", "u_projection_transition",
    ];
    const p = { program, loc: Object.fromEntries(names.map((n) => [n, gl.getUniformLocation(program, n)])) };
    this.programs.set(shader.variantName, p);
    return p;
  }

  render(gl: WebGL2RenderingContext, args: CustomRenderMethodInput) {
    const map = this.map!;
    if (!this.geometry || !this.colours || map.getZoom() < this.minzoom) return;
    const { program, loc } = this.program(args.shaderData);
    const pd = args.defaultProjectionData;
    gl.useProgram(program);
    gl.uniformMatrix4fv(loc.u_projection_matrix, false, pd.mainMatrix);
    gl.uniformMatrix4fv(loc.u_projection_fallback_matrix, false, pd.fallbackMatrix);
    gl.uniform4f(loc.u_projection_tile_mercator_coords, ...(pd.tileMercatorCoords as [number, number, number, number]));
    gl.uniform4f(loc.u_projection_clipping_plane, ...(pd.clippingPlane as [number, number, number, number]));
    gl.uniform1f(loc.u_projection_transition, pd.projectionTransition);
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, this.texture!);
    gl.uniform1i(loc.u_colours, 0);
    gl.uniform1i(loc.u_hover, this.hover);
    gl.bindVertexArray(this.geometry.vao);
    // On the flat map, repeat across the world copies in view; on the globe there is one world.
    const globe = map.getProjection()?.type === "globe";
    const bounds = map.getBounds();
    const first = globe ? 0 : Math.floor((bounds.getWest() + 180) / 360);
    const last = globe ? 0 : Math.floor((bounds.getEast() + 180) / 360);
    for (let wrap = first; wrap <= last; wrap++) {
      gl.uniform1f(loc.u_offset, wrap);
      gl.drawElements(gl.TRIANGLES, this.geometry.count, gl.UNSIGNED_INT, 0);
    }
    gl.bindVertexArray(null);
  }
}

const parsed = new Map<string, [number, number, number]>();

/** Hex colours to premultiplied RGBA bytes, one quadruple per feature. */
export function rgba(colours: string[]): Uint8Array {
  const out = new Uint8Array(colours.length * 4);
  colours.forEach((hex, i) => {
    let c = parsed.get(hex);
    if (!c) {
      const n = parseInt(hex.slice(1), 16);
      c = [(n >> 16) & 255, (n >> 8) & 255, n & 255];
      parsed.set(hex, c);
    }
    out.set([c[0], c[1], c[2], 255], i * 4);
  });
  return out;
}
