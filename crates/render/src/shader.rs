#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShaderStage {
    Vertex,
    Fragment,
    Geometry,
    Compute,
    TessellationControl,
    TessellationEvaluation,
}

#[derive(Debug, Clone)]
pub struct Shader {
    pub name: String,
    pub stage: ShaderStage,
    pub source: String,
    pub compiled: bool,
    pub handle: u32,
}

impl Shader {
    pub fn new(name: impl Into<String>, stage: ShaderStage, source: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            stage,
            source: source.into(),
            compiled: false,
            handle: 0,
        }
    }

    pub fn vertex(source: impl Into<String>) -> Self {
        Self::new("vertex", ShaderStage::Vertex, source)
    }

    pub fn fragment(source: impl Into<String>) -> Self {
        Self::new("fragment", ShaderStage::Fragment, source)
    }

    pub fn compute(source: impl Into<String>) -> Self {
        Self::new("compute", ShaderStage::Compute, source)
    }
}

#[derive(Debug, Clone)]
pub struct ShaderProgram {
    pub name: String,
    pub vertex_shader: Option<Shader>,
    pub fragment_shader: Option<Shader>,
    pub geometry_shader: Option<Shader>,
    pub compute_shader: Option<Shader>,
    pub linked: bool,
    pub handle: u32,
    pub uniforms: std::collections::HashMap<String, Uniform>,
}

#[derive(Debug, Clone)]
pub struct Uniform {
    pub name: String,
    pub uniform_type: UniformType,
    pub location: i32,
    pub size: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UniformType {
    Float,
    Vec2,
    Vec3,
    Vec4,
    Int,
    Bool,
    Mat3,
    Mat4,
    Sampler2D,
    SamplerCube,
}

impl ShaderProgram {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            vertex_shader: None,
            fragment_shader: None,
            geometry_shader: None,
            compute_shader: None,
            linked: false,
            handle: 0,
            uniforms: std::collections::HashMap::new(),
        }
    }

    pub fn with_vertex_shader(mut self, shader: Shader) -> Self {
        self.vertex_shader = Some(shader);
        self
    }

    pub fn with_fragment_shader(mut self, shader: Shader) -> Self {
        self.fragment_shader = Some(shader);
        self
    }

    pub fn with_geometry_shader(mut self, shader: Shader) -> Self {
        self.geometry_shader = Some(shader);
        self
    }

    pub fn with_compute_shader(mut self, shader: Shader) -> Self {
        self.compute_shader = Some(shader);
        self
    }

    pub fn add_uniform(&mut self, name: impl Into<String>, uniform_type: UniformType) {
        let name = name.into();
        self.uniforms.insert(name.clone(), Uniform {
            name,
            uniform_type,
            location: -1,
            size: 0,
        });
    }
}
