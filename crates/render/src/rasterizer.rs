use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use ow_math::{Mat4, Vec2, Vec3, Vec4};
use parking_lot::RwLock;

use crate::camera::Camera;
use crate::light::LightType;
use crate::mesh::Vertex;
use crate::pass::{RenderPass, RenderPassType};
use crate::renderer::Renderer;

/// Depth test comparison function.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepthTest {
    Less,
    LessEqual,
    Always,
    None,
}

/// Software Framebuffer managing 32-bit RGBA color and 32-bit floating-point depth buffer.
/// Color buffer format is 0xAARRGGBB (Alpha: bits 24..31, Red: bits 16..23, Green: bits 8..15, Blue: bits 0..7).
#[derive(Debug, Clone)]
pub struct FrameBuffer {
    pub width: u32,
    pub height: u32,
    pub color_buffer: Vec<u32>,
    pub depth_buffer: Vec<f32>,
    pub depth_test: DepthTest,
}

impl FrameBuffer {
    /// Creates a new FrameBuffer of the specified dimensions.
    pub fn new(width: u32, height: u32) -> Self {
        let size = (width as usize) * (height as usize);
        Self {
            width,
            height,
            color_buffer: vec![0; size],
            depth_buffer: vec![1.0; size],
            depth_test: DepthTest::LessEqual,
        }
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn set_depth_test(&mut self, test: DepthTest) {
        self.depth_test = test;
    }

    pub fn depth_test(&self) -> DepthTest {
        self.depth_test
    }

    /// Clears both color buffer and depth buffer to given values.
    pub fn clear(&mut self, color: u32, depth: f32) {
        self.color_buffer.fill(color);
        self.depth_buffer.fill(depth);
    }

    /// Retrieves color of pixel at (x, y). Returns 0 if out of bounds.
    #[inline]
    pub fn get_pixel(&self, x: u32, y: u32) -> u32 {
        if x < self.width && y < self.height {
            self.color_buffer[(y * self.width + x) as usize]
        } else {
            0
        }
    }

    /// Retrieves depth of pixel at (x, y). Returns infinity if out of bounds.
    #[inline]
    pub fn get_depth(&self, x: u32, y: u32) -> f32 {
        if x < self.width && y < self.height {
            self.depth_buffer[(y * self.width + x) as usize]
        } else {
            f32::INFINITY
        }
    }

    /// Evaluates if a given depth passes the current depth test.
    #[inline]
    pub fn test_depth(&self, x: u32, y: u32, depth: f32) -> bool {
        if x >= self.width || y >= self.height {
            return false;
        }
        let current_depth = self.depth_buffer[(y * self.width + x) as usize];
        match self.depth_test {
            DepthTest::Less => depth < current_depth,
            DepthTest::LessEqual => depth <= current_depth,
            DepthTest::Always => true,
            DepthTest::None => true,
        }
    }

    /// Sets pixel at (x, y) with depth testing. Returns true if pixel passed and was written.
    #[inline]
    pub fn set_pixel(&mut self, x: u32, y: u32, color: u32, depth: f32) -> bool {
        if x >= self.width || y >= self.height {
            return false;
        }
        let idx = (y * self.width + x) as usize;
        let current_depth = self.depth_buffer[idx];
        let passes = match self.depth_test {
            DepthTest::Less => depth < current_depth,
            DepthTest::LessEqual => depth <= current_depth,
            DepthTest::Always => true,
            DepthTest::None => true,
        };
        if passes {
            if self.depth_test != DepthTest::None {
                self.depth_buffer[idx] = depth;
            }
            self.color_buffer[idx] = color;
            true
        } else {
            false
        }
    }

    /// Sets pixel directly without checking or modifying depth buffer.
    #[inline]
    pub fn set_pixel_no_depth(&mut self, x: u32, y: u32, color: u32) {
        if x < self.width && y < self.height {
            let idx = (y * self.width + x) as usize;
            self.color_buffer[idx] = color;
        }
    }

    /// Exports framebuffer contents to a standard binary PPM (P6) file.
    pub fn save_ppm(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);
        write!(writer, "P6\n{} {}\n255\n", self.width, self.height)?;

        let mut rgb_bytes = Vec::with_capacity((self.width * self.height * 3) as usize);
        for &pixel in &self.color_buffer {
            let (r, g, b, _a) = u32_to_rgba(pixel);
            rgb_bytes.push(r);
            rgb_bytes.push(g);
            rgb_bytes.push(b);
        }
        writer.write_all(&rgb_bytes)?;
        writer.flush()?;
        Ok(())
    }

    /// Exports framebuffer contents to a standard uncompressed 24-bit BMP file.
    pub fn save_bmp(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);

        let width = self.width as usize;
        let height = self.height as usize;
        let row_stride = (width * 3 + 3) & !3; // Pad row to 4-byte boundary
        let padding_bytes = row_stride - width * 3;
        let image_size = (row_stride * height) as u32;
        let file_size = 54 + image_size;

        // BITMAPFILEHEADER (14 bytes)
        writer.write_all(b"BM")?;
        writer.write_all(&file_size.to_le_bytes())?;
        writer.write_all(&0u16.to_le_bytes())?; // reserved 1
        writer.write_all(&0u16.to_le_bytes())?; // reserved 2
        writer.write_all(&54u32.to_le_bytes())?; // offset to pixel bits

        // BITMAPINFOHEADER (40 bytes)
        writer.write_all(&40u32.to_le_bytes())?; // biSize
        writer.write_all(&(self.width as i32).to_le_bytes())?; // biWidth
        writer.write_all(&(self.height as i32).to_le_bytes())?; // biHeight (positive = bottom-up)
        writer.write_all(&1u16.to_le_bytes())?; // biPlanes
        writer.write_all(&24u16.to_le_bytes())?; // biBitCount
        writer.write_all(&0u32.to_le_bytes())?; // biCompression (BI_RGB)
        writer.write_all(&image_size.to_le_bytes())?; // biSizeImage
        writer.write_all(&2835i32.to_le_bytes())?; // biXPelsPerMeter (72 DPI)
        writer.write_all(&2835i32.to_le_bytes())?; // biYPelsPerMeter
        writer.write_all(&0u32.to_le_bytes())?; // biClrUsed
        writer.write_all(&0u32.to_le_bytes())?; // biClrImportant

        // Pixel data: bottom-to-top rows, B, G, R order
        let padding = [0u8; 3];
        for y in (0..height).rev() {
            let row_start = y * width;
            for x in 0..width {
                let pixel = self.color_buffer[row_start + x];
                let (r, g, b, _a) = u32_to_rgba(pixel);
                writer.write_all(&[b, g, r])?;
            }
            if padding_bytes > 0 {
                writer.write_all(&padding[..padding_bytes])?;
            }
        }
        writer.flush()?;
        Ok(())
    }
}

/// Packs 8-bit RGBA channels into 0xAARRGGBB format.
#[inline]
pub fn rgba_to_u32(r: u8, g: u8, b: u8, a: u8) -> u32 {
    ((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

/// Unpacks 0xAARRGGBB integer into 8-bit RGBA channels.
#[inline]
pub fn u32_to_rgba(c: u32) -> (u8, u8, u8, u8) {
    let a = ((c >> 24) & 0xFF) as u8;
    let r = ((c >> 16) & 0xFF) as u8;
    let g = ((c >> 8) & 0xFF) as u8;
    let b = (c & 0xFF) as u8;
    (r, g, b, a)
}

/// Converts float Vec4 [0.0..1.0] to packed 0xAARRGGBB format.
#[inline]
pub fn vec4_to_u32(v: Vec4) -> u32 {
    let r = (v.x.clamp(0.0, 1.0) * 255.0).round() as u8;
    let g = (v.y.clamp(0.0, 1.0) * 255.0).round() as u8;
    let b = (v.z.clamp(0.0, 1.0) * 255.0).round() as u8;
    let a = (v.w.clamp(0.0, 1.0) * 255.0).round() as u8;
    rgba_to_u32(r, g, b, a)
}

/// Converts packed 0xAARRGGBB format to float Vec4 [0.0..1.0].
#[inline]
pub fn u32_to_vec4(c: u32) -> Vec4 {
    let (r, g, b, a) = u32_to_rgba(c);
    Vec4::new(
        r as f32 / 255.0,
        g as f32 / 255.0,
        b as f32 / 255.0,
        a as f32 / 255.0,
    )
}

/// Shading models supported by the rasterizer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShadingModel {
    /// Face normal computed per triangle.
    Flat,
    /// Lighting computed per vertex and interpolated across triangle.
    Gouraud,
    /// Normals interpolated across triangle and lit per fragment.
    Phong,
    /// Wireframe edge rasterization using Bresenham algorithm.
    Wireframe,
}

/// Face culling mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CullMode {
    None,
    Back,
    Front,
}

/// Rasterizer run-time statistics.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RasterizerStats {
    pub triangles_rendered: u64,
    pub fragments_processed: u64,
    pub fragments_passed: u64,
}

/// High-performance 3D software rasterizer.
#[derive(Debug, Clone)]
pub struct Rasterizer {
    pub shading_model: ShadingModel,
    pub cull_mode: CullMode,
    pub wireframe: bool,
    pub wireframe_color: u32,
    pub ambient_intensity: f32,
    pub specular_power: f32,
    pub specular_intensity: f32,
    pub stats: RasterizerStats,
}

impl Default for Rasterizer {
    fn default() -> Self {
        Self::new()
    }
}

impl Rasterizer {
    /// Creates a new default Rasterizer instance.
    pub fn new() -> Self {
        Self {
            shading_model: ShadingModel::Phong,
            cull_mode: CullMode::None,
            wireframe: false,
            wireframe_color: rgba_to_u32(0, 255, 0, 255),
            ambient_intensity: 0.2,
            specular_power: 32.0,
            specular_intensity: 0.5,
            stats: RasterizerStats::default(),
        }
    }

    pub fn with_shading(mut self, model: ShadingModel) -> Self {
        self.shading_model = model;
        self
    }

    pub fn with_cull_mode(mut self, cull: CullMode) -> Self {
        self.cull_mode = cull;
        self
    }

    pub fn with_wireframe(mut self, wireframe: bool, color: u32) -> Self {
        self.wireframe = wireframe;
        self.wireframe_color = color;
        self
    }

    pub fn reset_stats(&mut self) {
        self.stats = RasterizerStats::default();
    }

    pub fn stats(&self) -> RasterizerStats {
        self.stats
    }

    /// Rasterizes a 3D mesh into the framebuffer.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_mesh(
        &mut self,
        fb: &mut FrameBuffer,
        vertices: &[Vertex],
        indices: &[u32],
        model_matrix: &Mat4,
        view_matrix: &Mat4,
        proj_matrix: &Mat4,
        light_dir: Vec3,
        light_color: Vec3,
    ) {
        if vertices.is_empty() {
            return;
        }

        let cam_pos = extract_camera_position(view_matrix);

        // Pre-transform all vertices to world and clip space
        let transformed: Vec<TransformedVertex> = vertices
            .iter()
            .map(|v| {
                let world_pos = model_matrix.transform_point(v.position);
                let world_normal = model_matrix.transform_vector(v.normal).normalize();
                let view_pos = view_matrix.transform_point(world_pos);
                let clip_pos = *proj_matrix * Vec4::new(view_pos.x, view_pos.y, view_pos.z, 1.0);
                TransformedVertex {
                    clip_pos,
                    world_pos,
                    world_normal,
                    uv: v.uv0,
                    color: v.color,
                }
            })
            .collect();

        if indices.is_empty() {
            for chunk in transformed.chunks_exact(3) {
                self.process_triangle(fb, &chunk[0], &chunk[1], &chunk[2], light_dir, light_color, cam_pos);
            }
        } else {
            for chunk in indices.chunks_exact(3) {
                let (i0, i1, i2) = (chunk[0] as usize, chunk[1] as usize, chunk[2] as usize);
                if i0 < transformed.len() && i1 < transformed.len() && i2 < transformed.len() {
                    self.process_triangle(
                        fb,
                        &transformed[i0],
                        &transformed[i1],
                        &transformed[i2],
                        light_dir,
                        light_color,
                        cam_pos,
                    );
                }
            }
        }
    }

    /// Performs near-plane clipping against w >= epsilon and rasterizes resulting triangles.
    #[allow(clippy::too_many_arguments)]
    fn process_triangle(
        &mut self,
        fb: &mut FrameBuffer,
        v0: &TransformedVertex,
        v1: &TransformedVertex,
        v2: &TransformedVertex,
        light_dir: Vec3,
        light_color: Vec3,
        cam_pos: Vec3,
    ) {
        let near_eps = 0.001f32;
        let in0 = v0.clip_pos.w >= near_eps;
        let in1 = v1.clip_pos.w >= near_eps;
        let in2 = v2.clip_pos.w >= near_eps;
        let in_count = (in0 as u8) + (in1 as u8) + (in2 as u8);

        match in_count {
            0 => {
                // Completely behind near plane: cull
            }
            3 => {
                self.rasterize_triangle(fb, v0, v1, v2, light_dir, light_color, cam_pos);
            }
            1 => {
                // 1 vertex in front, 2 behind: produces 1 clipped triangle
                let (v_in, v_out1, v_out2) = if in0 {
                    (v0, v1, v2)
                } else if in1 {
                    (v1, v2, v0)
                } else {
                    (v2, v0, v1)
                };
                let t1 = (near_eps - v_in.clip_pos.w) / (v_out1.clip_pos.w - v_in.clip_pos.w);
                let t2 = (near_eps - v_in.clip_pos.w) / (v_out2.clip_pos.w - v_in.clip_pos.w);
                let new_v1 = lerp_transformed(v_in, v_out1, t1);
                let new_v2 = lerp_transformed(v_in, v_out2, t2);
                self.rasterize_triangle(fb, v_in, &new_v1, &new_v2, light_dir, light_color, cam_pos);
            }
            2 => {
                // 2 vertices in front, 1 behind: produces 2 clipped triangles (quad)
                let (v_in1, v_in2, v_out) = if !in0 {
                    (v1, v2, v0)
                } else if !in1 {
                    (v2, v0, v1)
                } else {
                    (v0, v1, v2)
                };
                let t1 = (near_eps - v_in1.clip_pos.w) / (v_out.clip_pos.w - v_in1.clip_pos.w);
                let t2 = (near_eps - v_in2.clip_pos.w) / (v_out.clip_pos.w - v_in2.clip_pos.w);
                let new_v1 = lerp_transformed(v_in1, v_out, t1);
                let new_v2 = lerp_transformed(v_in2, v_out, t2);
                self.rasterize_triangle(fb, v_in1, v_in2, &new_v2, light_dir, light_color, cam_pos);
                self.rasterize_triangle(fb, v_in1, &new_v2, &new_v1, light_dir, light_color, cam_pos);
            }
            _ => unreachable!(),
        }
    }

    /// Converts transformed clip vertex to screen vertex.
    fn to_screen(&self, tv: &TransformedVertex, fb_width: f32, fb_height: f32) -> ScreenVertex {
        let inv_w = 1.0 / tv.clip_pos.w;
        let ndc_x = tv.clip_pos.x * inv_w;
        let ndc_y = tv.clip_pos.y * inv_w;
        let ndc_z = tv.clip_pos.z * inv_w;

        let sx = (ndc_x + 1.0) * 0.5 * fb_width;
        let sy = (1.0 - ndc_y) * 0.5 * fb_height;
        let sz = (ndc_z + 1.0) * 0.5; // Map [-1.0, 1.0] to [0.0, 1.0]

        ScreenVertex {
            screen: Vec3::new(sx, sy, sz),
            inv_w,
            world_pos: tv.world_pos,
            world_normal: tv.world_normal,
            uv: tv.uv,
            color: tv.color,
        }
    }

    /// Triangle rasterization with sub-pixel barycentric edge functions and perspective-correct interpolation.
    #[allow(clippy::too_many_arguments)]
    fn rasterize_triangle(
        &mut self,
        fb: &mut FrameBuffer,
        v0: &TransformedVertex,
        v1: &TransformedVertex,
        v2: &TransformedVertex,
        light_dir: Vec3,
        light_color: Vec3,
        cam_pos: Vec3,
    ) {
        let sv0 = self.to_screen(v0, fb.width as f32, fb.height as f32);
        let mut sv1 = self.to_screen(v1, fb.width as f32, fb.height as f32);
        let mut sv2 = self.to_screen(v2, fb.width as f32, fb.height as f32);

        let mut area = (sv1.screen.x - sv0.screen.x) * (sv2.screen.y - sv0.screen.y)
            - (sv1.screen.y - sv0.screen.y) * (sv2.screen.x - sv0.screen.x);

        if area.abs() < 1e-6 {
            return;
        }

        // Handle culling modes
        match self.cull_mode {
            CullMode::Back => {
                if area <= 0.0 {
                    return;
                }
            }
            CullMode::Front => {
                if area >= 0.0 {
                    return;
                }
                std::mem::swap(&mut sv1, &mut sv2);
                area = -area;
            }
            CullMode::None => {
                if area < 0.0 {
                    std::mem::swap(&mut sv1, &mut sv2);
                    area = -area;
                }
            }
        }

        let is_wireframe_only = self.shading_model == ShadingModel::Wireframe;
        if self.wireframe || is_wireframe_only {
            self.draw_wireframe_triangle(fb, &sv0, &sv1, &sv2);
            if is_wireframe_only {
                self.stats.triangles_rendered += 1;
                return;
            }
        }

        // Screen-space bounding box
        let min_x = (sv0.screen.x.min(sv1.screen.x).min(sv2.screen.x).floor() as i32)
            .max(0)
            .min(fb.width as i32 - 1) as u32;
        let max_x = (sv0.screen.x.max(sv1.screen.x).max(sv2.screen.x).ceil() as i32)
            .max(0)
            .min(fb.width as i32 - 1) as u32;
        let min_y = (sv0.screen.y.min(sv1.screen.y).min(sv2.screen.y).floor() as i32)
            .max(0)
            .min(fb.height as i32 - 1) as u32;
        let max_y = (sv0.screen.y.max(sv1.screen.y).max(sv2.screen.y).ceil() as i32)
            .max(0)
            .min(fb.height as i32 - 1) as u32;

        if min_x > max_x || min_y > max_y {
            return;
        }

        let inv_area = 1.0 / area;
        let dx12 = sv2.screen.x - sv1.screen.x;
        let dy12 = sv2.screen.y - sv1.screen.y;
        let dx20 = sv0.screen.x - sv2.screen.x;
        let dy20 = sv0.screen.y - sv2.screen.y;
        let dx01 = sv1.screen.x - sv0.screen.x;
        let dy01 = sv1.screen.y - sv0.screen.y;

        // Precompute Gouraud vertex colors if requested
        let (color0, color1, color2) = if self.shading_model == ShadingModel::Gouraud {
            (
                self.shade_fragment(sv0.world_pos, sv0.world_normal, sv0.color, light_dir, light_color, cam_pos),
                self.shade_fragment(sv1.world_pos, sv1.world_normal, sv1.color, light_dir, light_color, cam_pos),
                self.shade_fragment(sv2.world_pos, sv2.world_normal, sv2.color, light_dir, light_color, cam_pos),
            )
        } else {
            (sv0.color, sv1.color, sv2.color)
        };

        // Precompute Flat shading color if requested
        let flat_color = if self.shading_model == ShadingModel::Flat {
            let edge1 = sv1.world_pos - sv0.world_pos;
            let edge2 = sv2.world_pos - sv0.world_pos;
            let face_normal = {
                let cross = edge1.cross(edge2);
                if cross.length_squared() > 1e-6 {
                    cross.normalize()
                } else {
                    sv0.world_normal
                }
            };
            let avg_pos = (sv0.world_pos + sv1.world_pos + sv2.world_pos) * (1.0 / 3.0);
            let avg_col = (sv0.color + sv1.color + sv2.color) * (1.0 / 3.0);
            self.shade_fragment(avg_pos, face_normal, avg_col, light_dir, light_color, cam_pos)
        } else {
            Vec4::ZERO
        };

        for y in min_y..=max_y {
            let py = y as f32 + 0.5;
            for x in min_x..=max_x {
                let px = x as f32 + 0.5;

                // Barycentric edge functions
                let w0 = dx12 * (py - sv1.screen.y) - dy12 * (px - sv1.screen.x);
                let w1 = dx20 * (py - sv2.screen.y) - dy20 * (px - sv2.screen.x);
                let w2 = dx01 * (py - sv0.screen.y) - dy01 * (px - sv0.screen.x);

                // Sub-pixel barycentric check (small negative epsilon prevents cracks)
                if w0 < -1e-5 || w1 < -1e-5 || w2 < -1e-5 {
                    continue;
                }

                self.stats.fragments_processed += 1;

                let lambda0 = w0 * inv_area;
                let lambda1 = w1 * inv_area;
                let lambda2 = w2 * inv_area;

                // Perspective-correct interpolation: 1/w
                let inv_w = lambda0 * sv0.inv_w + lambda1 * sv1.inv_w + lambda2 * sv2.inv_w;
                if inv_w <= 0.0 {
                    continue;
                }
                let w = 1.0 / inv_w;

                // Depth interpolation: (z / w) * w
                let z_over_w = lambda0 * (sv0.screen.z * sv0.inv_w)
                    + lambda1 * (sv1.screen.z * sv1.inv_w)
                    + lambda2 * (sv2.screen.z * sv2.inv_w);
                let depth = z_over_w * w;

                // Early depth rejection test
                if !fb.test_depth(x, y, depth) {
                    continue;
                }

                let final_color = match self.shading_model {
                    ShadingModel::Flat => flat_color,
                    ShadingModel::Gouraud => {
                        let c_over_w = color0 * (sv0.inv_w * lambda0)
                            + color1 * (sv1.inv_w * lambda1)
                            + color2 * (sv2.inv_w * lambda2);
                        c_over_w * w
                    }
                    ShadingModel::Phong => {
                        let n_over_w = sv0.world_normal * (sv0.inv_w * lambda0)
                            + sv1.world_normal * (sv1.inv_w * lambda1)
                            + sv2.world_normal * (sv2.inv_w * lambda2);
                        let normal = (n_over_w * w).normalize();

                        let p_over_w = sv0.world_pos * (sv0.inv_w * lambda0)
                            + sv1.world_pos * (sv1.inv_w * lambda1)
                            + sv2.world_pos * (sv2.inv_w * lambda2);
                        let world_pos = p_over_w * w;

                        let c_over_w = sv0.color * (sv0.inv_w * lambda0)
                            + sv1.color * (sv1.inv_w * lambda1)
                            + sv2.color * (sv2.inv_w * lambda2);
                        let base_color = c_over_w * w;

                        self.shade_fragment(world_pos, normal, base_color, light_dir, light_color, cam_pos)
                    }
                    ShadingModel::Wireframe => unreachable!(),
                };

                let pixel_color = vec4_to_u32(final_color);
                if fb.set_pixel(x, y, pixel_color, depth) {
                    self.stats.fragments_passed += 1;
                }
            }
        }

        self.stats.triangles_rendered += 1;
    }

    /// Evaluates Blinn-Phong lighting: ambient + diffuse + specular.
    fn shade_fragment(
        &self,
        world_pos: Vec3,
        normal: Vec3,
        base_color: Vec4,
        light_dir: Vec3,
        light_color: Vec3,
        cam_pos: Vec3,
    ) -> Vec4 {
        let n = if normal.length_squared() > 1e-6 {
            normal.normalize()
        } else {
            Vec3::UP
        };

        let l = if light_dir.length_squared() > 1e-6 {
            light_dir.normalize()
        } else {
            Vec3::new(0.5, 1.0, 0.5).normalize()
        };

        let v = {
            let diff = cam_pos - world_pos;
            if diff.length_squared() > 1e-6 {
                diff.normalize()
            } else {
                Vec3::new(0.0, 0.0, 1.0)
            }
        };

        // Ambient component
        let ambient = light_color * self.ambient_intensity;

        // Diffuse component: (N · L)
        let n_dot_l = n.dot(&l).max(0.0);
        let diffuse = light_color * (n_dot_l * (1.0 - self.ambient_intensity));

        // Specular component (Blinn-Phong): (N · H)^shininess
        let mut specular = Vec3::ZERO;
        if n_dot_l > 0.0 {
            let h = (l + v).normalize();
            let n_dot_h = n.dot(&h).max(0.0);
            let spec_factor = n_dot_h.powf(self.specular_power) * self.specular_intensity;
            specular = light_color * spec_factor;
        }

        let surf_rgb = base_color.xyz();
        let r = (ambient.x + diffuse.x) * surf_rgb.x + specular.x;
        let g = (ambient.y + diffuse.y) * surf_rgb.y + specular.y;
        let b = (ambient.z + diffuse.z) * surf_rgb.z + specular.z;

        Vec4::new(
            r.clamp(0.0, 1.0),
            g.clamp(0.0, 1.0),
            b.clamp(0.0, 1.0),
            base_color.w.clamp(0.0, 1.0),
        )
    }

    /// Draws wireframe edges of a triangle using Bresenham line algorithm.
    fn draw_wireframe_triangle(
        &mut self,
        fb: &mut FrameBuffer,
        sv0: &ScreenVertex,
        sv1: &ScreenVertex,
        sv2: &ScreenVertex,
    ) {
        let p0 = (sv0.screen.x.round() as i32, sv0.screen.y.round() as i32, sv0.screen.z);
        let p1 = (sv1.screen.x.round() as i32, sv1.screen.y.round() as i32, sv1.screen.z);
        let p2 = (sv2.screen.x.round() as i32, sv2.screen.y.round() as i32, sv2.screen.z);

        draw_bresenham_line(fb, p0.0, p0.1, p0.2, p1.0, p1.1, p1.2, self.wireframe_color, &mut self.stats);
        draw_bresenham_line(fb, p1.0, p1.1, p1.2, p2.0, p2.1, p2.2, self.wireframe_color, &mut self.stats);
        draw_bresenham_line(fb, p2.0, p2.1, p2.2, p0.0, p0.1, p0.2, self.wireframe_color, &mut self.stats);
    }
}

/// Helper struct for vertex during geometry clipping and coordinate transformation.
#[derive(Debug, Clone, Copy)]
pub struct TransformedVertex {
    pub clip_pos: Vec4,
    pub world_pos: Vec3,
    pub world_normal: Vec3,
    pub uv: Vec2,
    pub color: Vec4,
}

/// Helper struct for vertex projected into screen coordinates.
#[derive(Debug, Clone, Copy)]
pub struct ScreenVertex {
    pub screen: Vec3,
    pub inv_w: f32,
    pub world_pos: Vec3,
    pub world_normal: Vec3,
    pub uv: Vec2,
    pub color: Vec4,
}

/// Linearly interpolates two TransformedVertices during near plane clipping.
fn lerp_transformed(a: &TransformedVertex, b: &TransformedVertex, t: f32) -> TransformedVertex {
    TransformedVertex {
        clip_pos: a.clip_pos + (b.clip_pos - a.clip_pos) * t,
        world_pos: a.world_pos.lerp(&b.world_pos, t),
        world_normal: a.world_normal.lerp(&b.world_normal, t).normalize(),
        uv: a.uv + (b.uv - a.uv) * t,
        color: a.color + (b.color - a.color) * t,
    }
}

/// Extracts camera eye position in world space from view matrix.
pub fn extract_camera_position(view_matrix: &Mat4) -> Vec3 {
    let m = &view_matrix.m;
    Vec3::new(
        -(m[0] * m[12] + m[1] * m[13] + m[2] * m[14]),
        -(m[4] * m[12] + m[5] * m[13] + m[6] * m[14]),
        -(m[8] * m[12] + m[9] * m[13] + m[10] * m[14]),
    )
}

/// Bresenham line rasterization algorithm with depth interpolation.
#[allow(clippy::too_many_arguments)]
pub fn draw_bresenham_line(
    fb: &mut FrameBuffer,
    x0: i32,
    y0: i32,
    z0: f32,
    x1: i32,
    y1: i32,
    z1: f32,
    color: u32,
    stats: &mut RasterizerStats,
) {
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;

    let mut curr_x = x0;
    let mut curr_y = y0;
    let total_steps = dx.max(-dy);
    let mut step = 0;

    loop {
        let t = if total_steps > 0 { step as f32 / total_steps as f32 } else { 0.0 };
        let z = z0 + (z1 - z0) * t;

        if curr_x >= 0 && curr_x < fb.width as i32 && curr_y >= 0 && curr_y < fb.height as i32 {
            stats.fragments_processed += 1;
            // Bias depth slightly by -1e-4 so wireframe lines sit cleanly on top of triangle surfaces
            if fb.set_pixel(curr_x as u32, curr_y as u32, color, z - 0.0001) {
                stats.fragments_passed += 1;
            }
        }

        if curr_x == x1 && curr_y == y1 {
            break;
        }

        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            curr_x += sx;
        }
        if e2 <= dx {
            err += dx;
            curr_y += sy;
        }
        step += 1;
    }
}

/// Software rasterization render pass implementing `crate::pass::RenderPass`.
pub struct SoftwareRasterPass {
    enabled: bool,
    framebuffer: RwLock<FrameBuffer>,
    rasterizer: RwLock<Rasterizer>,
}

impl SoftwareRasterPass {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            enabled: true,
            framebuffer: RwLock::new(FrameBuffer::new(width, height)),
            rasterizer: RwLock::new(Rasterizer::new()),
        }
    }

    pub fn new_with(framebuffer: FrameBuffer, rasterizer: Rasterizer) -> Self {
        Self {
            enabled: true,
            framebuffer: RwLock::new(framebuffer),
            rasterizer: RwLock::new(rasterizer),
        }
    }

    pub fn framebuffer(&self) -> &RwLock<FrameBuffer> {
        &self.framebuffer
    }

    pub fn rasterizer(&self) -> &RwLock<Rasterizer> {
        &self.rasterizer
    }

    /// Exports the current frame to disk as an uncompressed 24-bit BMP image.
    pub fn export_frame_bmp(&self, path: &Path) -> std::io::Result<()> {
        self.framebuffer.read().save_bmp(path)
    }

    /// Exports the current frame to disk as a binary PPM image.
    pub fn export_frame_ppm(&self, path: &Path) -> std::io::Result<()> {
        self.framebuffer.read().save_ppm(path)
    }
}

impl RenderPass for SoftwareRasterPass {
    fn pass_type(&self) -> RenderPassType {
        RenderPassType::SoftwareRaster
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    fn execute(&self, renderer: &Renderer) {
        if !self.enabled {
            return;
        }

        // Retrieve camera or default
        let (view_matrix, proj_matrix) = if let Some(mut camera) = renderer.get_camera(0) {
            if camera.view_matrix == Mat4::identity() && camera.projection_matrix == Mat4::identity() {
                camera.update();
            }
            (camera.view_matrix, camera.projection_matrix)
        } else {
            let mut default_cam = Camera::new_perspective(
                Vec3::new(0.0, 0.0, -5.0),
                60.0f32.to_radians(),
                0.1,
                1000.0,
            );
            default_cam.update();
            (default_cam.view_matrix, default_cam.projection_matrix)
        };

        // Determine directional light
        let (light_dir, light_color) = {
            let lights = renderer.get_all_lights();
            let dir_light = lights.iter().find(|l| l.enabled && l.light_type == LightType::Directional);
            if let Some(light) = dir_light {
                (-light.direction, light.color * light.intensity)
            } else if let Some(light) = lights.first() {
                (-light.direction, light.color * light.intensity)
            } else {
                (Vec3::new(0.5, 1.0, 0.5).normalize(), Vec3::ONE)
            }
        };

        let clear_color = vec4_to_u32(renderer.get_config().clear_color);
        let mut fb = self.framebuffer.write();
        fb.clear(clear_color, 1.0);

        let mut rasterizer = self.rasterizer.write();
        for mesh in renderer.get_all_meshes() {
            rasterizer.draw_mesh(
                &mut fb,
                &mesh.vertex_buffer.vertices,
                &mesh.index_buffer.indices,
                &Mat4::identity(),
                &view_matrix,
                &proj_matrix,
                light_dir,
                light_color,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::Camera;
    use crate::light::Light;
    use crate::mesh::{Mesh, Vertex};
    use crate::renderer::{RenderConfig, Renderer};
    use std::fs::File;
    use std::io::Read;

    #[test]
    fn test_framebuffer_creation_clear_and_pixels() {
        let mut fb = FrameBuffer::new(64, 32);
        assert_eq!(fb.width(), 64);
        assert_eq!(fb.height(), 32);

        let clear_color = rgba_to_u32(12, 34, 56, 255);
        fb.clear(clear_color, 0.75);

        assert_eq!(fb.get_pixel(0, 0), clear_color);
        assert_eq!(fb.get_pixel(63, 31), clear_color);
        assert!((fb.get_depth(0, 0) - 0.75).abs() < 1e-6);
        assert!((fb.get_depth(63, 31) - 0.75).abs() < 1e-6);

        // Out of bounds check
        assert_eq!(fb.get_pixel(100, 100), 0);
        assert_eq!(fb.get_depth(100, 100), f32::INFINITY);
        assert!(!fb.set_pixel(100, 100, 0xFFFFFFFF, 0.1));
    }

    #[test]
    fn test_depth_testing_less_and_less_equal() {
        let mut fb = FrameBuffer::new(10, 10);
        fb.clear(0, 1.0);
        fb.set_depth_test(DepthTest::Less);

        let red = rgba_to_u32(255, 0, 0, 255);
        let blue = rgba_to_u32(0, 0, 255, 255);

        // Initial write at z = 0.5 passes
        assert!(fb.set_pixel(5, 5, red, 0.5));
        assert_eq!(fb.get_pixel(5, 5), red);
        assert!((fb.get_depth(5, 5) - 0.5).abs() < 1e-6);

        // Deeper pixel at z = 0.8 fails
        assert!(!fb.set_pixel(5, 5, blue, 0.8));
        assert_eq!(fb.get_pixel(5, 5), red);

        // Equal pixel at z = 0.5 fails with DepthTest::Less
        assert!(!fb.set_pixel(5, 5, blue, 0.5));
        assert_eq!(fb.get_pixel(5, 5), red);

        // Switch to LessEqual: equal depth passes
        fb.set_depth_test(DepthTest::LessEqual);
        assert!(fb.set_pixel(5, 5, blue, 0.5));
        assert_eq!(fb.get_pixel(5, 5), blue);

        // Closer pixel at z = 0.2 passes
        let green = rgba_to_u32(0, 255, 0, 255);
        assert!(fb.set_pixel(5, 5, green, 0.2));
        assert_eq!(fb.get_pixel(5, 5), green);
        assert!((fb.get_depth(5, 5) - 0.2).abs() < 1e-6);
    }

    #[test]
    fn test_save_ppm_and_save_bmp_format() {
        let dir = std::env::temp_dir().join("ow_test_rasterizer_export");
        let ppm_path = dir.join("test_frame.ppm");
        let bmp_path = dir.join("test_frame.bmp");

        let mut fb = FrameBuffer::new(8, 4);
        fb.clear(rgba_to_u32(0, 0, 0, 255), 1.0);

        // Draw RGB pattern
        fb.set_pixel_no_depth(0, 0, rgba_to_u32(255, 0, 0, 255)); // Red
        fb.set_pixel_no_depth(1, 0, rgba_to_u32(0, 255, 0, 255)); // Green
        fb.set_pixel_no_depth(2, 0, rgba_to_u32(0, 0, 255, 255)); // Blue

        // Save PPM
        fb.save_ppm(&ppm_path).expect("Failed to save PPM");
        assert!(ppm_path.exists());

        let mut ppm_bytes = Vec::new();
        File::open(&ppm_path)
            .unwrap()
            .read_to_end(&mut ppm_bytes)
            .unwrap();
        assert!(ppm_bytes.starts_with(b"P6\n8 4\n255\n"));

        // Save BMP
        fb.save_bmp(&bmp_path).expect("Failed to save BMP");
        assert!(bmp_path.exists());

        let mut bmp_bytes = Vec::new();
        File::open(&bmp_path)
            .unwrap()
            .read_to_end(&mut bmp_bytes)
            .unwrap();

        // Check BMP file header
        assert_eq!(&bmp_bytes[0..2], b"BM");
        let expected_stride = (8 * 3 + 3) & !3; // 24 bytes
        let expected_size = 54 + expected_stride * 4;
        let file_size = u32::from_le_bytes(bmp_bytes[2..6].try_into().unwrap());
        assert_eq!(file_size, expected_size as u32);
        let offset = u32::from_le_bytes(bmp_bytes[10..14].try_into().unwrap());
        assert_eq!(offset, 54);

        // Clean up
        let _ = std::fs::remove_file(ppm_path);
        let _ = std::fs::remove_file(bmp_path);
        let _ = std::fs::remove_dir(dir);
    }

    #[test]
    fn test_single_triangle_rasterization() {
        let mut fb = FrameBuffer::new(100, 100);
        fb.clear(rgba_to_u32(0, 0, 0, 255), 1.0);

        let mut rasterizer = Rasterizer::new();
        rasterizer.shading_model = ShadingModel::Flat;
        rasterizer.cull_mode = CullMode::None;

        let vertices = vec![
            Vertex::new(Vec3::new(0.0, 0.5, 0.0)).with_color(Vec4::new(1.0, 0.0, 0.0, 1.0)),
            Vertex::new(Vec3::new(-0.5, -0.5, 0.0)).with_color(Vec4::new(1.0, 0.0, 0.0, 1.0)),
            Vertex::new(Vec3::new(0.5, -0.5, 0.0)).with_color(Vec4::new(1.0, 0.0, 0.0, 1.0)),
        ];
        let indices = vec![0, 1, 2];

        // Orthographic projection mapping [-1, 1] to view
        let ortho = Mat4::orthographic(-1.0, 1.0, -1.0, 1.0, -1.0, 1.0);

        rasterizer.draw_mesh(
            &mut fb,
            &vertices,
            &indices,
            &Mat4::identity(),
            &Mat4::identity(),
            &ortho,
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::ONE,
        );

        assert_eq!(rasterizer.stats().triangles_rendered, 1);
        assert!(rasterizer.stats().fragments_processed > 0);
        assert!(rasterizer.stats().fragments_passed > 0);

        // Center pixel (50, 50) must be inside the triangle and shaded
        let center_pixel = fb.get_pixel(50, 50);
        assert_ne!(center_pixel, rgba_to_u32(0, 0, 0, 255));
        // Corner pixel (5, 5) must be outside the triangle
        let corner_pixel = fb.get_pixel(5, 5);
        assert_eq!(corner_pixel, rgba_to_u32(0, 0, 0, 255));
    }

    #[test]
    fn test_depth_buffering_triangle_occlusion() {
        let mut fb = FrameBuffer::new(100, 100);
        fb.clear(rgba_to_u32(0, 0, 0, 255), 1.0);

        let mut rasterizer = Rasterizer::new();
        rasterizer.shading_model = ShadingModel::Flat;
        rasterizer.cull_mode = CullMode::None;

        // Front triangle at Z = -0.2 (red)
        let front_vertices = vec![
            Vertex::new(Vec3::new(0.0, 0.5, -0.2)).with_color(Vec4::new(1.0, 0.0, 0.0, 1.0)),
            Vertex::new(Vec3::new(-0.5, -0.5, -0.2)).with_color(Vec4::new(1.0, 0.0, 0.0, 1.0)),
            Vertex::new(Vec3::new(0.5, -0.5, -0.2)).with_color(Vec4::new(1.0, 0.0, 0.0, 1.0)),
        ];

        // Back triangle at Z = -0.8 (blue)
        let back_vertices = vec![
            Vertex::new(Vec3::new(0.0, 0.5, -0.8)).with_color(Vec4::new(0.0, 0.0, 1.0, 1.0)),
            Vertex::new(Vec3::new(-0.5, -0.5, -0.8)).with_color(Vec4::new(0.0, 0.0, 1.0, 1.0)),
            Vertex::new(Vec3::new(0.5, -0.5, -0.8)).with_color(Vec4::new(0.0, 0.0, 1.0, 1.0)),
        ];

        let ortho = Mat4::orthographic(-1.0, 1.0, -1.0, 1.0, -1.0, 1.0);

        // Draw back triangle first, then front triangle
        rasterizer.draw_mesh(
            &mut fb,
            &back_vertices,
            &[0, 1, 2],
            &Mat4::identity(),
            &Mat4::identity(),
            &ortho,
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::ONE,
        );


        rasterizer.draw_mesh(
            &mut fb,
            &front_vertices,
            &[0, 1, 2],
            &Mat4::identity(),
            &Mat4::identity(),
            &ortho,
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::ONE,
        );

        let center = fb.get_pixel(50, 50);
        let (r, _g, b, _a) = u32_to_rgba(center);
        assert!(r > b, "Red front triangle should dominate: r={}, b={}", r, b);

        // Now test the reverse order on a new framebuffer: draw front first, then back
        let mut fb2 = FrameBuffer::new(100, 100);
        fb2.clear(rgba_to_u32(0, 0, 0, 255), 1.0);
        rasterizer.reset_stats();

        rasterizer.draw_mesh(
            &mut fb2,
            &front_vertices,
            &[0, 1, 2],
            &Mat4::identity(),
            &Mat4::identity(),
            &ortho,
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::ONE,
        );

        let pass_count_after_front = rasterizer.stats().fragments_passed;

        rasterizer.draw_mesh(
            &mut fb2,
            &back_vertices,
            &[0, 1, 2],
            &Mat4::identity(),
            &Mat4::identity(),
            &ortho,
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::ONE,
        );

        // Back triangle fragments should be rejected by depth test
        assert_eq!(
            rasterizer.stats().fragments_passed,
            pass_count_after_front,
            "Back triangle fragments should be occluded by depth test"
        );
        let center2 = fb2.get_pixel(50, 50);
        let (r2, _g2, b2, _a2) = u32_to_rgba(center2);
        assert!(r2 > b2, "Red front triangle must remain visible");
    }

    #[test]
    fn test_shading_models_and_wireframe() {
        let vertices = vec![
            Vertex::new(Vec3::new(0.0, 0.5, 0.0))
                .with_normal(Vec3::new(0.0, 0.0, 1.0))
                .with_color(Vec4::new(1.0, 1.0, 1.0, 1.0)),
            Vertex::new(Vec3::new(-0.5, -0.5, 0.0))
                .with_normal(Vec3::new(-1.0, 0.0, 1.0).normalize())
                .with_color(Vec4::new(1.0, 0.0, 0.0, 1.0)),
            Vertex::new(Vec3::new(0.5, -0.5, 0.0))
                .with_normal(Vec3::new(1.0, 0.0, 1.0).normalize())
                .with_color(Vec4::new(0.0, 0.0, 1.0, 1.0)),
        ];
        let ortho = Mat4::orthographic(-1.0, 1.0, -1.0, 1.0, -1.0, 1.0);

        for model in [
            ShadingModel::Flat,
            ShadingModel::Gouraud,
            ShadingModel::Phong,
            ShadingModel::Wireframe,
        ] {
            let mut fb = FrameBuffer::new(60, 60);
            fb.clear(0, 1.0);

            let mut rasterizer = Rasterizer::new();
            rasterizer.shading_model = model;
            rasterizer.cull_mode = CullMode::None;

            rasterizer.draw_mesh(
                &mut fb,
                &vertices,
                &[0, 1, 2],
                &Mat4::identity(),
                &Mat4::identity(),
                &ortho,
                Vec3::new(0.0, 1.0, 1.0),
                Vec3::ONE,
            );

            assert_eq!(rasterizer.stats().triangles_rendered, 1);
            assert!(rasterizer.stats().fragments_passed > 0);
        }
    }

    #[test]
    fn test_near_plane_clipping() {
        let mut fb = FrameBuffer::new(80, 80);
        fb.clear(0, 1.0);

        let mut rasterizer = Rasterizer::new();
        rasterizer.shading_model = ShadingModel::Flat;
        rasterizer.cull_mode = CullMode::None;

        // Perspective camera looking at (0, 0, 0) from (0, 0, 2)
        let view = Mat4::look_at(Vec3::new(0.0, 0.0, 2.0), Vec3::ZERO, Vec3::UP);
        let proj = Mat4::perspective(60.0f32.to_radians(), 1.0, 0.5, 100.0);

        // Triangle spanning across near plane:
        // Vertex 0 is in front of camera (Z = 0, distance 2), vertex 1 & 2 are behind camera (Z = 4, distance -2)
        let vertices = vec![
            Vertex::new(Vec3::new(0.0, 1.0, 0.0)),
            Vertex::new(Vec3::new(-1.0, -1.0, 4.0)),
            Vertex::new(Vec3::new(1.0, -1.0, 4.0)),
        ];

        rasterizer.draw_mesh(
            &mut fb,
            &vertices,
            &[0, 1, 2],
            &Mat4::identity(),
            &view,
            &proj,
            Vec3::new(0.0, 1.0, 1.0),
            Vec3::ONE,
        );

        // Clipped triangle must render without panicking or creating NaN/infinite coords
        assert!(rasterizer.stats().triangles_rendered > 0);
    }

    #[test]
    fn test_software_raster_pass_integration() {
        let config = RenderConfig {
            width: 80,
            height: 60,
            clear_color: Vec4::new(0.1, 0.1, 0.1, 1.0),
            ..Default::default()
        };
        let renderer = Renderer::new(config);

        // Add perspective camera looking towards origin from (0, 0, -3)
        let mut cam = Camera::new_perspective(
            Vec3::new(0.0, 0.0, -3.0),
            60.0f32.to_radians(),
            0.1,
            100.0,
        );
        cam.update();
        renderer.add_camera(cam);

        // Add directional light
        let light = Light::new("Sun", LightType::Directional)
            .with_direction(Vec3::new(0.0, -1.0, -1.0))
            .with_color(Vec3::ONE)
            .with_intensity(1.0);
        renderer.add_light(light);

        // Register 3D triangle mesh
        let mesh = Mesh::new("TestTriangle")
            .with_vertices(vec![
                Vertex::new(Vec3::new(0.0, 0.8, 0.0)).with_color(Vec4::new(1.0, 0.0, 0.0, 1.0)),
                Vertex::new(Vec3::new(-0.8, -0.8, 0.0)).with_color(Vec4::new(0.0, 1.0, 0.0, 1.0)),
                Vertex::new(Vec3::new(0.8, -0.8, 0.0)).with_color(Vec4::new(0.0, 0.0, 1.0, 1.0)),
            ])
            .with_indices(vec![0, 1, 2]);
        renderer.register_mesh(mesh);

        // Setup software raster pass
        let pass = SoftwareRasterPass::new(80, 60);
        assert_eq!(pass.pass_type(), RenderPassType::SoftwareRaster);
        assert!(pass.is_enabled());

        pass.execute(&renderer);

        let fb = pass.framebuffer().read();
        assert_eq!(fb.width(), 80);
        assert_eq!(fb.height(), 60);

        // Center should have rendered triangle pixel
        let center_color = fb.get_pixel(40, 30);
        assert_ne!(center_color, vec4_to_u32(Vec4::new(0.1, 0.1, 0.1, 1.0)));

        // Test export frame
        let temp_bmp = std::env::temp_dir().join("ow_pass_test.bmp");
        pass.export_frame_bmp(&temp_bmp).expect("Export failed");
        assert!(temp_bmp.exists());
        let _ = std::fs::remove_file(temp_bmp);
    }
}
