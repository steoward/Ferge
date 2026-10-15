use crate::vec::{Vec3, Vec4};
use crate::quat::Quat;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mat4 {
    pub m: [f32; 16],
}

impl Mat4 {
    pub const fn identity() -> Self {
        Self {
            m: [
                1.0, 0.0, 0.0, 0.0,
                0.0, 1.0, 0.0, 0.0,
                0.0, 0.0, 1.0, 0.0,
                0.0, 0.0, 0.0, 1.0,
            ],
        }
    }

    pub const fn from_cols(cols: [Vec4; 4]) -> Self {
        Self {
            m: [
                cols[0].x, cols[0].y, cols[0].z, cols[0].w,
                cols[1].x, cols[1].y, cols[1].z, cols[1].w,
                cols[2].x, cols[2].y, cols[2].z, cols[2].w,
                cols[3].x, cols[3].y, cols[3].z, cols[3].w,
            ],
        }
    }

    pub fn perspective(fov_y: f32, aspect: f32, near: f32, far: f32) -> Self {
        let tan_half_fov = (fov_y * 0.5).tan();
        let mut m = [0.0f32; 16];
        m[0] = 1.0 / (aspect * tan_half_fov);
        m[5] = 1.0 / tan_half_fov;
        m[10] = -(far + near) / (far - near);
        m[11] = -1.0;
        m[14] = -(2.0 * far * near) / (far - near);
        Self { m }
    }

    pub fn orthographic(left: f32, right: f32, bottom: f32, top: f32, near: f32, far: f32) -> Self {
        let mut m = [0.0f32; 16];
        m[0] = 2.0 / (right - left);
        m[5] = 2.0 / (top - bottom);
        m[10] = -2.0 / (far - near);
        m[12] = -(right + left) / (right - left);
        m[13] = -(top + bottom) / (top - bottom);
        m[14] = -(far + near) / (far - near);
        m[15] = 1.0;
        Self { m }
    }

    pub fn look_at(eye: Vec3, target: Vec3, up: Vec3) -> Self {
        let f = (target - eye).normalize();
        let s = f.cross(up).normalize();
        let u = s.cross(f);

        let mut m = [0.0f32; 16];
        m[0] = s.x;
        m[4] = s.y;
        m[8] = s.z;
        m[1] = u.x;
        m[5] = u.y;
        m[9] = u.z;
        m[2] = -f.x;
        m[6] = -f.y;
        m[10] = -f.z;
        m[12] = -s.dot(&eye);
        m[13] = -u.dot(&eye);
        m[14] = f.dot(&eye);
        m[15] = 1.0;
        Self { m }
    }

    pub fn translate(&self, translation: Vec3) -> Self {
        let mut result = *self;
        result.m[12] = self.m[0] * translation.x + self.m[4] * translation.y + self.m[8] * translation.z + self.m[12];
        result.m[13] = self.m[1] * translation.x + self.m[5] * translation.y + self.m[9] * translation.z + self.m[13];
        result.m[14] = self.m[2] * translation.x + self.m[6] * translation.y + self.m[10] * translation.z + self.m[14];
        result.m[15] = self.m[3] * translation.x + self.m[7] * translation.y + self.m[11] * translation.z + self.m[15];
        result
    }

    pub fn scale(&self, scale: Vec3) -> Self {
        let mut result = *self;
        result.m[0] *= scale.x;
        result.m[1] *= scale.x;
        result.m[2] *= scale.x;
        result.m[3] *= scale.x;
        result.m[4] *= scale.y;
        result.m[5] *= scale.y;
        result.m[6] *= scale.y;
        result.m[7] *= scale.y;
        result.m[8] *= scale.z;
        result.m[9] *= scale.z;
        result.m[10] *= scale.z;
        result.m[11] *= scale.z;
        result
    }

    pub fn rotate(&self, rotation: Quat) -> Self {
        let rot = rotation.to_mat3();
        let mut result = [0.0f32; 16];

        // Storage is column-major (m[col * 4 + row]), matching transform_point,
        // transform_vector and from_cols. Writing the rotation block row-major
        // here would transpose R and therefore invert the rotation, so the
        // indices below deliberately walk the other way round.
        for row in 0..3 {
            for col in 0..3 {
                result[col * 4 + row] = self.m[row] * rot[col]
                    + self.m[4 + row] * rot[3 + col]
                    + self.m[8 + row] * rot[6 + col];
            }
        }

        // The last row and the translation column are unaffected by a rotation.
        result[3] = self.m[3];
        result[7] = self.m[7];
        result[11] = self.m[11];
        result[12] = self.m[12];
        result[13] = self.m[13];
        result[14] = self.m[14];
        result[15] = self.m[15];

        Self { m: result }
    }

    pub fn transpose(&self) -> Self {
        let mut m = [0.0f32; 16];
        for i in 0..4 {
            for j in 0..4 {
                m[j * 4 + i] = self.m[i * 4 + j];
            }
        }
        Self { m }
    }

    pub fn transform_point(&self, point: Vec3) -> Vec3 {
        let x = self.m[0] * point.x + self.m[4] * point.y + self.m[8] * point.z + self.m[12];
        let y = self.m[1] * point.x + self.m[5] * point.y + self.m[9] * point.z + self.m[13];
        let z = self.m[2] * point.x + self.m[6] * point.y + self.m[10] * point.z + self.m[14];
        let w = self.m[3] * point.x + self.m[7] * point.y + self.m[11] * point.z + self.m[15];

        if w != 0.0 && w != 1.0 {
            Vec3::new(x / w, y / w, z / w)
        } else {
            Vec3::new(x, y, z)
        }
    }

    pub fn transform_vector(&self, vector: Vec3) -> Vec3 {
        Vec3::new(
            self.m[0] * vector.x + self.m[4] * vector.y + self.m[8] * vector.z,
            self.m[1] * vector.x + self.m[5] * vector.y + self.m[9] * vector.z,
            self.m[2] * vector.x + self.m[6] * vector.y + self.m[10] * vector.z,
        )
    }
}

impl std::ops::Mul for Mat4 {
    type Output = Self;
    fn mul(self, other: Self) -> Self {
        let mut result = [0.0f32; 16];
        for i in 0..4 {
            for j in 0..4 {
                for k in 0..4 {
                    result[i * 4 + j] += self.m[i * 4 + k] * other.m[k * 4 + j];
                }
            }
        }
        Self { m: result }
    }
}

impl std::ops::Mul<Vec4> for Mat4 {
    type Output = Vec4;
    fn mul(self, v: Vec4) -> Vec4 {
        Vec4::new(
            self.m[0] * v.x + self.m[4] * v.y + self.m[8] * v.z + self.m[12] * v.w,
            self.m[1] * v.x + self.m[5] * v.y + self.m[9] * v.z + self.m[13] * v.w,
            self.m[2] * v.x + self.m[6] * v.y + self.m[10] * v.z + self.m[14] * v.w,
            self.m[3] * v.x + self.m[7] * v.y + self.m[11] * v.z + self.m[15] * v.w,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mat3 {
    pub m: [f32; 9],
}

impl Mat3 {
    pub const fn identity() -> Self {
        Self {
            m: [
                1.0, 0.0, 0.0,
                0.0, 1.0, 0.0,
                0.0, 0.0, 1.0,
            ],
        }
    }
}
