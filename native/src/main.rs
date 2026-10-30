use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use ceteris_app::bench::Bench;
use ceteris_app::camera::Camera;
use ceteris_app::fill::Fill;
use ceteris_app::freight::{Costs, Freight};
use ceteris_app::inspect;
use ceteris_app::labels::Anchors;
use ceteris_app::map::{Layer, Map, Picked, Style};
use ceteris_app::modes::{self, Mode};
use ceteris_app::web::{Counties, Provinces};
use slint::{Color, ModelRc, VecModel};

slint::include_modules!();

/// `$CETERIS_PACK`, else `data/packs/2000` in the repository, as `ceteris_geo::pack::default_dir`.
fn pack_dir() -> PathBuf {
    std::env::var_os("CETERIS_PACK")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../data/packs/2000"))
}

fn model<T: Clone + 'static>(items: Vec<T>) -> ModelRc<T> {
    Rc::new(VecModel::from(items)).into()
}

struct State {
    /// Taken when the map uploads them.
    fills: Option<(Fill, Option<Fill>)>,
    provinces: Provinces,
    counties: Option<Counties>,
    anchors: Anchors,
    /// The camera and viewport the labels were placed for.
    labelled: Option<(Camera, [f64; 2])>,
    style: Style,
    map: Option<Map>,
    camera: Option<Camera>,
    viewport: [f64; 2],
    scale_factor: f64,
    /// The UI scale setting (ADR 0221), on top of the OS scale factor.
    ui_scale: f64,
    /// Requested and in-flight picks; `true` for a click, `false` for a hover.
    pick: Option<([u32; 2], bool)>,
    picking: Option<bool>,
    mode: Mode,
    /// Province colours waiting for the next frame's upload.
    fill: Option<Vec<[u8; 4]>>,
    selected: Option<(u32, Option<u32>)>,
    freight: Freight,
    costs: Option<Costs>,
    bench: Option<Bench>,
}

impl State {
    fn physical(&self, x: f32, y: f32) -> [f64; 2] {
        [
            f64::from(x) * self.scale_factor,
            f64::from(y) * self.scale_factor,
        ]
    }

    fn request_pick(&mut self, at: [f64; 2], click: bool) {
        // A click is never replaced by a hover.
        if click || !self.pick.is_some_and(|(_, c)| c) {
            self.pick = Some((at.map(|v| v as u32), click));
        }
    }

    fn costs(&self) -> Option<&Costs> {
        self.costs.as_ref()
    }

    /// Recolours the map for the current mode and refreshes the legend.
    fn paint(&mut self, app: &AppWindow) {
        let costs = (self.mode == Mode::Cost)
            .then(|| self.costs().map(|c| &c.cost[..]))
            .flatten();
        let painted = modes::paint(self.mode, &self.provinces, costs);
        app.set_legend_title(painted.title.into());
        app.set_legend_note(painted.note.into());
        app.set_legend(model(
            painted
                .legend
                .into_iter()
                .map(|item| {
                    let [r, g, b] = item.colour;
                    LegendItem {
                        colour: Color::from_rgb_u8(r, g, b),
                        label: item.label.into(),
                    }
                })
                .collect(),
        ));
        self.fill = Some(painted.fill);
        self.style.origin = (self.mode == Mode::Cost)
            .then(|| self.costs().map(|c| c.origin))
            .flatten();
        app.window().request_redraw();
    }

    fn inspect(&self, app: &AppWindow) {
        let Some((p, county)) = self.selected else {
            app.set_inspector_open(false);
            return;
        };
        let rows = inspect::rows(
            &self.provinces,
            p as usize,
            self.counties.as_ref(),
            county,
            self.costs(),
        );
        app.set_inspector_title(self.provinces.name[p as usize].clone().into());
        app.set_inspector(model(
            rows.into_iter()
                .map(|r| Row {
                    key: r.key.into(),
                    value: r.value.into(),
                    number: r.number,
                })
                .collect(),
        ));
        app.set_inspector_open(true);
    }

    fn picked(&mut self, app: &AppWindow, picked: Picked, click: bool) {
        if !click {
            self.style.hover = picked.province;
            app.set_status(describe(&self.provinces, self.counties.as_ref(), picked).into());
            return;
        }
        let Some(p) = picked.province else { return };
        self.selected = Some((p, picked.county));
        self.style.selected = Some(p);
        if self.mode == Mode::Cost {
            self.freight.request(p);
        }
        self.inspect(app);
    }
}

fn mode(app: &AppWindow) -> Mode {
    match app.get_mode() {
        1 => Mode::Terrain,
        // Group 0 is all groups.
        2 => Mode::Resources(usize::try_from(app.get_resource_group() - 1).ok()),
        3 => Mode::Cost,
        _ => Mode::Political,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pack = pack_dir();
    let dir = pack.join("web");
    let fill = Fill::read(&dir.join("provinces.fill"))?;
    let provinces = Provinces::read(&dir.join("provinces.json"))?;
    // Counties are optional: a pack built without them still opens.
    let county_fill = Fill::read(&dir.join("counties.fill"));
    let counties = Counties::read(&dir.join("counties.json"));
    let county_error = match (&county_fill, &counties) {
        (Ok(_), Ok(_)) => None,
        (Err(e), _) | (_, Err(e)) => Some(format!("counties not found, rebuild the pack: {e}")),
    };
    let bench = std::env::var_os("CETERIS_BENCH").map(|_| Bench::new());

    slint::BackendSelector::new()
        .require_wgpu_30(slint::wgpu_30::WGPUConfiguration::default())
        .select()?;
    let app = AppWindow::new()?;
    if let Some(e) = county_error {
        app.set_error(e.into());
    }
    let weak = app.as_weak();
    let freight = Freight::spawn(pack, provinces.id.clone(), move || {
        let weak = weak.clone();
        let _ = slint::invoke_from_event_loop(move || {
            weak.upgrade().inspect(|a| a.window().request_redraw());
        });
    });
    let state = Rc::new(RefCell::new(State {
        anchors: Anchors::new(&fill, &provinces),
        labelled: None,
        fills: Some((fill, county_fill.ok())),
        provinces,
        counties: counties.ok(),
        // The benchmark recolours counties, so it shows them.
        style: Style {
            county_fill: bench.is_some(),
            ..Style::default()
        },
        map: None,
        camera: None,
        viewport: [1.0, 1.0],
        scale_factor: 1.0,
        ui_scale: 1.0,
        pick: None,
        picking: None,
        mode: Mode::Political,
        fill: None,
        selected: None,
        freight,
        costs: None,
        bench,
    }));
    state.borrow_mut().paint(&app);

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
    for click in [true, false] {
        let s = state.clone();
        let weak = app.as_weak();
        let handler = move |x, y| {
            let mut s = s.borrow_mut();
            let at = s.physical(x, y);
            s.request_pick(at, click);
            weak.upgrade().inspect(|a| a.window().request_redraw());
        };
        if click {
            app.on_select(handler);
        } else {
            app.on_hover(handler);
        }
    }
    let s = state.clone();
    let weak = app.as_weak();
    app.on_unhover(move || {
        let Some(app) = weak.upgrade() else { return };
        s.borrow_mut().style.hover = None;
        app.set_status("".into());
        app.window().request_redraw();
    });

    let s = state.clone();
    let weak = app.as_weak();
    app.on_close_inspector(move || {
        let Some(app) = weak.upgrade() else { return };
        let mut s = s.borrow_mut();
        s.selected = None;
        s.style.selected = None;
        s.inspect(&app);
        app.window().request_redraw();
    });

    let s = state.clone();
    let weak = app.as_weak();
    app.on_mode_changed(move || {
        let Some(app) = weak.upgrade() else { return };
        let mut s = s.borrow_mut();
        s.mode = mode(&app);
        if s.mode == Mode::Cost
            && s.costs.is_none()
            && let Some((p, _)) = s.selected
        {
            s.freight.request(p);
        }
        s.paint(&app);
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

    let s = state.clone();
    let weak = app.as_weak();
    app.on_counties(move |on| {
        s.borrow_mut().style.county_borders = on;
        weak.upgrade().inspect(|a| a.window().request_redraw());
    });

    let s = state.clone();
    let weak = app.as_weak();
    app.on_ui_scale(move |scale| {
        let Some(app) = weak.upgrade() else { return };
        let mut s = s.borrow_mut();
        s.ui_scale = f64::from(scale);
        s.labelled = None;
        app.global::<Tokens>().set_scale(scale);
        app.window().request_redraw();
    });

    let weak = app.as_weak();
    app.on_dismiss_error(move || {
        weak.upgrade().inspect(|a| a.set_error("".into()));
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
                let (fill, counties) = s.fills.take().expect("setup runs once");
                s.map = Some(Map::new(
                    device,
                    queue,
                    &fill,
                    counties.as_ref(),
                    &s.provinces.country,
                ));
            }
            slint::RenderingState::BeforeRendering => {
                match s.freight.poll() {
                    Some(Ok(costs)) => {
                        s.costs = Some(costs);
                        s.paint(&app);
                        s.inspect(&app);
                    }
                    Some(Err(e)) => app.set_error(e.into()),
                    None => {}
                }
                s.scale_factor = f64::from(app.window().scale_factor());
                let viewport = s.physical(app.get_map_width(), app.get_map_height());
                s.viewport = viewport.map(|v| v.round().max(1.0));
                let viewport = s.viewport;
                let camera = *s
                    .camera
                    .get_or_insert_with(|| Camera::fit(viewport[0], viewport[1]));
                if s.labelled != Some((camera, viewport)) {
                    s.labelled = Some((camera, viewport));
                    let em = 12.0 * s.scale_factor * s.ui_scale;
                    let k = s.scale_factor as f32;
                    let labels = s
                        .anchors
                        .place(&camera, viewport, em)
                        .into_iter()
                        .map(|l| MapLabel {
                            text: l.text.into(),
                            x: l.at[0] as f32 / k,
                            y: l.at[1] as f32 / k,
                            country: l.country,
                        })
                        .collect();
                    app.set_labels(model(labels));
                }
                let State {
                    map,
                    bench,
                    style,
                    fill,
                    pick,
                    picking,
                    ..
                } = &mut *s;
                let Some(map) = map.as_mut() else { return };
                if let Some(fill) = fill.take() {
                    map.set_colours(queue, Layer::Provinces, &fill);
                }
                let camera = match bench.as_mut() {
                    Some(b) => b.frame(map, queue, viewport, camera),
                    None => camera,
                };
                let pick = if map.pick_pending() {
                    None
                } else {
                    pick.take().map(|(at, click)| {
                        *picking = Some(click);
                        at
                    })
                };
                if let Some(texture) = map.render(
                    device,
                    queue,
                    viewport.map(|v| v as u32),
                    &camera,
                    *style,
                    pick,
                ) {
                    match slint::Image::try_from(texture) {
                        Ok(image) => app.set_map_texture(image),
                        Err(e) => eprintln!("map texture: {e}"),
                    }
                }
                if let Some(picked) = map.poll_pick(device) {
                    let click = s.picking.take().unwrap_or(false);
                    s.picked(&app, picked, click);
                    app.window().request_redraw();
                } else if s.pick.is_some() || s.map.as_ref().is_some_and(Map::pick_pending) {
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

fn describe(provinces: &Provinces, counties: Option<&Counties>, picked: Picked) -> String {
    let Some(p) = picked.province.map(|p| p as usize) else {
        return String::new();
    };
    let country = &provinces.countries.name[provinces.country[p] as usize];
    let place = format!("{} ({}), {country}", provinces.name[p], provinces.id[p]);
    match (picked.county, counties) {
        (Some(c), Some(counties)) => format!("{}, {place}", counties.name[c as usize]),
        _ => place,
    }
}
