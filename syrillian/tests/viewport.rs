use syrillian::windowing::{RenderTarget, ViewportId};

#[test]
fn test_viewport_id() {
    let primary = ViewportId::PRIMARY;
    assert_eq!(primary.get(), 0);
    assert!(primary.is_primary());

    let secondary = ViewportId(1);
    assert_eq!(secondary.get(), 1);
    assert!(!secondary.is_primary());
}

#[test]
fn test_render_target() {
    let target = RenderTarget::PRIMARY_WINDOW;

    if let RenderTarget::Viewport(id) = target {
        assert!(id.is_primary());
    } else {
        panic!("Primary window target should be a viewport");
    }
}
