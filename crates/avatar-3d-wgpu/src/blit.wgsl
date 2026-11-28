// Copies a drawn frame onto a window's surface. One triangle covers the
// target; each pixel samples the frame at the same place, so a frame drawn
// at the target's size lands pixel for pixel.

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

// For a target that takes linear light: an sRGB format, which encodes on
// write, or a floating-point one. The frame's sRGB texture decodes to
// linear light when sampled.
@fragment
fn fs_linear(in: Blit) -> @location(0) vec4<f32> {
    return vec4<f32>(textureSample(frame, frame_sampler, in.uv).rgb, 1.0);
}

// For a plain 8- or 10-bit target, which stores what it is given: the
// sampled linear light is encoded to sRGB here.
@fragment
fn fs_encode(in: Blit) -> @location(0) vec4<f32> {
    let c = textureSample(frame, frame_sampler, in.uv).rgb;
    let low = 12.92 * c;
    let high = 1.055 * pow(c, vec3<f32>(1.0 / 2.4)) - 0.055;
    return vec4<f32>(select(high, low, c <= vec3<f32>(0.0031308)), 1.0);
}
