use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use ceteris_app::bench::Bench;
use ceteris_app::camera::Camera;
use ceteris_app::fill::Fill;
use ceteris_app::map::Map;
use ceteris_app::modes;
use ceteris_app::web::Provinces;

slint::include_modules!();

/// `$CETERIS_PACK`, else `data/packs/2000` in the repository, as `ceteris_geo::pack::default_dir`.
fn pack_dir() -> PathBuf {
    std::env::var_os("CETERIS_PACK")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../data/packs/2000"))
}

struct State {
    fill: Option<Fill>,
    provinces: Provinces,
    map: Option<Map>,
    camera: Option<Camera>,
    viewport: [f64; 2],
    scale_factor: f64,
    pick: Option<[u32; 2]>,
    bench: Option<Bench>,
}

impl State {
    fn physical(&self, x: f32, y: f32) -> [f64; 2] {
        [
            f64::from(x) * self.scale_factor,
            f64::from(y) * self.scale_factor,
        ]
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = pack_dir().join("web");
    let fill = Fill::read(&dir.join("provinces.fill"))?;
    let provinces = Provinces::read(&dir.join("provinces.json"))?;

    slint::BackendSelector::new()
        .require_wgpu_30(slint::wgpu_30::WGPUConfiguration::default())
        .select()?;
    let app = AppWindow::new()?;
    let state = Rc::new(RefCell::new(State {
        fill: Some(fill),
        provinces,
        map: None,
        camera: None,
        viewport: [1.0, 1.0],
        scale_factor: 1.0,
        pick: None,
        bench: std::env::var_os("CETERIS_BENCH").map(|_| Bench::new()),
    }));

    let s = state.clone();
    let weak = app.as_weak();
    app.on_pan(move |dx, dy| {
        let mut s = s.borrow_mut();
        let [dx, dy] = s.physical(dx, dy);
        if let Some(c) = s.camera.as_mut() {
            c.pan(dx, dy);
        }
        weak.upgrade().inspect(|a| a.window().request_redraw());
    });
    let s = state.clone();
    let weak = app.as_weak();
    app.on_zoom(move |x, y, delta| {
        let mut s = s.borrow_mut();
        let (px, viewport) = (s.physical(x, y), s.viewport);
        if let Some(c) = s.camera.as_mut() {
            c.zoom_at(viewport, px, (f64::from(delta) / 240.0).exp2());
        }
        weak.upgrade().inspect(|a| a.window().request_redraw());
    });
    let s = state.clone();
    let weak = app.as_weak();
    app.on_select(move |x, y| {
        let mut s = s.borrow_mut();
        let [x, y] = s.physical(x, y);
        s.pick = Some([x as u32, y as u32]);
        weak.upgrade().inspect(|a| a.window().request_redraw());
    });

    let s = state.clone();
    let weak = app.as_weak();
    app.on_globe(move |on| {
        let mut s = s.borrow_mut();
        let viewport = s.viewport;
        if let Some(c) = s.camera.as_mut() {
            c.set_globe(viewport, on);
        }
        weak.upgrade().inspect(|a| a.window().request_redraw());
    });

    let weak = app.as_weak();
    app.window().set_rendering_notifier(move |rendering, api| {
        let slint::GraphicsAPI::WGPU30 { device, queue, .. } = api else {
            return;
        };
        let Some(app) = weak.upgrade() else { return };
        let mut s = state.borrow_mut();
        match rendering {
            slint::RenderingState::RenderingSetup => {
                let fill = s.fill.take().expect("setup runs once");
                let map = Map::new(device, &fill);
                map.set_colours(queue, &modes::political(&s.provinces));
                s.map = Some(map);
            }
            slint::RenderingState::BeforeRendering => {
                s.scale_factor = f64::from(app.window().scale_factor());
                let viewport = s.physical(app.get_map_width(), app.get_map_height());
                s.viewport = viewport.map(|v| v.round().max(1.0));
                let viewport = s.viewport;
                let camera = *s
                    .camera
                    .get_or_insert_with(|| Camera::fit(viewport[0], viewport[1]));
                let pick = s.pick.take();
                let State {
                    map,
                    bench,
                    provinces,
                    ..
                } = &mut *s;
                let Some(map) = map.as_mut() else { return };
                let camera = match bench.as_mut() {
                    Some(b) => b.frame(map, queue, provinces.id.len(), viewport, camera),
                    None => camera,
                };
                if let Some(texture) =
                    map.render(device, queue, viewport.map(|v| v as u32), &camera, pick)
                {
                    match slint::Image::try_from(texture) {
                        Ok(image) => app.set_map_texture(image),
                        Err(e) => eprintln!("map texture: {e}"),
                    }
                }
                if let Some(picked) = map.poll_pick(device) {
                    app.set_status(match picked {
                        Some(p) => {
                            let i = p as usize;
                            let c = provinces.country[i] as usize;
                            format!(
                                "{} ({}), {}",
                                provinces.name[i], provinces.id[i], provinces.countries.name[c]
                            )
                            .into()
                        }
                        None => "".into(),
                    });
                } else if s.map.as_ref().is_some_and(Map::pick_pending) {
                    app.window().request_redraw();
                }
            }
            slint::RenderingState::AfterRendering => {
                if let Some(b) = s.bench.take_if(|b| b.done()) {
                    b.report();
                    let _ = slint::quit_event_loop();
                } else if s.bench.is_some() {
                    app.window().request_redraw();
                }
            }
            _ => {}
        }
    })?;
    app.run()?;
    Ok(())
}
