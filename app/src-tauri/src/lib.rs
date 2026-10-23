//! Desktop shell: loads the pack once and answers the map's least-cost queries over IPC
//! with raw little-endian f32 buffers (Section 19.1; Arrow arrives with tabular state in P2).

use std::sync::Mutex;

use ceteris_geo::graph::TransportParams;
use ceteris_geo::pack::{Pack, default_dir};
use ceteris_geo::paths::PathCache;
use tauri::ipc::Response;

struct Engine {
    pack: Pack,
    cache: PathCache,
}

// Async, so path queries run off the main (UI) thread.
#[tauri::command]
async fn cost_from(
    engine: tauri::State<'_, Mutex<Engine>>,
    source: u32,
) -> Result<Response, String> {
    let mut engine = engine.lock().map_err(|e| e.to_string())?;
    let Engine { pack, cache } = &mut *engine;
    let n = pack.provinces.len();
    if source as usize >= n {
        return Err(format!("no province {source}"));
    }
    Ok(Response::new(
        cache.tree(&pack.graph, source).province_bytes(n),
    ))
}

fn setup<R: tauri::Runtime>(builder: tauri::Builder<R>, pack: Pack) -> tauri::Builder<R> {
    builder
        .manage(Mutex::new(Engine {
            pack,
            cache: PathCache::default(),
        }))
        .invoke_handler(tauri::generate_handler![cost_from])
}

pub fn run() {
    let dir = default_dir();
    let pack = Pack::load(&dir, &TransportParams::default()).unwrap_or_else(|e| {
        panic!(
            "{e} (pack at {}; build it with `uv run python -m ceteris.packs`)",
            dir.display()
        )
    });
    setup(tauri::Builder::default(), pack)
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
        let app = setup(mock_builder(), pack)
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
