use crate::schema::BoundingVolumeJson;
use glam::{DMat3, DMat4, DVec3, DVec4};

const WGS84_A: f64 = 6_378_137.0;
const WGS84_E2: f64 = 0.006_694_379_990_141_316_5;

#[derive(Debug, Clone)]
pub enum BoundingVolume {
    Box {
        center: DVec3,
        half_axes: DMat3,
    },
    Region {
        west: f64,
        south: f64,
        east: f64,
        north: f64,
        min_height: f64,
        max_height: f64,
    },
    Sphere {
        center: DVec3,
        radius: f64,
    },
}

impl BoundingVolume {
    pub fn from_json(j: &BoundingVolumeJson) -> Self {
        match j {
            BoundingVolumeJson::Box { b } => Self::Box {
                center: DVec3::new(b[0], b[1], b[2]),
                half_axes: DMat3::from_cols(
                    DVec3::new(b[3], b[4], b[5]),
                    DVec3::new(b[6], b[7], b[8]),
                    DVec3::new(b[9], b[10], b[11]),
                ),
            },
            BoundingVolumeJson::Region { region } => Self::Region {
                west: region[0],
                south: region[1],
                east: region[2],
                north: region[3],
                min_height: region[4],
                max_height: region[5],
            },
            BoundingVolumeJson::Sphere { sphere } => Self::Sphere {
                center: DVec3::new(sphere[0], sphere[1], sphere[2]),
                radius: sphere[3].max(0.0),
            },
        }
    }

    pub fn kind_name(&self) -> &'static str {
        match self {
            Self::Box { .. } => "box",
            Self::Region { .. } => "region",
            Self::Sphere { .. } => "sphere",
        }
    }

    /// 按 3D Tiles 规范: Box/Sphere 随 tile.transform 变换, Region 为绝对 WGS84 经纬高不随 transform 变换
    pub fn to_world_bounds(&self, world_transform: DMat4) -> WorldBounds {
        match self {
            Self::Box { center, half_axes } => {
                let wc = world_transform.transform_point3(*center);
                let wa = DMat3::from_mat4(world_transform) * *half_axes;
                WorldBounds::from_obb(wc, wa)
            }
            Self::Sphere { center, radius } => {
                let wc = world_transform.transform_point3(*center);
                let lin = DMat3::from_mat4(world_transform);
                let max_scale = lin
                    .x_axis
                    .length()
                    .max(lin.y_axis.length())
                    .max(lin.z_axis.length());
                let wr = (*radius * max_scale).max(1e-3);
                WorldBounds::from_obb(wc, DMat3::from_diagonal(DVec3::splat(wr)))
            }
            Self::Region {
                west,
                south,
                east,
                north,
                min_height,
                max_height,
            } => {
                let mut min_p = DVec3::splat(f64::INFINITY);
                let mut max_p = DVec3::splat(f64::NEG_INFINITY);
                let lons = [*west, (*west + *east) * 0.5, *east];
                let lats = [*south, (*south + *north) * 0.5, *north];
                let hs = [*min_height, *max_height];
                for &lon in &lons {
                    for &lat in &lats {
                        for &h in &hs {
                            let p = geodetic_to_ecef(lon, lat, h);
                            min_p = min_p.min(p);
                            max_p = max_p.max(p);
                        }
                    }
                }
                let center = (min_p + max_p) * 0.5;
                let half = (max_p - min_p) * 0.5;
                WorldBounds::from_obb(center, DMat3::from_diagonal(half))
            }
        }
    }
}

pub fn geodetic_to_ecef(lon_rad: f64, lat_rad: f64, height_m: f64) -> DVec3 {
    let sin_lat = lat_rad.sin();
    let cos_lat = lat_rad.cos();
    let n = WGS84_A / (1.0 - WGS84_E2 * sin_lat * sin_lat).sqrt();
    let x = (n + height_m) * cos_lat * lon_rad.cos();
    let y = (n + height_m) * cos_lat * lon_rad.sin();
    let z = (n * (1.0 - WGS84_E2) + height_m) * sin_lat;
    DVec3::new(x, y, z)
}

#[derive(Debug, Clone, Copy)]
pub struct WorldBounds {
    pub center: DVec3,
    pub half_axes: DMat3,
    pub inv_half_axes: DMat3,
    pub has_valid_inv: bool,
    pub radius: f64,
    pub aabb_min: DVec3,
    pub aabb_max: DVec3,
}

impl WorldBounds {
    pub fn from_obb(center: DVec3, half_axes: DMat3) -> Self {
        let u = half_axes.x_axis;
        let v = half_axes.y_axis;
        let w = half_axes.z_axis;
        let extent = u.abs() + v.abs() + w.abs();
        let aabb_min = center - extent;
        let aabb_max = center + extent;

        let mut max_r2 = 0.0_f64;
        for sx in [-1.0, 1.0] {
            for sy in [-1.0, 1.0] {
                for sz in [-1.0, 1.0] {
                    let corner = u * sx + v * sy + w * sz;
                    max_r2 = max_r2.max(corner.length_squared());
                }
            }
        }
        let det = half_axes.determinant();
        let (inv_half_axes, has_valid_inv) = if det.abs() > 1e-18 {
            (half_axes.inverse(), true)
        } else {
            (DMat3::IDENTITY, false)
        };

        Self {
            center,
            half_axes,
            inv_half_axes,
            has_valid_inv,
            radius: max_r2.sqrt().max(1e-3),
            aabb_min,
            aabb_max,
        }
    }

    pub fn distance_to_point(&self, point: DVec3) -> f64 {
        let delta = point - self.center;
        if self.has_valid_inv {
            let local = self.inv_half_axes * delta;
            let clamped = local.clamp(DVec3::splat(-1.0), DVec3::splat(1.0));
            let closest_world = self.center + self.half_axes * clamped;
            point.distance(closest_world)
        } else {
            (delta.length() - self.radius).max(0.0)
        }
    }

    /// 6个相机相对空间视锥平面测试: 平面方程 normal·(x - camera_pos) + d >= 0
    pub fn intersects_frustum(&self, planes_rel_cam: &[DVec4; 6], camera_pos: DVec3) -> bool {
        let rel_center = self.center - camera_pos;
        let u = self.half_axes.x_axis;
        let v = self.half_axes.y_axis;
        let w = self.half_axes.z_axis;

        for p in planes_rel_cam {
            let n = DVec3::new(p.x, p.y, p.z);
            let r = n.dot(u).abs() + n.dot(v).abs() + n.dot(w).abs();
            let signed_dist = n.dot(rel_center) + p.w;
            if signed_dist + r < 0.0 {
                return false;
            }
        }
        true
    }

    pub fn intersect_ray(&self, ray_origin: DVec3, ray_dir: DVec3) -> Option<f64> {
        if self.has_valid_inv {
            let o = self.inv_half_axes * (ray_origin - self.center);
            let d = self.inv_half_axes * ray_dir;
            intersect_unit_cube(o, d)
        } else {
            let oc = ray_origin - self.center;
            let b = oc.dot(ray_dir);
            let c = oc.dot(oc) - self.radius * self.radius;
            let disc = b * b - c;
            if disc < 0.0 {
                None
            } else {
                let t = (-b - disc.sqrt()).max(0.0);
                Some(t)
            }
        }
    }

    /// 返回 24 个端点 (12条线段的起点/终点，以世界坐标表示)
    pub fn corners(&self) -> [DVec3; 8] {
        let u = self.half_axes.x_axis;
        let v = self.half_axes.y_axis;
        let w = self.half_axes.z_axis;
        let c = self.center;
        [
            c - u - v - w,
            c + u - v - w,
            c + u + v - w,
            c - u + v - w,
            c - u - v + w,
            c + u - v + w,
            c + u + v + w,
            c - u + v + w,
        ]
    }
}

pub const BOX_EDGE_INDICES: [(usize, usize); 12] = [
    (0, 1),
    (1, 2),
    (2, 3),
    (3, 0),
    (4, 5),
    (5, 6),
    (6, 7),
    (7, 4),
    (0, 4),
    (1, 5),
    (2, 6),
    (3, 7),
];

fn intersect_unit_cube(o: DVec3, d: DVec3) -> Option<f64> {
    let mut tmin = 0.0_f64;
    let mut tmax = f64::INFINITY;
    for i in 0..3 {
        let orig = o[i];
        let dir = d[i];
        if dir.abs() < 1e-12 {
            if orig < -1.0 || orig > 1.0 {
                return None;
            }
        } else {
            let inv = 1.0 / dir;
            let mut t1 = (-1.0 - orig) * inv;
            let mut t2 = (1.0 - orig) * inv;
            if t1 > t2 {
                std::mem::swap(&mut t1, &mut t2);
            }
            tmin = tmin.max(t1);
            tmax = tmax.min(t2);
            if tmin > tmax {
                return None;
            }
        }
    }
    Some(tmin)
}
