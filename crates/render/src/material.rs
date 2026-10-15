use ow_math::{Vec3, Vec4};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MaterialProperty {
    Float(f32),
    Vec2(ow_math::Vec2),
    Vec3(Vec3),
    Vec4(Vec4),
    Int(i32),
    Bool(bool),
    Texture(u32),
}

#[derive(Debug, Clone)]
pub struct Material {
    pub name: String,
    pub shader: Option<crate::shader::ShaderProgram>,
    pub properties: std::collections::HashMap<String, MaterialProperty>,
    pub base_color: Vec4,
    pub metallic: f32,
    pub roughness: f32,
    pub emissive: Vec3,
    pub alpha: f32,
    pub double_sided: bool,
    pub wireframe: bool,
}

impl Material {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            shader: None,
            properties: std::collections::HashMap::new(),
            base_color: Vec4::new(1.0, 1.0, 1.0, 1.0),
            metallic: 0.0,
            roughness: 0.5,
            emissive: Vec3::ZERO,
            alpha: 1.0,
            double_sided: false,
            wireframe: false,
        }
    }

    pub fn with_base_color(mut self, color: Vec4) -> Self {
        self.base_color = color;
        self
    }

    pub fn with_metallic(mut self, metallic: f32) -> Self {
        self.metallic = metallic.clamp(0.0, 1.0);
        self
    }

    pub fn with_roughness(mut self, roughness: f32) -> Self {
        self.roughness = roughness.clamp(0.0, 1.0);
        self
    }

    pub fn with_emissive(mut self, emissive: Vec3) -> Self {
        self.emissive = emissive;
        self
    }

    pub fn with_alpha(mut self, alpha: f32) -> Self {
        self.alpha = alpha.clamp(0.0, 1.0);
        self
    }

    pub fn with_double_sided(mut self, double_sided: bool) -> Self {
        self.double_sided = double_sided;
        self
    }

    pub fn with_wireframe(mut self, wireframe: bool) -> Self {
        self.wireframe = wireframe;
        self
    }

    pub fn set_property(&mut self, name: impl Into<String>, property: MaterialProperty) {
        self.properties.insert(name.into(), property);
    }

    pub fn get_property(&self, name: &str) -> Option<&MaterialProperty> {
        self.properties.get(name)
    }
}

#[derive(Debug, Clone)]
pub struct Texture {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub format: TextureFormat,
    pub data: Vec<u8>,
    pub mip_levels: u32,
    pub is_cubemap: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextureFormat {
    R8,
    RG8,
    RGB8,
    RGBA8,
    R16F,
    RG16F,
    RGB16F,
    RGBA16F,
    R32F,
    RG32F,
    RGB32F,
    RGBA32F,
    Depth16,
    Depth24,
    Depth32F,
    Depth24Stencil8,
    BC1,
    BC3,
    BC5,
    BC7,
}

impl Texture {
    pub fn new(name: impl Into<String>, width: u32, height: u32, format: TextureFormat) -> Self {
        Self {
            name: name.into(),
            width,
            height,
            format,
            data: Vec::new(),
            mip_levels: 1,
            is_cubemap: false,
        }
    }

    pub fn with_data(mut self, data: Vec<u8>) -> Self {
        self.data = data;
        self
    }

    pub fn with_mip_levels(mut self, mip_levels: u32) -> Self {
        self.mip_levels = mip_levels;
        self
    }

    pub fn with_cubemap(mut self, is_cubemap: bool) -> Self {
        self.is_cubemap = is_cubemap;
        self
    }

    pub fn size_in_bytes(&self) -> usize {
        let bpp = match self.format {
            TextureFormat::R8 => 1,
            TextureFormat::RG8 => 2,
            TextureFormat::RGB8 => 3,
            TextureFormat::RGBA8 => 4,
            TextureFormat::R16F => 2,
            TextureFormat::RG16F => 4,
            TextureFormat::RGB16F => 6,
            TextureFormat::RGBA16F => 8,
            TextureFormat::R32F => 4,
            TextureFormat::RG32F => 8,
            TextureFormat::RGB32F => 12,
            TextureFormat::RGBA32F => 16,
            TextureFormat::Depth16 => 2,
            TextureFormat::Depth24 => 3,
            TextureFormat::Depth32F => 4,
            TextureFormat::Depth24Stencil8 => 4,
            TextureFormat::BC1 => 1,
            TextureFormat::BC3 => 1,
            TextureFormat::BC5 => 1,
            TextureFormat::BC7 => 1,
        };
        (self.width * self.height * bpp) as usize
    }
}
