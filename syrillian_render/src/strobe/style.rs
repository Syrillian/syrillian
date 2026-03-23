use crate::strobe::ui_element::Padding;

#[derive(Debug, Clone, Copy, Default)]
pub enum Size {
    #[default]
    Auto,
    Fixed(f32),
    Fill(f32),
}

#[derive(Debug, Clone, Copy, Default)]
pub enum Align {
    #[default]
    Start,
    Center,
    End,
    Stretch,
}

#[derive(Default, Clone)]
pub struct Style {
    pub padding: Padding,
    pub width: Size,
    pub height: Size,
    pub align: Align,
}
