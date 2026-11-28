// Copies a drawn frame onto a window's surface, or converts a frame drawn
// over a transparent background into the image it is read back as. One
// triangle covers the target; each pixel samples the frame at the same
// place, so a frame drawn at the target's size lands pixel for pixel.
//
// The frame holds linear colour premultiplied by alpha, sRGB-encoded, and
// its sRGB texture decodes that to linear light when sampled.

@group(0) @binding(0) var frame: texture_2d<f32>;
@group(0) @binding(1) var frame_sampler: sampler;

struct Blit {
    @builtin(position) position: vec4<f32>,
    // Texture coordinates, 0 to 1 across the target, top left first.
    @location(0) uv: vec2<f32>,
};

// Vertices 0, 1 and 2 at uv (0, 0), (2, 0) and (0, 2): a triangle twice the
// target's size whose inside covers all of it.
@vertex
fn vs_blit(@builtin(vertex_index) index: u32) -> Blit {
    let uv = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    var out: Blit;
    out.position = vec4<f32>(2.0 * uv.x - 1.0, 1.0 - 2.0 * uv.y, 0.0, 1.0);
    out.uv = uv;
    return out;
}

// Linear light, 0 to 1, as sRGB values, as an sRGB format encodes on write.
fn encode(c: vec3<f32>) -> vec3<f32> {
    let low = 12.92 * c;
    let high = 1.055 * pow(c, vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(high, low, c <= vec3<f32>(0.0031308));
}

// For a target that takes linear light: an sRGB format, which encodes on
// write, or a floating-point one. Alpha is carried through unchanged, so
// the colour stays premultiplied by it.
@fragment
fn fs_linear(in: Blit) -> @location(0) vec4<f32> {
    return textureSample(frame, frame_sampler, in.uv);
}

// For a plain 8- or 10-bit target, which stores what it is given: the
// sampled linear light is encoded to sRGB here, and alpha is carried
// through unchanged.
@fragment
fn fs_encode(in: Blit) -> @location(0) vec4<f32> {
    let c = textureSample(frame, frame_sampler, in.uv);
    return vec4<f32>(encode(c.rgb), c.a);
}

// The frame's pixel at this one with its colour divided by its alpha in
// linear light and encoded to sRGB: the straight colour, or 0 where
// nothing covers the pixel. Read with textureLoad, so each pixel is taken
// exactly as stored.
fn straight(position: vec4<f32>) -> vec4<f32> {
    let p = textureLoad(frame, vec2<i32>(position.xy), 0);
    if (p.a <= 0.0) {
        return vec4<f32>(0.0);
    }
    return vec4<f32>(encode(min(p.rgb / p.a, vec3<f32>(1.0))), p.a);
}

// For a read-back image in plain RGBA8 with straight alpha.
@fragment
fn fs_straight(in: Blit) -> @location(0) vec4<f32> {
    return straight(in.position);
}

// For a read-back image in plain RGBA8 with premultiplied alpha: the
// straight sRGB colour multiplied by alpha.
@fragment
fn fs_premultiplied(in: Blit) -> @location(0) vec4<f32> {
    let s = straight(in.position);
    return vec4<f32>(s.rgb * s.a, s.a);
}
