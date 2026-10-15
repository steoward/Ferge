use crate::renderer::Renderer;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RenderPassType {
    Shadow,
    Geometry,
    Lighting,
    PostProcess,
    UI,
    Debug,
}

pub trait RenderPass: Send + Sync {
    fn pass_type(&self) -> RenderPassType;
    fn execute(&self, renderer: &Renderer);
    fn is_enabled(&self) -> bool;
    fn set_enabled(&mut self, enabled: bool);
}

pub struct ShadowPass {
    enabled: bool,
    shadow_map_size: u32,
    cascade_count: u32,
    cascade_splits: Vec<f32>,
}

impl ShadowPass {
    pub fn new(shadow_map_size: u32, cascade_count: u32) -> Self {
        Self {
            enabled: true,
            shadow_map_size,
            cascade_count,
            cascade_splits: vec![0.0; cascade_count as usize],
        }
    }

    pub fn shadow_map_size(&self) -> u32 {
        self.shadow_map_size
    }

    pub fn cascade_count(&self) -> u32 {
        self.cascade_count
    }

    pub fn cascade_splits(&self) -> &[f32] {
        &self.cascade_splits
    }
}

impl RenderPass for ShadowPass {
    fn pass_type(&self) -> RenderPassType {
        RenderPassType::Shadow
    }

    fn execute(&self, _renderer: &Renderer) {
        // Shadow map rendering implementation
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
}

pub struct GeometryPass {
    enabled: bool,
    render_opaque: bool,
    render_transparent: bool,
    render_skybox: bool,
}

impl Default for GeometryPass {
    fn default() -> Self {
        Self::new()
    }
}

impl GeometryPass {
    pub fn new() -> Self {
        Self {
            enabled: true,
            render_opaque: true,
            render_transparent: true,
            render_skybox: true,
        }
    }

    pub fn set_render_opaque(&mut self, value: bool) {
        self.render_opaque = value;
    }

    pub fn set_render_transparent(&mut self, value: bool) {
        self.render_transparent = value;
    }

    pub fn set_render_skybox(&mut self, value: bool) {
        self.render_skybox = value;
    }

    pub fn renders_opaque(&self) -> bool {
        self.render_opaque
    }

    pub fn renders_transparent(&self) -> bool {
        self.render_transparent
    }

    pub fn renders_skybox(&self) -> bool {
        self.render_skybox
    }
}

impl RenderPass for GeometryPass {
    fn pass_type(&self) -> RenderPassType {
        RenderPassType::Geometry
    }

    fn execute(&self, _renderer: &Renderer) {
        // Geometry buffer rendering implementation
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
}

pub struct LightingPass {
    enabled: bool,
    use_deferred: bool,
    use_ssao: bool,
    use_ssr: bool,
}

impl Default for LightingPass {
    fn default() -> Self {
        Self::new()
    }
}

impl LightingPass {
    pub fn new() -> Self {
        Self {
            enabled: true,
            use_deferred: true,
            use_ssao: true,
            use_ssr: false,
        }
    }

    pub fn set_deferred(&mut self, value: bool) {
        self.use_deferred = value;
    }

    pub fn set_ssao(&mut self, value: bool) {
        self.use_ssao = value;
    }

    pub fn set_ssr(&mut self, value: bool) {
        self.use_ssr = value;
    }

    pub fn is_deferred(&self) -> bool {
        self.use_deferred
    }

    pub fn uses_ssao(&self) -> bool {
        self.use_ssao
    }

    pub fn uses_ssr(&self) -> bool {
        self.use_ssr
    }
}

impl RenderPass for LightingPass {
    fn pass_type(&self) -> RenderPassType {
        RenderPassType::Lighting
    }

    fn execute(&self, _renderer: &Renderer) {
        // Lighting calculation implementation
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
}

pub struct PostProcessPass {
    enabled: bool,
    effects: Vec<PostProcessEffect>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PostProcessEffect {
    Bloom,
    ToneMapping,
    FXAA,
    SSAO,
    SSR,
    MotionBlur,
    DepthOfField,
    Vignette,
    ChromaticAberration,
    FilmGrain,
}

impl Default for PostProcessPass {
    fn default() -> Self {
        Self::new()
    }
}

impl PostProcessPass {
    pub fn new() -> Self {
        Self {
            enabled: true,
            effects: vec![
                PostProcessEffect::Bloom,
                PostProcessEffect::ToneMapping,
                PostProcessEffect::FXAA,
            ],
        }
    }

    pub fn add_effect(&mut self, effect: PostProcessEffect) {
        if !self.effects.contains(&effect) {
            self.effects.push(effect);
        }
    }

    pub fn remove_effect(&mut self, effect: PostProcessEffect) {
        self.effects.retain(|e| *e != effect);
    }

    pub fn has_effect(&self, effect: PostProcessEffect) -> bool {
        self.effects.contains(&effect)
    }

    pub fn effects(&self) -> &[PostProcessEffect] {
        &self.effects
    }
}

impl RenderPass for PostProcessPass {
    fn pass_type(&self) -> RenderPassType {
        RenderPassType::PostProcess
    }

    fn execute(&self, _renderer: &Renderer) {
        // Post-processing implementation
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
}
