use glam::{DMat4, DVec3, DVec4, Mat4};
use tiles_core::FocusPose;

#[derive(Debug, Clone)]
pub struct OrbitCamera {
    pub target: DVec3,
    pub local_up: DVec3,
    pub yaw_rad: f64,
    pub pitch_rad: f64,
    pub distance: f64,
    pub fov_y_rad: f64,
    pub z_near: f64,
}

impl Default for OrbitCamera {
    fn default() -> Self {
        Self {
            target: DVec3::ZERO,
            local_up: DVec3::Z,
            yaw_rad: 0.6,
            pitch_rad: 0.75,
            distance: 500.0,
            fov_y_rad: 50.0_f64.to_radians(),
            z_near: 0.5,
        }
    }
}

impl OrbitCamera {
    pub fn focus_on(&mut self, pose: FocusPose) {
        self.target = pose.target;
        self.local_up = if pose.local_up.length_squared() > 1e-8 {
            pose.local_up.normalize()
        } else {
            DVec3::Z
        };
        let half_fov = (self.fov_y_rad * 0.5).max(0.1);
        self.distance = (pose.radius / half_fov.sin() * 1.15).clamp(2.0, 2.0e7);
        self.z_near = (self.distance * 1e-4).clamp(0.1, 5.0);
        self.pitch_rad = 0.72;
    }

    fn basis(&self) -> (DVec3, DVec3, DVec3) {
        let up = self.local_up.normalize();
        let ref_axis = if up.z.abs() < 0.99 { DVec3::Z } else { DVec3::X };
        let east = ref_axis.cross(up).normalize();
        let north = up.cross(east).normalize();
        (east, north, up)
    }

    pub fn eye_world(&self) -> DVec3 {
        let (east, north, up) = self.basis();
        let cp = self.pitch_rad.cos();
        let sp = self.pitch_rad.sin();
        let horiz = north * self.yaw_rad.cos() + east * self.yaw_rad.sin();
        let offset_dir = (horiz * cp + up * sp).normalize();
        self.target + offset_dir * self.distance
    }

    pub fn camera_axes(&self) -> (DVec3, DVec3, DVec3) {
        let eye = self.eye_world();
        let forward = (self.target - eye).normalize();
        let (east, _, up) = self.basis();
        let mut right = forward.cross(up);
        if right.length_squared() < 1e-8 {
            right = east;
        } else {
            right = right.normalize();
        }
        let cam_up = right.cross(forward).normalize();
        (right, cam_up, forward)
    }

    pub fn orbit(&mut self, dx_px: f64, dy_px: f64) {
        self.yaw_rad -= dx_px * 0.006;
        self.pitch_rad = (self.pitch_rad + dy_px * 0.006).clamp(-1.52, 1.54);
    }

    pub fn pan(&mut self, dx_px: f64, dy_px: f64, viewport_h: f64) {
        let (right, cam_up, _) = self.camera_axes();
        let scale = (2.0 * self.distance * (self.fov_y_rad * 0.5).tan()) / viewport_h.max(1.0);
        self.target -= right * (dx_px * scale);
        self.target += cam_up * (dy_px * scale);
    }

    pub fn zoom(&mut self, delta_steps: f64) {
        let factor = (0.88_f64).powf(delta_steps);
        self.distance = (self.distance * factor).clamp(1.0, 5.0e7);
        self.z_near = (self.distance * 1e-4).clamp(0.05, 5.0);
    }

    /// 计算相机相对(RTE)的 View-Projection 矩阵 (f64) 与 Reversed-Z 无限远投影
    pub fn view_proj_rel_cam_f64(&self, aspect: f64) -> DMat4 {
        let eye = self.eye_world();
        let (_, _, up) = self.basis();
        let forward = (self.target - eye).normalize();
        let view_rot = DMat4::look_to_rh(DVec3::ZERO, forward, up);
        let proj = reversed_z_infinite_perspective_f64(self.fov_y_rad, aspect.max(0.01), self.z_near);
        proj * view_rot
    }

    pub fn view_proj_rel_cam_f32(&self, aspect: f64) -> Mat4 {
        self.view_proj_rel_cam_f64(aspect).as_mat4()
    }

    /// 提取相机相对空间的 6 个视锥裁剪平面: normal · p_rel + d >= 0
    pub fn frustum_planes_rel_cam(&self, aspect: f64) -> [DVec4; 6] {
        let m = self.view_proj_rel_cam_f64(aspect);
        let r0 = DVec4::new(m.x_axis.x, m.y_axis.x, m.z_axis.x, m.w_axis.x);
        let r1 = DVec4::new(m.x_axis.y, m.y_axis.y, m.z_axis.y, m.w_axis.y);
        let r2 = DVec4::new(m.x_axis.z, m.y_axis.z, m.z_axis.z, m.w_axis.z);
        let r3 = DVec4::new(m.x_axis.w, m.y_axis.w, m.z_axis.w, m.w_axis.w);

        let raw = [
            r3 + r0, // Left
            r3 - r0, // Right
            r3 + r1, // Bottom
            r3 - r1, // Top
            r2,      // Far (z >= 0)
            r3 - r2, // Near (z <= w)
        ];
        raw.map(normalize_plane)
    }

    pub fn screen_to_ray(&self, px: f64, py: f64, width: f64, height: f64) -> (DVec3, DVec3) {
        let aspect = (width / height.max(1.0)).max(0.01);
        let ndc_x = (2.0 * px / width.max(1.0)) - 1.0;
        let ndc_y = 1.0 - (2.0 * py / height.max(1.0));
        let inv_vp = self.view_proj_rel_cam_f64(aspect).inverse();

        let p_near_clip = inv_vp * DVec4::new(ndc_x, ndc_y, 1.0, 1.0);
        let p_far_clip = inv_vp * DVec4::new(ndc_x, ndc_y, 0.25, 1.0);

        let p_near = p_near_clip.truncate() / p_near_clip.w;
        let p_far = p_far_clip.truncate() / p_far_clip.w;
        let dir = (p_far - p_near).normalize();
        (self.eye_world(), dir)
    }
}

fn normalize_plane(p: DVec4) -> DVec4 {
    let len = DVec3::new(p.x, p.y, p.z).length();
    if len > 1e-12 {
        p / len
    } else {
        p
    }
}

/// WebGPU NDC (z in [0, 1]) 的 Reversed-Z 无限远右手透视矩阵 (z_near -> 1.0, inf -> 0.0)
fn reversed_z_infinite_perspective_f64(fov_y_rad: f64, aspect: f64, z_near: f64) -> DMat4 {
    let f = 1.0 / (fov_y_rad * 0.5).tan();
    DMat4::from_cols(
        DVec4::new(f / aspect, 0.0, 0.0, 0.0),
        DVec4::new(0.0, f, 0.0, 0.0),
        DVec4::new(0.0, 0.0, 0.0, -1.0),
        DVec4::new(0.0, 0.0, z_near, 0.0),
    )
}
