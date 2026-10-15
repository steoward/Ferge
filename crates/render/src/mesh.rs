use ow_math::{Vec2, Vec3, Vec4};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vertex {
    pub position: Vec3,
    pub normal: Vec3,
    pub tangent: Vec3,
    pub bitangent: Vec3,
    pub color: Vec4,
    pub uv0: Vec2,
    pub uv1: Vec2,
    pub bone_weights: Vec4,
    pub bone_indices: [u32; 4],
}

impl Vertex {
    pub fn new(position: Vec3) -> Self {
        Self {
            position,
            normal: Vec3::UP,
            tangent: Vec3::RIGHT,
            bitangent: Vec3::FORWARD,
            color: Vec4::new(1.0, 1.0, 1.0, 1.0),
            uv0: Vec2::ZERO,
            uv1: Vec2::ZERO,
            bone_weights: Vec4::new(1.0, 0.0, 0.0, 0.0),
            bone_indices: [0, 0, 0, 0],
        }
    }

    pub fn with_normal(mut self, normal: Vec3) -> Self {
        self.normal = normal;
        self
    }

    pub fn with_uv(mut self, uv: Vec2) -> Self {
        self.uv0 = uv;
        self
    }

    pub fn with_color(mut self, color: Vec4) -> Self {
        self.color = color;
        self
    }
}

#[derive(Debug, Clone)]
pub struct VertexBuffer {
    pub vertices: Vec<Vertex>,
    pub vertex_count: usize,
    pub vertex_stride: usize,
}

impl VertexBuffer {
    pub fn new(vertices: Vec<Vertex>) -> Self {
        let vertex_count = vertices.len();
        let vertex_stride = std::mem::size_of::<Vertex>();
        Self {
            vertices,
            vertex_count,
            vertex_stride,
        }
    }

    pub fn empty() -> Self {
        Self {
            vertices: Vec::new(),
            vertex_count: 0,
            vertex_stride: std::mem::size_of::<Vertex>(),
        }
    }

    pub fn add_vertex(&mut self, vertex: Vertex) {
        self.vertices.push(vertex);
        self.vertex_count = self.vertices.len();
    }

    pub fn clear(&mut self) {
        self.vertices.clear();
        self.vertex_count = 0;
    }
}

#[derive(Debug, Clone)]
pub struct IndexBuffer {
    pub indices: Vec<u32>,
    pub index_count: usize,
}

impl IndexBuffer {
    pub fn new(indices: Vec<u32>) -> Self {
        let index_count = indices.len();
        Self {
            indices,
            index_count,
        }
    }

    pub fn empty() -> Self {
        Self {
            indices: Vec::new(),
            index_count: 0,
        }
    }

    pub fn add_index(&mut self, index: u32) {
        self.indices.push(index);
        self.index_count = self.indices.len();
    }

    pub fn clear(&mut self) {
        self.indices.clear();
        self.index_count = 0;
    }
}

#[derive(Debug, Clone)]
pub struct Mesh {
    pub name: String,
    pub vertex_buffer: VertexBuffer,
    pub index_buffer: IndexBuffer,
    pub bounding_box: (Vec3, Vec3),
    pub bounding_sphere: (Vec3, f32),
}

impl Mesh {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            vertex_buffer: VertexBuffer::empty(),
            index_buffer: IndexBuffer::empty(),
            bounding_box: (Vec3::ZERO, Vec3::ZERO),
            bounding_sphere: (Vec3::ZERO, 0.0),
        }
    }

    pub fn with_vertices(mut self, vertices: Vec<Vertex>) -> Self {
        self.vertex_buffer = VertexBuffer::new(vertices);
        self.calculate_bounds();
        self
    }

    pub fn with_indices(mut self, indices: Vec<u32>) -> Self {
        self.index_buffer = IndexBuffer::new(indices);
        self
    }

    pub fn calculate_bounds(&mut self) {
        if self.vertex_buffer.vertices.is_empty() {
            return;
        }

        let mut min = Vec3::new(f32::INFINITY, f32::INFINITY, f32::INFINITY);
        let mut max = Vec3::new(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);

        for vertex in &self.vertex_buffer.vertices {
            min.x = min.x.min(vertex.position.x);
            min.y = min.y.min(vertex.position.y);
            min.z = min.z.min(vertex.position.z);
            max.x = max.x.max(vertex.position.x);
            max.y = max.y.max(vertex.position.y);
            max.z = max.z.max(vertex.position.z);
        }

        self.bounding_box = (min, max);
        let center = (min + max) * 0.5;
        let mut max_dist: f32 = 0.0;
        for vertex in &self.vertex_buffer.vertices {
            let dist = (vertex.position - center).length();
            max_dist = max_dist.max(dist);
        }
        self.bounding_sphere = (center, max_dist);
    }

    pub fn triangle_count(&self) -> usize {
        self.index_buffer.index_count / 3
    }

    pub fn vertex_count(&self) -> usize {
        self.vertex_buffer.vertex_count
    }
}
