use crate::AppSettings;
use crate::rendering::UiContext;
use crate::world::World;
use std::error::Error;
use std::marker::PhantomData;
use winit::dpi::{PhysicalSize, Size};
use winit::window::WindowAttributes;

#[allow(unused)]
pub trait AppState: Sized + Default + 'static {
    fn init(&mut self, world: &mut World) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    fn update(&mut self, world: &mut World) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    fn late_update(&mut self, world: &mut World) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    fn post_update(&mut self, world: &mut World) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    fn on_gui(&mut self, world: &mut World, ctx: &UiContext) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    fn destroy(&mut self, world: &mut World) -> Result<(), Box<dyn Error>> {
        Ok(())
    }
}

pub trait AppRuntime: AppState {
    fn configure(title: &str, width: u32, height: u32) -> AppSettings<Self>;

    fn default_config() -> AppSettings<Self>;
}

impl<S: AppState> AppRuntime for S {
    fn configure(title: &str, width: u32, height: u32) -> AppSettings<Self> {
        AppSettings {
            main_window: WindowAttributes::default()
                .with_inner_size(Size::Physical(PhysicalSize { width, height }))
                .with_title(title),
            _state_type: PhantomData,
        }
    }

    fn default_config() -> AppSettings<Self> {
        AppSettings {
            main_window: WindowAttributes::default()
                .with_inner_size(Size::Physical(PhysicalSize {
                    width: 800,
                    height: 600,
                }))
                .with_title("Syrillian Window"),
            _state_type: PhantomData,
        }
    }
}
