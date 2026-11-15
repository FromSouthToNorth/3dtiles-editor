// 点云 billboard 点精灵: 每个点实例展开为屏幕恒定尺寸的小四边形
// （WebGPU 的 PointList 拓扑固定 1 设备像素, 稀疏点云几乎不可见, 故用实例化四边形）

struct FrameUniform {
    view_proj: mat4x4<f32>,
    sun_dir: vec4<f32>,
    eye_fwd: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> frame: FrameUniform;

struct PointUniform {
    model_rel_cam: mat4x4<f32>,
    // xy = 视口尺寸(px), z = 点直径(px), w = 保留
    viewport: vec4<f32>,
    // rgb = 高亮色, a = 高亮混合强度 (0 = 原色)
    tint: vec4<f32>,
};

@group(1) @binding(0)
var<uniform> draw: PointUniform;

struct PointIn {
    // 实例步进: 相对相机(RTE)前仍为 tile 局部坐标
    @location(0) rel_pos: vec3<f32>,
    @location(1) color: vec4<f32>,
    // 顶点步进: 四边形角点 [-0.5, 0.5]^2
    @location(2) corner: vec2<f32>,
};

struct PointOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
};

@vertex
fn vs_point(in: PointIn) -> PointOut {
    var out: PointOut;
    let rel = draw.model_rel_cam * vec4<f32>(in.rel_pos, 1.0);
    let center = frame.view_proj * rel;
    // 屏幕恒定尺寸: NDC 偏移量 = 角点 * 点直径 / 视口 * w 补偿
    let ndc_step = vec2<f32>(
        draw.viewport.z / draw.viewport.x,
        draw.viewport.z / draw.viewport.y,
    );
    var pos = center;
    pos.x += in.corner.x * ndc_step.x * center.w;
    pos.y += in.corner.y * ndc_step.y * center.w;
    // 四个角点共享中心深度, 避免四边形与网格深度穿插
    out.clip_pos = pos;
    out.uv = in.corner * 2.0;
    out.color = in.color;
    return out;
}

@fragment
fn fs_point(in: PointOut) -> @location(0) vec4<f32> {
    // 圆形点: 裁掉四边形四角
    if (dot(in.uv, in.uv) > 1.0) {
        discard;
    }
    let mixed = mix(in.color, draw.tint, draw.tint.a);
    return vec4<f32>(mixed.rgb, 1.0);
}
