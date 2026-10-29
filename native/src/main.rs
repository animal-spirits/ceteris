use ceteris_app::map::Map;

slint::include_modules!();

fn main() -> Result<(), Box<dyn std::error::Error>> {
    slint::BackendSelector::new()
        .require_wgpu_30(slint::wgpu_30::WGPUConfiguration::default())
        .select()?;
    let app = AppWindow::new()?;
    let weak = app.as_weak();
    let mut map = Map::new();
    app.window().set_rendering_notifier(move |state, api| {
        let (
            slint::RenderingState::BeforeRendering,
            slint::GraphicsAPI::WGPU30 { device, queue, .. },
        ) = (state, api)
        else {
            return;
        };
        let Some(app) = weak.upgrade() else { return };
        let size = app.window().size();
        if let Some(texture) = map.render(device, queue, size.width, size.height) {
            match slint::Image::try_from(texture) {
                Ok(image) => app.set_map_texture(image),
                Err(e) => eprintln!("map texture: {e}"),
            }
        }
    })?;
    app.run()?;
    Ok(())
}
