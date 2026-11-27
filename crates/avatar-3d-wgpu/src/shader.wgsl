// The small robots' shading model and the ground shadow.
//
// Every solid is shaded the same way: a wrapped key light fixed to the
// camera blends from a dark tone of the surface's own colour to its base
// colour, so a surface facing the viewer stays close to the 2D colour and
// shade never turns grey. A soft sky term, a weak fill light, a darkening
// at grazing angles that keeps silhouettes crisp at icon size, and a
// highlight whose strength, sharpness and tint the finish sets complete
// it. There is no tone mapping. Colours are linear here; the target's
// sRGB format encodes them on write.
//
// Textures are 2D drawings projected along the depth axis: each vertex
// carries its rest-local 2D position (the projection coordinate) and each
// texture a binding that maps it to uv. The upload stores every texel as
// linear colour premultiplied by alpha, sRGB-encoded, so sampling returns
// linear premultiplied colour and blending is `t + under · (1 − t.a)`.
// On one surface the order is paint, marks (shaded like the paint), then
// the face (unlit, at the brightness the 2D drawing bakes in). Where the
// rest surface turns away from the depth axis, a projected drawing would
// stretch across it, so a texture fades out as the rest normal's z falls
// from 0.42 to 0.12.

// Lights in view space, fixed to the camera: the key from above right in
// front, the fill from the left.
const KEY = vec3<f32>(0.36, 0.5, 0.79);
const FILL = vec3<f32>(-0.7, 0.1, 0.5);
// How far the key light wraps round the shadow side.
const WRAP: f32 = 0.45;
// The rest normal z over which a projected texture fades out.
const FADE = vec2<f32>(0.12, 0.42);

// Per-vertex flags.
const FLAG_TEXTURED_FRONT: u32 = 1u;
const FLAG_FACING_FRONT: u32 = 2u;
const NO_TEXTURE: u32 = 0xffffffffu;

// The ground shadow's colour: sRGB (0.18, 0.14, 0.2) in linear terms.
const SHADOW_COLOUR = vec3<f32>(0.027212, 0.017389, 0.033105);

struct Frame {
    view: mat4x4<f32>,
    projection: mat4x4<f32>,
};

struct RobotUniform {
    // Where the robot stands in the scene.
    model: mat4x4<f32>,
    // The texture shown in each face slot this frame, or -1: slot 1 is the
    // head's face, slots 2 and 3 the left and right stalk eyes.
    faces: vec4<i32>,
};

struct Joint {
    // Pose matrix times the inverse rest matrix: rest model space to posed
    // model space.
    skin: mat4x4<f32>,
    // The inverse transpose of the skin matrix's upper 3×3, for normals.
    normal: mat4x4<f32>,
};

struct Material {
    // Linear base colour, and in w how much it shows unlit (its emission).
    base: vec4<f32>,
    // The dark tone as a ratio of the base colour, per channel.
    dark: vec4<f32>,
    // Highlight strength, highlight sharpness, edge darkening, metal tint.
    finish: vec4<f32>,
};

@group(0) @binding(0) var<uniform> frame: Frame;

@group(1) @binding(0) var<uniform> robot: RobotUniform;
@group(1) @binding(1) var<storage, read> joints: array<Joint>;
@group(1) @binding(2) var<storage, read> materials: array<Material>;
// Per texture: canvas origin x, origin y and edge, in projection units.
@group(1) @binding(3) var<storage, read> bindings: array<vec4<f32>>;
@group(1) @binding(4) var layers: texture_2d_array<f32>;
@group(1) @binding(5) var layer_sampler: sampler;

struct VertexIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) projection: vec2<f32>,
    @location(3) joints: vec4<u32>,
    @location(4) weights: vec4<f32>,
    // Material, flags, face slot (0 for none) and marks texture.
    @location(5) info: vec4<u32>,
};

struct VertexOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) view_position: vec3<f32>,
    @location(1) view_normal: vec3<f32>,
    @location(2) projection: vec2<f32>,
    @location(3) @interpolate(flat) info: vec4<u32>,
    // The rest normal's z, which projected textures fade by.
    @location(4) rest_nz: f32,
};

@vertex
fn vs_robot(v: VertexIn) -> VertexOut {
    var p = vec3<f32>(0.0);
    var n = vec3<f32>(0.0);
    for (var i = 0u; i < 4u; i++) {
        let w = v.weights[i];
        if (w > 0.0) {
            let j = joints[v.joints[i]];
            p += w * (j.skin * vec4<f32>(v.position, 1.0)).xyz;
            n += w * (j.normal * vec4<f32>(v.normal, 0.0)).xyz;
        }
    }
    let view = frame.view * robot.model * vec4<f32>(p, 1.0);
    var out: VertexOut;
    out.clip = frame.projection * view;
    out.view_position = view.xyz;
    out.view_normal = (frame.view * robot.model * vec4<f32>(n, 0.0)).xyz;
    out.projection = v.projection;
    out.info = v.info;
    out.rest_nz = v.normal.z;
    return out;
}

// Texture `index` at projection coordinate `p`, or nothing outside its
// canvas. The gradients are taken before any branch, so this may run in
// non-uniform control flow.
fn layer_sample(index: u32, p: vec2<f32>, dx: vec2<f32>, dy: vec2<f32>) -> vec4<f32> {
    let b = bindings[index];
    let uv = (p - b.xy) / b.z;
    if (any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0))) {
        return vec4<f32>(0.0);
    }
    return textureSampleGrad(layers, layer_sampler, uv, i32(index), dx / b.z, dy / b.z);
}

@fragment
fn fs_robot(in: VertexOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    let dx = dpdx(in.projection);
    let dy = dpdy(in.projection);
    let m = materials[in.info.x];
    let flags = in.info.y;
    let fade = smoothstep(FADE.x, FADE.y, in.rest_nz);

    // Paint, then the hull's marks over it, shaded together.
    var base = m.base.rgb;
    if ((flags & FLAG_FACING_FRONT) != 0u && in.info.w != NO_TEXTURE) {
        let t = layer_sample(in.info.w, in.projection, dx, dy) * fade;
        base = t.rgb + base * (1.0 - t.a);
    }

    var n = normalize(in.view_normal);
    if (!front) {
        n = -n;
    }
    let v = normalize(-in.view_position);
    let key = normalize(KEY);
    let dark = base * m.dark.rgb;
    var d = clamp((dot(n, key) + WRAP) / (1.0 + WRAP), 0.0, 1.0);
    d = d * d * (3.0 - 2.0 * d);
    var col = mix(dark, base, min(d / 0.86, 1.0)) + base * 0.03 * max(d - 0.9, 0.0) / 0.1;
    col *= 0.97 + 0.06 * (0.5 + 0.5 * n.y);
    col += base * 0.03 * max(dot(n, normalize(FILL)), 0.0);
    let graze = 1.0 - max(dot(n, v), 0.0);
    col = mix(col, dark * 0.82, graze * graze * graze * m.finish.z);
    // Lamps and flames show their own colour whatever the light.
    col = mix(col, base, m.base.a);
    let h = normalize(key + v);
    var highlight = pow(max(dot(n, h), 1e-6), m.finish.y) * m.finish.x * (0.35 + 0.65 * d);
    // A white highlight washes out dark paint, so it is held back on dark
    // colours, though not on metal.
    let luminance = dot(base, vec3<f32>(0.3, 0.59, 0.11));
    highlight *= mix(mix(0.3, 1.0, sqrt(clamp(luminance, 0.0, 1.0))), 1.0, m.finish.w);
    col += mix(vec3<f32>(1.0), base * 1.6, m.finish.w) * highlight;

    // The face, unlit, over everything.
    let slot = in.info.z;
    if ((flags & FLAG_TEXTURED_FRONT) != 0u && slot > 0u) {
        let face = robot.faces[slot];
        if (face >= 0) {
            let t = layer_sample(u32(face), in.projection, dx, dy) * fade;
            col = t.rgb + col * (1.0 - t.a);
        }
    }
    return vec4<f32>(col, 1.0);
}

struct ShadowIn {
    // The disc's centre on the floor.
    @location(0) centre: vec3<f32>,
    // Its half extents along x and z.
    @location(1) radii: vec2<f32>,
    @location(2) opacity: f32,
};

struct ShadowOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) offset: vec2<f32>,
    @location(1) opacity: f32,
};

@vertex
fn vs_shadow(@builtin(vertex_index) i: u32, s: ShadowIn) -> ShadowOut {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(1.0, -1.0),
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(-1.0, 1.0),
        vec2<f32>(1.0, 1.0),
    );
    let c = corners[i];
    let world = vec4<f32>(s.centre.x + c.x * s.radii.x, s.centre.y, s.centre.z + c.y * s.radii.y, 1.0);
    var out: ShadowOut;
    out.clip = frame.projection * frame.view * world;
    out.offset = c;
    out.opacity = s.opacity;
    return out;
}

// A soft disc: full at the centre, fading to nothing at the rim.
@fragment
fn fs_shadow(in: ShadowOut) -> @location(0) vec4<f32> {
    let a = 1.0 - smoothstep(0.0, 1.0, length(in.offset));
    return vec4<f32>(SHADOW_COLOUR, a * a * in.opacity);
}
