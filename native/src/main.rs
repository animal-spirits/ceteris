use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, OnceLock};
use std::time::Instant;

use ceteris_app::bench::Bench;
use ceteris_app::camera::Camera;
use ceteris_app::clock::{self, Clock};
use ceteris_app::commands::{self, Command};
use ceteris_app::econ::{self, View};
use ceteris_app::fill::Fill;
use ceteris_app::freight::{Costs, Freight};
use ceteris_app::inspect;
use ceteris_app::labels::Anchors;
use ceteris_app::map::{Layer, Map, Picked, Style};
use ceteris_app::modes::{self, Mode};
use ceteris_app::sim::Runner;
use ceteris_app::web::{Counties, Provinces};
use ceteris_core::sim::Sim;
use ceteris_econ::{EconInputs, EconParams, EconWorld};
use ceteris_trade::TradeParams;
use slint::platform::Key;
use slint::{Color, ModelRc, VecModel};

slint::include_modules!();

/// `$CETERIS_PACK`, else `data/packs/2000` in the repository, as `ceteris_geo::pack::default_dir`.
fn pack_dir() -> PathBuf {
    std::env::var_os("CETERIS_PACK")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../data/packs/2000"))
}

/// The econ world of the pack, from its calibrated steady state when the pack has one.
fn econ(dir: &Path) -> Result<Sim<EconWorld>, String> {
    let pack = ceteris_geo::pack::Pack::load(dir, &ceteris_geo::graph::TransportParams::default())
        .map_err(|e| format!("the economy did not load: {e}"))?;
    let inputs =
        EconInputs::from_pack(&pack, dir).map_err(|e| format!("the economy did not load: {e}"))?;
    Ok(EconWorld::sim(
        Arc::new(inputs),
        EconParams::default(),
        TradeParams::default(),
    ))
}

/// Requests a redraw from any thread.
fn waker(app: &AppWindow) -> impl Fn() + Send + 'static {
    let weak = app.as_weak();
    move || {
        let weak = weak.clone();
        let _ = slint::invoke_from_event_loop(move || {
            weak.upgrade().inspect(|a| a.window().request_redraw());
        });
    }
}

/// The keymap's name for a key event's text (0188).
fn key_name(text: &str) -> String {
    let named = [(Key::Space, "Space"), (Key::F5, "F5"), (Key::F10, "F10")];
    named
        .iter()
        .find(|(key, _)| text.chars().eq(std::iter::once(char::from(*key))))
        .map_or_else(|| text.to_owned(), |(_, name)| (*name).to_owned())
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
    /// Province colours waiting for the next frame's upload, for the presented tick and the
    /// next one.
    fill: Option<Vec<[u8; 4]>>,
    next_fill: Option<Vec<[u8; 4]>>,
    selected: Option<(u32, Option<u32>)>,
    freight: Freight,
    costs: Option<Costs>,
    bench: Option<Bench>,
    runner: Runner<View>,
    clock: Clock,
    /// The presented tick's view and the next one, once published.
    current: Option<View>,
    next: Option<View>,
    /// A step waiting for the next view.
    stepping: bool,
    last_frame: Option<Instant>,
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
        let painted = modes::paint(self.mode, &self.provinces, costs, self.current.as_ref());
        if self.mode.moves() {
            self.next_fill = self
                .next
                .as_ref()
                .map(|next| modes::paint(self.mode, &self.provinces, None, Some(next)).fill);
        }
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
        let mut rows = inspect::rows(
            &self.provinces,
            p as usize,
            self.counties.as_ref(),
            county,
            self.costs(),
        );
        if let Some(v) = &self.current
            && let Some(i) = v.province(p)
        {
            rows.extend(inspect::economy(v, i, &clock::date(v.tick, 0)));
        }
        app.set_inspector_title(self.provinces.name[p as usize].clone().into());
        app.set_inspector(model(
            rows.into_iter()
                .map(|r| Row {
                    key: r.key.into(),
                    value: r.value.into(),
                    number: r.number,
                    heading: r.heading,
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

impl State {
    /// Takes views from the ring, advances the clock and shows the date (ADR 0208).
    fn present(&mut self, app: &AppWindow) {
        let now = Instant::now();
        let seconds = self
            .last_frame
            .replace(now)
            .map_or(0.0, |t| now.duration_since(t).as_secs_f64());
        let mut changed = self.pull(app);
        let mut crossed = self.clock.advance(seconds, self.next.is_some());
        if self.stepping && self.next.is_some() {
            self.stepping = false;
            self.clock.step();
            crossed = true;
        }
        if crossed {
            self.current = self.next.take();
            self.pull(app);
            changed = true;
        }
        if changed && (self.mode.moves() || crossed) {
            self.paint(app);
        }
        if crossed && self.selected.is_some() {
            self.inspect(app);
        }
        self.style.mix = if self.mode.moves() && self.next.is_some() {
            self.clock.fraction as f32
        } else {
            0.0
        };
        let ready = self.current.is_some();
        app.set_running(ready);
        app.set_date(
            if ready {
                self.clock.date()
            } else {
                "loading the economy".into()
            }
            .into(),
        );
        app.set_tick(self.current.as_ref().map_or(0, |v| v.tick as i32));
        app.set_playing(self.clock.playing);
        app.set_speed(i32::from(self.clock.speed));
        app.set_sim_bound(self.clock.sim_bound);
        if self.clock.playing {
            app.window().request_redraw();
        }
    }

    /// Takes the next view from the ring if there is room for it; true if one came.
    fn pull(&mut self, app: &AppWindow) -> bool {
        if self.next.is_some() {
            return false;
        }
        match self.runner.next() {
            Some(Ok(view)) if self.current.is_none() => self.current = Some(view),
            Some(Ok(view)) => self.next = Some(view),
            Some(Err(e)) => {
                self.clock.playing = false;
                app.set_error(e.into());
                return false;
            }
            None => return false,
        }
        true
    }

    fn execute(&mut self, app: &AppWindow, cmd: Command) {
        let c = &mut self.clock;
        match cmd {
            Command::TogglePause => c.playing = !c.playing,
            Command::Run => c.playing = true,
            Command::Pause => c.playing = false,
            Command::Speed(n) => c.set_speed(n),
            Command::Faster => c.set_speed(c.speed + 1),
            Command::Slower => c.set_speed(c.speed.saturating_sub(1)),
            Command::Step => {
                c.playing = false;
                self.stepping = true;
            }
            Command::Pan(x, y) => {
                let v = self.viewport;
                if let Some(cam) = self.camera.as_mut() {
                    cam.pan(-0.1 * v[0] * f64::from(x), -0.1 * v[1] * f64::from(y));
                }
            }
            Command::ZoomIn | Command::ZoomOut => {
                let v = self.viewport;
                let factor = if cmd == Command::ZoomIn {
                    2f64.sqrt()
                } else {
                    0.5f64.sqrt()
                };
                if let Some(cam) = self.camera.as_mut() {
                    cam.zoom_at(v, [v[0] / 2.0, v[1] / 2.0], factor);
                }
            }
            Command::ZoomReset => {
                let v = self.viewport;
                if let Some(cam) = self.camera.as_mut() {
                    let globe = cam.globe;
                    *cam = Camera::fit(v[0], v[1]);
                    cam.set_globe(v, globe);
                }
            }
        }
        // Keep the frame clock from counting the paused time.
        self.last_frame = None;
        app.window().request_redraw();
    }
}

fn mode(app: &AppWindow) -> Mode {
    let good = usize::try_from(app.get_good()).unwrap_or(0);
    match app.get_mode() {
        1 => Mode::Terrain,
        // Group 0 is all groups.
        2 => Mode::Resources(usize::try_from(app.get_resource_group() - 1).ok()),
        3 => Mode::Cost,
        4 => Mode::Price(good),
        5 => Mode::Output(good),
        6 => Mode::NetExports(good),
        7 => Mode::Wage,
        8 => Mode::Unemployment,
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
    let freight = Freight::spawn(pack.clone(), provinces.id.clone(), waker(&app));
    let runner = Runner::spawn(
        4,
        move || econ(&pack),
        {
            let layout = OnceLock::new();
            move |sim| {
                let layout = layout.get_or_init(|| Arc::new(econ::Layout::new(sim.world())));
                View::new(sim, layout)
            }
        },
        waker(&app),
    );
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
        next_fill: None,
        selected: None,
        freight,
        costs: None,
        bench,
        runner,
        clock: Clock::default(),
        current: None,
        next: None,
        stepping: false,
        last_frame: None,
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

    let s = state.clone();
    let weak = app.as_weak();
    app.on_command(move |name| {
        let Some(app) = weak.upgrade() else { return };
        if let Some(cmd) = commands::by_name(&name) {
            s.borrow_mut().execute(&app, cmd);
        }
    });

    let s = state.clone();
    let weak = app.as_weak();
    app.on_key(move |text, ctrl, alt, shift| {
        let Some(app) = weak.upgrade() else {
            return false;
        };
        let chord = commands::chord(&key_name(&text), ctrl, alt, shift);
        let Some(cmd) = commands::lookup(&chord) else {
            return false;
        };
        s.borrow_mut().execute(&app, cmd);
        true
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
                s.present(&app);
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
                    // Text changes made while rendering show on the next frame.
                    app.window().request_redraw();
                }
                let State {
                    map,
                    bench,
                    style,
                    fill,
                    next_fill,
                    pick,
                    picking,
                    ..
                } = &mut *s;
                let Some(map) = map.as_mut() else { return };
                if let Some(fill) = fill.take() {
                    map.set_colours(queue, Layer::Provinces, &fill);
                }
                if let Some(fill) = next_fill.take() {
                    map.set_next_colours(queue, &fill);
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
