//! Desktop shell: loads the pack once and answers the map's least-cost queries over IPC
//! with raw little-endian f32 buffers (Section 19.1; Arrow arrives with tabular state in P2).

use std::sync::Mutex;

use ceteris_geo::graph::TransportParams;
use ceteris_geo::pack::{Pack, default_dir, read_table, u32s};
use ceteris_geo::paths::PathCache;
use tauri::ipc::Response;

struct Engine {
    pack: Pack,
    cache: PathCache,
    /// Province of each county (ADR 0009).
    county_province: Vec<u32>,
}

// Async, so path queries run off the main (UI) thread.
#[tauri::command]
async fn cost_from(
    engine: tauri::State<'_, Mutex<Engine>>,
    source: u32,
) -> Result<Response, String> {
    let mut engine = engine.lock().map_err(|e| e.to_string())?;
    let Engine { pack, cache, .. } = &mut *engine;
    let n = pack.provinces.len();
    if source as usize >= n {
        return Err(format!("no province {source}"));
    }
    Ok(Response::new(
        cache.tree(&pack.graph, source).province_bytes(n),
    ))
}

/// Least cost from `source` to each county's province as little-endian f32: a county-scale
/// payload for the map benchmark.
#[tauri::command]
async fn county_costs(
    engine: tauri::State<'_, Mutex<Engine>>,
    source: u32,
) -> Result<Response, String> {
    let mut engine = engine.lock().map_err(|e| e.to_string())?;
    let Engine {
        pack,
        cache,
        county_province,
    } = &mut *engine;
    if source as usize >= pack.provinces.len() {
        return Err(format!("no province {source}"));
    }
    let tree = cache.tree(&pack.graph, source);
    let bytes: Vec<u8> = county_province
        .iter()
        .flat_map(|&p| (tree.cost[p as usize] as f32).to_le_bytes())
        .collect();
    Ok(Response::new(bytes))
}

/// Map benchmark (`CETERIS_BENCH=1`): the page asks whether to run, results go to stdout, and
/// the app exits, since the window cannot be observed from a script.
#[tauri::command]
fn bench_requested<R: tauri::Runtime>(window: tauri::WebviewWindow<R>) -> bool {
    let on = std::env::var_os("CETERIS_BENCH").is_some();
    if on {
        // An occluded webview throttles animation frames; keep it in front while measuring.
        let _ = window.set_always_on_top(true);
        let _ = window.set_focus();
    }
    on
}

#[tauri::command]
fn bench_log(line: String) {
    println!("CETERIS_BENCH_LOG {line}");
}

#[tauri::command]
fn bench_report<R: tauri::Runtime>(app: tauri::AppHandle<R>, report: String) {
    println!("CETERIS_BENCH {report}");
    app.exit(0);
}

fn setup<R: tauri::Runtime>(
    builder: tauri::Builder<R>,
    pack: Pack,
    county_province: Vec<u32>,
) -> tauri::Builder<R> {
    builder
        .manage(Mutex::new(Engine {
            pack,
            cache: PathCache::default(),
            county_province,
        }))
        .invoke_handler(tauri::generate_handler![
            cost_from,
            county_costs,
            bench_requested,
            bench_log,
            bench_report
        ])
}

pub fn run() {
    let dir = default_dir();
    let pack = Pack::load(&dir, &TransportParams::default()).unwrap_or_else(|e| {
        panic!(
            "{e} (pack at {}; build it with `uv run python -m ceteris.packs`)",
            dir.display()
        )
    });
    let county_province = read_table(&dir.join("counties.arrow"))
        .and_then(|t| u32s(&t, "province"))
        .unwrap_or_else(|e| panic!("{e}"));
    setup(tauri::Builder::default(), pack, county_province)
        .run(tauri::generate_context!())
        .expect("error while running the Ceteris Paribus app");
}

#[cfg(test)]
mod tests {
    use super::*;
    use ceteris_geo::paths::shortest;
    use tauri::test::{INVOKE_KEY, get_ipc_response, mock_builder, mock_context, noop_assets};
    use tauri::webview::InvokeRequest;

    #[test]
    fn cost_from_returns_costs_then_hours_as_f32() {
        let dir = default_dir();
        if !dir.join("provinces.arrow").exists() {
            eprintln!("skipping: no pack at {}", dir.display());
            return;
        }
        let pack = Pack::load(&dir, &TransportParams::default()).unwrap();
        let n = pack.provinces.len();
        let expected = shortest(&pack.graph, 7);
        let app = setup(mock_builder(), pack, Vec::new())
            .build(mock_context(noop_assets()))
            .unwrap();
        let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap();
        let request = |source: u32| InvokeRequest {
            cmd: "cost_from".into(),
            callback: tauri::ipc::CallbackFn(0),
            error: tauri::ipc::CallbackFn(1),
            url: "tauri://localhost".parse().unwrap(),
            body: tauri::ipc::InvokeBody::Json(serde_json::json!({ "source": source })),
            headers: Default::default(),
            invoke_key: INVOKE_KEY.to_string(),
        };
        let body = get_ipc_response(&webview, request(7)).unwrap();
        let tauri::ipc::InvokeResponseBody::Raw(bytes) = body else {
            panic!("expected raw bytes")
        };
        assert_eq!(bytes.len(), 2 * n * 4);
        let f = |k: usize| f32::from_le_bytes(bytes[4 * k..4 * k + 4].try_into().unwrap());
        assert_eq!(f(0), expected.cost[0] as f32);
        assert_eq!(f(n), expected.hours[0] as f32);
        assert!(get_ipc_response(&webview, request(n as u32)).is_err());
    }
}
