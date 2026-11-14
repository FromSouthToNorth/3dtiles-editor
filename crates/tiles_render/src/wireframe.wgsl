struct FrameUniform {
    view_proj: mat4x4<f32>,
    sun_dir: vec4<f32>,
    eye_fwd: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> frame: FrameUniform;

struct LineIn {
    @location(0) rel_pos: vec3<f32>,
    @location(1) color: vec4<f32>,
};

struct LineOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_line(in: LineIn) -> LineOut {
    var out: LineOut;
    out.clip_pos = frame.view_proj * vec4<f32>(in.rel_pos, 1.0);
    out.color = in.color;
    return out;
}

@fragment
fn fs_line(in: LineOut) -> @location(0) vec4<f32> {
    return in.color;
}
