use crate::vec::Vec3;
use crate::quat::Quat;
use crate::mat::Mat4;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    pub position: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Transform {
    pub const fn new(position: Vec3, rotation: Quat, scale: Vec3) -> Self {
        Self {
            position,
            rotation,
            scale,
        }
    }

    pub const fn identity() -> Self {
        Self {
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }

    pub fn to_mat4(&self) -> Mat4 {
        let mut mat = Mat4::identity();
        mat = mat.translate(self.position);
        mat = mat.rotate(self.rotation);
        mat = mat.scale(self.scale);
        mat
    }

    /// Composes two transforms: `self` applied first, then `other`.
    ///
    /// The resulting scale accumulates multiplicatively; the rotation is the
    /// concatenation of the two rotations; the translation is carried through
    /// the second transform's rotation and scale.
    pub fn compose(&self, child: &Transform) -> Transform {
        let rotated_position = self.rotation.rotate_vector(Vec3::new(
            child.position.x * self.scale.x,
            child.position.y * self.scale.y,
            child.position.z * self.scale.z,
        ));
        Transform {
            position: Vec3::new(
                self.position.x + rotated_position.x,
                self.position.y + rotated_position.y,
                self.position.z + rotated_position.z,
            ),
            rotation: (child.rotation * self.rotation).normalize(),
            scale: Vec3::new(
                self.scale.x * child.scale.x,
                self.scale.y * child.scale.y,
                self.scale.z * child.scale.z,
            ),
        }
    }

    /// The inverse transform, such that `t.compose(&t.inverse())` is the
    /// identity for any transform with non-zero scale.
    pub fn inverse(&self) -> Transform {
        let inv_rot = self.rotation.conjugate();
        let inv_scale = Vec3::new(
            if self.scale.x.abs() > 1e-12 { 1.0 / self.scale.x } else { 0.0 },
            if self.scale.y.abs() > 1e-12 { 1.0 / self.scale.y } else { 0.0 },
            if self.scale.z.abs() > 1e-12 { 1.0 / self.scale.z } else { 0.0 },
        );
        let back = inv_rot.rotate_vector(Vec3::new(
            -self.position.x,
            -self.position.y,
            -self.position.z,
        ));
        Transform {
            position: Vec3::new(
                back.x * inv_scale.x,
                back.y * inv_scale.y,
                back.z * inv_scale.z,
            ),
            rotation: inv_rot,
            scale: inv_scale,
        }
    }

    pub fn forward(&self) -> Vec3 {
        self.rotation.rotate_vector(Vec3::FORWARD)
    }

    pub fn right(&self) -> Vec3 {
        self.rotation.rotate_vector(Vec3::RIGHT)
    }

    pub fn up(&self) -> Vec3 {
        self.rotation.rotate_vector(Vec3::UP)
    }

    pub fn translate(&mut self, translation: Vec3) {
        self.position = self.position + translation;
    }

    pub fn rotate(&mut self, rotation: Quat) {
        self.rotation = (rotation * self.rotation).normalize();
    }

    pub fn scale_by(&mut self, scale: Vec3) {
        self.scale = Vec3::new(
            self.scale.x * scale.x,
            self.scale.y * scale.y,
            self.scale.z * scale.z,
        );
    }

    pub fn lerp(&self, other: &Self, t: f32) -> Self {
        Self {
            position: self.position.lerp(&other.position, t),
            rotation: self.rotation.slerp(&other.rotation, t),
            scale: Vec3::new(
                self.scale.x + (other.scale.x - self.scale.x) * t,
                self.scale.y + (other.scale.y - self.scale.y) * t,
                self.scale.z + (other.scale.z - self.scale.z) * t,
            ),
        }
    }
}

impl std::ops::Mul<Transform> for Transform {
    type Output = Transform;
    /// Composes two transforms: `self` is the parent, `child` is applied
    /// inside it. This is the operation forward kinematics performs.
    fn mul(self, child: Transform) -> Transform {
        self.compose(&child)
    }
}
