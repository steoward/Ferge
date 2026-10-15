use crate::vec::Vec3;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quat {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Quat {
    pub const fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { x, y, z, w }
    }

    pub const IDENTITY: Self = Self::new(0.0, 0.0, 0.0, 1.0);

    pub fn from_axis_angle(axis: Vec3, angle: f32) -> Self {
        let half_angle = angle * 0.5;
        let s = half_angle.sin();
        let axis = axis.normalize();
        Self::new(axis.x * s, axis.y * s, axis.z * s, half_angle.cos())
    }

    pub fn from_euler(roll: f32, pitch: f32, yaw: f32) -> Self {
        let cy = (yaw * 0.5).cos();
        let sy = (yaw * 0.5).sin();
        let cp = (pitch * 0.5).cos();
        let sp = (pitch * 0.5).sin();
        let cr = (roll * 0.5).cos();
        let sr = (roll * 0.5).sin();

        Self::new(
            sr * cp * cy - cr * sp * sy,
            cr * sp * cy + sr * cp * sy,
            cr * cp * sy - sr * sp * cy,
            cr * cp * cy + sr * sp * sy,
        )
    }

    pub fn length(&self) -> f32 {
        (self.x * self.x + self.y * self.y + self.z * self.z + self.w * self.w).sqrt()
    }

    pub fn normalize(&self) -> Self {
        let len = self.length();
        if len > 0.0 {
            Quat::new(self.x / len, self.y / len, self.z / len, self.w / len)
        } else {
            *self
        }
    }

    pub fn conjugate(&self) -> Self {
        Self::new(-self.x, -self.y, -self.z, self.w)
    }

    pub fn dot(&self, other: &Self) -> f32 {
        self.x * other.x + self.y * other.y + self.z * other.z + self.w * other.w
    }

    pub fn slerp(&self, other: &Self, t: f32) -> Self {
        let mut other = *other;
        let mut dot = self.dot(&other);

        // Take the short arc: if the quaternions point in opposite hemispheres
        // the direct path is the long way round.
        if dot < 0.0 {
            other = other * -1.0;
            dot = -dot;
        }

        // Below this threshold the quaternions are nearly identical and linear
        // interpolation is both cheaper and numerically better behaved.
        if dot > 0.9995 {
            return (*self + (other - *self) * t).normalize();
        }

        let theta_0 = dot.acos();
        let theta = theta_0 * t;

        // The unambiguous form of the two weights: sin((1-t)θ) and sin(tθ)
        // both over sin(θ). Written this way the result is guaranteed to lie on
        // the unit sphere and to interpolate the arc rather than the chord.
        let sin_0 = (theta_0 - theta).sin();
        let sin_1 = theta.sin();
        let sin_denom = theta_0.sin();

        if sin_denom.abs() < 1e-8 {
            return (*self + (other - *self) * t).normalize();
        }

        let s0 = sin_0 / sin_denom;
        let s1 = sin_1 / sin_denom;

        (*self * s0 + other * s1).normalize()
    }

    pub fn to_mat3(&self) -> [f32; 9] {
        let x2 = self.x * self.x;
        let y2 = self.y * self.y;
        let z2 = self.z * self.z;
        let xy = self.x * self.y;
        let xz = self.x * self.z;
        let yz = self.y * self.z;
        let wx = self.w * self.x;
        let wy = self.w * self.y;
        let wz = self.w * self.z;

        [
            1.0 - 2.0 * (y2 + z2), 2.0 * (xy - wz), 2.0 * (xz + wy),
            2.0 * (xy + wz), 1.0 - 2.0 * (x2 + z2), 2.0 * (yz - wx),
            2.0 * (xz - wy), 2.0 * (yz + wx), 1.0 - 2.0 * (x2 + y2),
        ]
    }

    pub fn rotate_vector(&self, v: Vec3) -> Vec3 {
        let qv = Vec3::new(self.x, self.y, self.z);
        let uv = qv.cross(v);
        let uuv = qv.cross(uv);
        v + (uv * self.w + uuv) * 2.0
    }
}

impl Default for Quat {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl std::ops::Mul for Quat {
    type Output = Self;
    fn mul(self, other: Self) -> Self {
        Self::new(
            self.w * other.x + self.x * other.w + self.y * other.z - self.z * other.y,
            self.w * other.y - self.x * other.z + self.y * other.w + self.z * other.x,
            self.w * other.z + self.x * other.y - self.y * other.x + self.z * other.w,
            self.w * other.w - self.x * other.x - self.y * other.y - self.z * other.z,
        )
    }
}

impl std::ops::Mul<f32> for Quat {
    type Output = Self;
    fn mul(self, scalar: f32) -> Self {
        Self::new(
            self.x * scalar,
            self.y * scalar,
            self.z * scalar,
            self.w * scalar,
        )
    }
}

impl std::ops::Add for Quat {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self::new(
            self.x + other.x,
            self.y + other.y,
            self.z + other.z,
            self.w + other.w,
        )
    }
}

impl std::ops::Sub for Quat {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        Self::new(
            self.x - other.x,
            self.y - other.y,
            self.z - other.z,
            self.w - other.w,
        )
    }
}
