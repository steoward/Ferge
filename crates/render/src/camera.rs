use ow_math::{Vec3, Mat4};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CameraType {
    Perspective,
    Orthographic,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl Viewport {
    pub fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self { x, y, width, height }
    }

    pub fn aspect_ratio(&self) -> f32 {
        self.width as f32 / self.height as f32
    }
}

#[derive(Debug, Clone)]
pub struct Camera {
    pub camera_type: CameraType,
    pub position: Vec3,
    pub rotation: ow_math::Quat,
    pub fov_y: f32,
    pub near: f32,
    pub far: f32,
    pub ortho_left: f32,
    pub ortho_right: f32,
    pub ortho_bottom: f32,
    pub ortho_top: f32,
    pub viewport: Viewport,
    pub view_matrix: Mat4,
    pub projection_matrix: Mat4,
    pub view_projection_matrix: Mat4,
}

impl Camera {
    pub fn new_perspective(position: Vec3, fov_y: f32, near: f32, far: f32) -> Self {
        Self {
            camera_type: CameraType::Perspective,
            position,
            rotation: ow_math::Quat::IDENTITY,
            fov_y,
            near,
            far,
            ortho_left: -1.0,
            ortho_right: 1.0,
            ortho_bottom: -1.0,
            ortho_top: 1.0,
            viewport: Viewport::new(0, 0, 1920, 1080),
            view_matrix: Mat4::identity(),
            projection_matrix: Mat4::identity(),
            view_projection_matrix: Mat4::identity(),
        }
    }

    pub fn new_orthographic(
        position: Vec3,
        left: f32,
        right: f32,
        bottom: f32,
        top: f32,
        near: f32,
        far: f32,
    ) -> Self {
        Self {
            camera_type: CameraType::Orthographic,
            position,
            rotation: ow_math::Quat::IDENTITY,
            fov_y: 0.0,
            near,
            far,
            ortho_left: left,
            ortho_right: right,
            ortho_bottom: bottom,
            ortho_top: top,
            viewport: Viewport::new(0, 0, 1920, 1080),
            view_matrix: Mat4::identity(),
            projection_matrix: Mat4::identity(),
            view_projection_matrix: Mat4::identity(),
        }
    }

    pub fn update(&mut self) {
        self.update_view_matrix();
        self.update_projection_matrix();
        self.view_projection_matrix = self.projection_matrix * self.view_matrix;
    }

    fn update_view_matrix(&mut self) {
        let target = self.position + self.rotation.rotate_vector(Vec3::FORWARD);
        let up = self.rotation.rotate_vector(Vec3::UP);
        self.view_matrix = Mat4::look_at(self.position, target, up);
    }

    fn update_projection_matrix(&mut self) {
        match self.camera_type {
            CameraType::Perspective => {
                self.projection_matrix = Mat4::perspective(
                    self.fov_y,
                    self.viewport.aspect_ratio(),
                    self.near,
                    self.far,
                );
            }
            CameraType::Orthographic => {
                self.projection_matrix = Mat4::orthographic(
                    self.ortho_left,
                    self.ortho_right,
                    self.ortho_bottom,
                    self.ortho_top,
                    self.near,
                    self.far,
                );
            }
        }
    }

    pub fn set_viewport(&mut self, viewport: Viewport) {
        self.viewport = viewport;
    }

    pub fn screen_to_world(&self, screen_pos: Vec2, depth: f32) -> Vec3 {
        // Simplified unprojection
        let ndc_x = (screen_pos.x / self.viewport.width as f32) * 2.0 - 1.0;
        let ndc_y = 1.0 - (screen_pos.y / self.viewport.height as f32) * 2.0;

        let inv_vp = self.view_projection_matrix;
        
        inv_vp.transform_point(Vec3::new(ndc_x, ndc_y, depth))
    }

    pub fn world_to_screen(&self, world_pos: Vec3) -> Vec2 {
        let clip = self.view_projection_matrix * ow_math::Vec4::new(world_pos.x, world_pos.y, world_pos.z, 1.0);
        let ndc = clip.xyz() / clip.w;
        let screen_x = (ndc.x + 1.0) * 0.5 * self.viewport.width as f32;
        let screen_y = (1.0 - ndc.y) * 0.5 * self.viewport.height as f32;
        Vec2::new(screen_x, screen_y)
    }
}

use ow_math::Vec2;
