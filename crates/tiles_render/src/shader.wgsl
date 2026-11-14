struct FrameUniform {
    view_proj: mat4x4<f32>,
    sun_dir: vec4<f32>,
    eye_fwd: vec4<f32>,
};

struct DrawUniform {
    model_rel_cam: mat4x4<f32>,
    base_color_factor: vec4<f32>,
    flags: vec4<f32>, // x = selected (0.0 | 1.0)
};

@group(0) @binding(0)
var<uniform> frame: FrameUniform;

@group(1) @binding(0)
var<uniform> draw: DrawUniform;
@group(1) @binding(1)
var base_tex: texture_2d<f32>;
@group(1) @binding(2)
var base_sampler: sampler;

struct VsIn {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

struct VsOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) rel_pos: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

@vertex
fn vs_main(v: VsIn) -> VsOut {
    var out: VsOut;
    let rel = draw.model_rel_cam * vec4<f32>(v.pos, 1.0);
    out.clip_pos = frame.view_proj * rel;
    out.rel_pos = rel.xyz;
    out.world_normal = (draw.model_rel_cam * vec4<f32>(v.normal, 0.0)).xyz;
    out.uv = v.uv;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let tex_color = textureSample(base_tex, base_sampler, in.uv);
    let albedo = tex_color.rgb * draw.base_color_factor.rgb;

    var n: vec3<f32>;
    if (dot(in.world_normal, in.world_normal) > 0.05) {
        n = normalize(in.world_normal);
    } else {
        // 实景倾斜摄影常缺失顶点法线，使用屏幕导数实时恢复几何面法线
        let dx = dpdx(in.rel_pos);
        let dy = dpdy(in.rel_pos);
        let c = cross(dx, dy);
        n = select(vec3<f32>(0.0, 0.0, 1.0), normalize(c), dot(c, c) > 1e-12);
    }

    // 确保双面法线始终朝向相机一侧（rel_pos 原点即相机）
    let view_dir = normalize(-in.rel_pos);
    if (dot(n, view_dir) < 0.0) {
        n = -n;
    }

    let sun = normalize(frame.sun_dir.xyz);
    let ndl = max(dot(n, sun), 0.0);
    let headlight = max(dot(n, view_dir), 0.0);
    let lighting = 0.58 + 0.30 * ndl + 0.16 * headlight;

    var final_rgb = albedo * lighting;
    if (draw.flags.x > 0.5) {
        final_rgb = mix(final_rgb, vec3<f32>(0.22, 0.74, 0.98), 0.32);
    }
    return vec4<f32>(final_rgb, tex_color.a * draw.base_color_factor.a);
}
