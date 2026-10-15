use ow_math::Vec3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LightType {
    Directional,
    Point,
    Spot,
    Area,
}

#[derive(Debug, Clone)]
pub struct Light {
    pub name: String,
    pub light_type: LightType,
    pub position: Vec3,
    pub direction: Vec3,
    pub color: Vec3,
    pub intensity: f32,
    pub range: f32,
    pub inner_cone_angle: f32,
    pub outer_cone_angle: f32,
    pub cast_shadows: bool,
    pub shadow_map_size: u32,
    pub shadow_bias: f32,
    pub enabled: bool,
}

impl Light {
    pub fn new(name: impl Into<String>, light_type: LightType) -> Self {
        Self {
            name: name.into(),
            light_type,
            position: Vec3::ZERO,
            direction: Vec3::FORWARD,
            color: Vec3::ONE,
            intensity: 1.0,
            range: 10.0,
            inner_cone_angle: 30.0,
            outer_cone_angle: 45.0,
            cast_shadows: false,
            shadow_map_size: 1024,
            shadow_bias: 0.005,
            enabled: true,
        }
    }

    pub fn with_position(mut self, position: Vec3) -> Self {
        self.position = position;
        self
    }

    pub fn with_direction(mut self, direction: Vec3) -> Self {
        self.direction = direction.normalize();
        self
    }

    pub fn with_color(mut self, color: Vec3) -> Self {
        self.color = color;
        self
    }

    pub fn with_intensity(mut self, intensity: f32) -> Self {
        self.intensity = intensity;
        self
    }

    pub fn with_range(mut self, range: f32) -> Self {
        self.range = range;
        self
    }

    pub fn with_cone_angles(mut self, inner: f32, outer: f32) -> Self {
        self.inner_cone_angle = inner;
        self.outer_cone_angle = outer;
        self
    }

    pub fn with_shadows(mut self, cast_shadows: bool) -> Self {
        self.cast_shadows = cast_shadows;
        self
    }

    pub fn with_shadow_map_size(mut self, size: u32) -> Self {
        self.shadow_map_size = size;
        self
    }

    pub fn with_shadow_bias(mut self, bias: f32) -> Self {
        self.shadow_bias = bias;
        self
    }

    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

pub type DirectionalLight = Light;
pub type PointLight = Light;
pub type SpotLight = Light;
