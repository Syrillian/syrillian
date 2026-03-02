use argh::FromArgs;
use glamx::UVec2;
use std::cmp::Ordering;
use std::sync::LazyLock;
use std::time::Duration;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Default)]
pub enum AntiAliasingMode {
    Off,
    #[default]
    Fxaa,
}

fn present_mode(mode: &str) -> Result<Option<wgpu::PresentMode>, String> {
    let parsed = match mode {
        "vsync" => wgpu::PresentMode::AutoVsync,
        "no_vsync" => wgpu::PresentMode::AutoNoVsync,
        "fifo" => wgpu::PresentMode::Fifo,
        "fifo_relaxed" => wgpu::PresentMode::FifoRelaxed,
        "mailbox" => wgpu::PresentMode::Mailbox,
        "immediate" => wgpu::PresentMode::Immediate,
        _ => return Ok(None),
    };
    Ok(Some(parsed))
}

fn window_size(size: &str) -> Result<Option<UVec2>, String> {
    let char = if size.contains('x') { 'x' } else { ',' };

    let mut split = size.split(char);
    let x: Option<u32> = split.next().and_then(|x| x.parse().ok());
    let y: Option<u32> = split.next().and_then(|y| y.parse().ok());

    let size = match (x, y) {
        (Some(x), Some(y)) => UVec2::new(x, y),
        (Some(x), _) => UVec2::new(x, x),
        _ => return Ok(None),
    };

    Ok(Some(size))
}

fn force_backend(backend: &str) -> Result<Option<Vec<wgpu::Backends>>, String> {
    let backends: wgpu::Backends = wgpu::Backends::from_comma_list(backend);

    if backends.is_empty() {
        return Ok(None);
    }

    let mut backends: Vec<wgpu::Backends> = backends.into_iter().collect();

    // push vulkan back if it's a choice because all other backends are more stable
    backends.sort_by(|a, _| {
        if a.contains(wgpu::Backends::VULKAN) {
            Ordering::Greater
        } else {
            Ordering::Less
        }
    });

    Ok(Some(backends))
}

fn aa_mode(mode: &str) -> Result<Option<AntiAliasingMode>, String> {
    let mode = match mode {
        "off" => AntiAliasingMode::Off,
        "fxaa" => AntiAliasingMode::Fxaa,
        _ => return Ok(None),
    };
    Ok(Some(mode))
}

/// Engine arguments
#[derive(Default, FromArgs)]
pub struct EngineArgs {
    #[argh(switch, hidden_help)]
    pub fullscreen: bool,
    #[argh(switch, hidden_help)]
    pub no_fullscreen: bool, // TODO: Implement
    #[argh(switch, hidden_help)]
    pub no_frustum_culling: bool,
    #[argh(switch, hidden_help)]
    pub no_shadows: bool,
    #[argh(switch, hidden_help)]
    pub no_ssr: bool,
    #[argh(switch, hidden_help)]
    pub no_ssao: bool,
    #[argh(switch, hidden_help)]
    pub no_bloom: bool,

    #[argh(option, hidden_help)]
    pub max_frames_in_flight: Option<u32>,
    #[argh(option, hidden_help)]
    pub max_fps: Option<u32>,
    #[argh(option, hidden_help)]
    pub physics_timestep: Option<f64>,

    #[argh(option, hidden_help, from_str_fn(present_mode))]
    pub present_mode: Option<Option<wgpu::PresentMode>>,
    #[argh(option, hidden_help, from_str_fn(window_size))]
    pub window_size: Option<Option<UVec2>>,
    #[argh(option, hidden_help, from_str_fn(force_backend))]
    pub force_backend: Option<Option<Vec<wgpu::Backends>>>,
    #[argh(option, hidden_help, from_str_fn(aa_mode))]
    pub aa_mode: Option<Option<AntiAliasingMode>>,
    #[argh(option, hidden_help)]
    pub bloom_threshold: Option<f32>,
    #[argh(option, hidden_help)]
    pub bloom_soft_knee: Option<f32>,
    #[argh(option, hidden_help)]
    pub bloom_intensity: Option<f32>,
    #[argh(option, hidden_help)]
    pub bloom_radius: Option<f32>,
    #[argh(option, hidden_help)]
    pub bloom_clamp_max: Option<f32>,
    #[argh(option, hidden_help)]
    pub bloom_blur_passes: Option<u32>,
}

impl EngineArgs {
    fn init() -> Option<EngineArgs> {
        let mut args = std::env::args();
        let cmd_name = args.next()?;
        let args: Vec<String> = args.collect();
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        EngineArgs::from_args(&[&cmd_name], &args).ok()
    }

    pub fn get() -> &'static EngineArgs {
        static INSTANCE: LazyLock<EngineArgs> =
            LazyLock::new(|| EngineArgs::init().unwrap_or_default());
        &INSTANCE
    }

    pub fn default_window_size() -> UVec2 {
        EngineArgs::get()
            .window_size
            .flatten()
            .unwrap_or(UVec2::new(800, 600))
    }

    pub fn max_fps() -> Option<u32> {
        EngineArgs::get().max_fps.filter(|&fps| fps > 0)
    }

    pub fn max_frame_interval() -> Option<Duration> {
        Self::max_fps().map(|fps| Duration::from_secs_f32(1.0 / fps as f32))
    }

    pub fn aa_mode() -> AntiAliasingMode {
        EngineArgs::get().aa_mode.flatten().unwrap_or_default()
    }
}
