use wgpu::naga::WithSpan;
use wgpu::naga::front::wgsl;
use wgpu::naga::front::wgsl::ParseError;
use wgpu::naga::valid::{Capabilities, ModuleInfo, ValidationError, ValidationFlags, Validator};

#[derive(Debug)]
pub enum ShaderValidError {
    Parse(ParseError),
    ValidationError(Box<WithSpan<ValidationError>>),
}

impl ShaderValidError {
    pub fn emit_to_stderr(&self, source: &str) {
        match self {
            ShaderValidError::Parse(e) => e.emit_to_stderr(source),
            ShaderValidError::ValidationError(e) => e.emit_to_stderr(source),
        }
    }

    pub fn emit_to_stderr_with_path(&self, source: &str, path: &str) {
        match self {
            ShaderValidError::Parse(e) => e.emit_to_stderr_with_path(source, path),
            ShaderValidError::ValidationError(e) => e.emit_to_stderr_with_path(source, path),
        }
    }

    pub fn emit_to_string(&self, source: &str) -> String {
        match self {
            ShaderValidError::Parse(e) => e.emit_to_string(source),
            ShaderValidError::ValidationError(e) => e.emit_to_string(source),
        }
    }
}

pub fn validate_wgsl_source(shader: &str) -> Result<ModuleInfo, ShaderValidError> {
    let module = wgsl::parse_str(shader).map_err(ShaderValidError::Parse)?;
    let mut validator = Validator::new(ValidationFlags::all(), Capabilities::all());
    validator
        .validate(&module)
        .map_err(|e| ShaderValidError::ValidationError(Box::new(e)))
}
