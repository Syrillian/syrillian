use parking_lot::RwLock;

static DEBUG_RENDERER: RwLock<DebugRenderer> = RwLock::new(DebugRenderer::default_const());

#[derive(Default, Debug, Clone)]
pub struct DebugRenderer {
    pub mesh_edges: bool,
    pub vertex_normals: bool,
    pub rays: bool,
    pub colliders_edges: bool,
    pub text_geometry: bool,
    pub light: bool,
}

impl DebugRenderer {
    const fn default_const() -> Self {
        DebugRenderer {
            mesh_edges: false,
            colliders_edges: false,
            vertex_normals: false,
            rays: false,
            text_geometry: false,
            light: false,
        }
    }

    pub fn next_mode() -> u32 {
        let mut inner = DEBUG_RENDERER.write();
        let mode = inner._mode();
        inner._set_mode(mode + 1)
    }

    pub fn mesh_edges() -> bool {
        let inner = DEBUG_RENDERER.read();
        inner.mesh_edges
    }

    pub fn collider_mesh() -> bool {
        let inner = DEBUG_RENDERER.read();
        inner.colliders_edges
    }

    pub fn mesh_vertex_normals() -> bool {
        let inner = DEBUG_RENDERER.read();
        inner.vertex_normals
    }

    pub fn physics_rays() -> bool {
        let inner = DEBUG_RENDERER.read();
        inner.rays
    }

    pub fn text_geometry() -> bool {
        let inner = DEBUG_RENDERER.read();
        inner.text_geometry
    }

    pub fn light() -> bool {
        let inner = DEBUG_RENDERER.read();
        inner.light
    }

    pub fn off() {
        let mut inner = DEBUG_RENDERER.write();
        inner._off();
    }

    pub fn mode() -> u32 {
        let inner = DEBUG_RENDERER.write();
        inner._mode()
    }

    fn _mode(&self) -> u32 {
        if self.mesh_edges {
            1
        } else if self.vertex_normals {
            2
        } else if self.rays {
            3
        } else if self.colliders_edges {
            4
        } else if self.text_geometry {
            5
        } else if self.light {
            6
        } else {
            0
        }
    }

    pub fn set_mode(mode: u32) {
        let mut inner = DEBUG_RENDERER.write();
        inner._set_mode(mode);
    }

    fn _set_mode(&mut self, mode: u32) -> u32 {
        self._off();
        match mode {
            1 => self.mesh_edges = true,
            2 => self.vertex_normals = true,
            3 => self.rays = true,
            4 => self.colliders_edges = true,
            5 => self.text_geometry = true,
            6 => self.light = true,
            _ => return 0,
        }
        mode
    }

    fn _off(&mut self) {
        self.mesh_edges = false;
        self.colliders_edges = false;
        self.vertex_normals = false;
        self.rays = false;
        self.text_geometry = false;
        self.light = false;
    }
}
