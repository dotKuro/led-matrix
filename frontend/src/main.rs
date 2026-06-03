use std::collections::HashMap;

use gloo_net::http::Request;
use leptos::html::Canvas;
use leptos::prelude::*;
use leptos_router::components::{A, Route, Router, Routes};
use leptos_router::path;
use serde::Deserialize;
use wasm_bindgen::prelude::*;
use wasm_bindgen::{Clamped, JsCast};
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{
    BinaryType, CanvasRenderingContext2d, File, HtmlCanvasElement, HtmlImageElement,
    HtmlInputElement, ImageData, MessageEvent, WebSocket,
};

const MATRIX_WIDTH: u32 = 32;
const MATRIX_HEIGHT: u32 = 32;

#[derive(Deserialize, Clone)]
struct Status {
    running: Option<String>,
}

#[derive(Deserialize, Clone)]
struct PatternInfo {
    name: String,
    inputs: Vec<InputSpec>,
}

#[derive(Deserialize, Clone)]
struct InputSpec {
    key: String,
    label: String,
    #[serde(rename = "type")]
    kind: String,
    default: serde_json::Value,
}

/// App-level state shared across tab switches. Patterns and their input values
/// live here so they survive PatternsPage being unmounted by the router.
#[derive(Clone, Copy)]
struct PatternsCtx {
    patterns: ReadSignal<Vec<PatternInfo>>,
    running: ReadSignal<Option<String>>,
    set_running: WriteSignal<Option<String>>,
    values: RwSignal<HashMap<String, HashMap<String, serde_json::Value>>>,
}

fn main() {
    console_error_panic_hook::set_once();
    mount_to_body(|| view! { <App /> });
}

#[component]
fn App() -> impl IntoView {
    let (patterns, set_patterns) = signal(Vec::<PatternInfo>::new());
    let (running, set_running) = signal(None::<String>);
    let values: RwSignal<HashMap<String, HashMap<String, serde_json::Value>>> =
        RwSignal::new(HashMap::new());

    // Fetch patterns once at App mount; seed values with each input's default.
    spawn_local(async move {
        if let Ok(resp) = Request::get("/api/patterns").send().await {
            if let Ok(list) = resp.json::<Vec<PatternInfo>>().await {
                let mut defaults: HashMap<String, HashMap<String, serde_json::Value>> =
                    HashMap::new();
                for info in &list {
                    let mut m = HashMap::new();
                    for inp in &info.inputs {
                        m.insert(inp.key.clone(), inp.default.clone());
                    }
                    defaults.insert(info.name.clone(), m);
                }
                values.set(defaults);
                set_patterns.set(list);
            }
        }
        if let Ok(resp) = Request::get("/api/pattern/status").send().await {
            if let Ok(status) = resp.json::<Status>().await {
                set_running.set(status.running);
            }
        }
    });

    provide_context(PatternsCtx {
        patterns,
        running,
        set_running,
        values,
    });

    view! {
        <Router>
            <header class="nav">
                <h1>"LED Matrix"</h1>
            </header>
            <main class="content">
                <div class="app-layout">
                    <div class="controls-column">
                        <nav class="tabs">
                            <A href="/" exact=true>"Patterns"</A>
                            <A href="/image">"Image"</A>
                        </nav>
                        <Routes fallback=|| view! { <p>"Not found"</p> }>
                            <Route path=path!("/") view=PatternsPage />
                            <Route path=path!("/image") view=ImagePage />
                        </Routes>
                    </div>
                    <aside class="simulator-column">
                        <div class="simulator-card">
                            <p class="label">"Live preview"</p>
                            <SimulatorView />
                        </div>
                    </aside>
                </div>
            </main>
        </Router>
    }
}

#[component]
fn PatternsPage() -> impl IntoView {
    let ctx = use_context::<PatternsCtx>().expect("PatternsCtx must be provided by <App/>");

    view! {
        <h2>"Patterns"</h2>
        <div class="pattern-grid">
            <For
                each=move || ctx.patterns.get()
                key=|info| info.name.clone()
                children=move |info| view! { <PatternCard info=info /> }
            />
        </div>
    }
}

/// Snapshot all input values for `pattern_name` into a serde JSON map.
fn current_params_for(
    values: RwSignal<HashMap<String, HashMap<String, serde_json::Value>>>,
    pattern_name: &str,
) -> serde_json::Map<String, serde_json::Value> {
    values
        .get_untracked()
        .get(pattern_name)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .collect()
}

#[component]
fn PatternCard(info: PatternInfo) -> impl IntoView {
    let ctx = use_context::<PatternsCtx>().expect("PatternsCtx must be provided by <App/>");
    let name = info.name.clone();
    let inputs = info.inputs.clone();

    let stored_name = StoredValue::new(name.clone());

    let is_running = move || {
        let n = stored_name.get_value();
        ctx.running.get().as_deref() == Some(n.as_str())
    };

    let on_click = move |_| {
        let currently_running = is_running();
        let name = stored_name.get_value();
        let params = current_params_for(ctx.values, &name);
        spawn_local(async move {
            if currently_running {
                if Request::post("/api/pattern/stop").send().await.is_ok() {
                    ctx.set_running.set(None);
                }
            } else {
                let body = serde_json::json!({ "name": &name, "params": params });
                if let Ok(req) = Request::post("/api/pattern/run").json(&body) {
                    if req.send().await.is_ok() {
                        ctx.set_running.set(Some(name));
                    }
                }
            }
        });
    };

    let name_for_inputs = name.clone();

    view! {
        <div class="pattern-card" class:running=is_running>
            <h3>{name.clone()}</h3>
            <For
                each=move || inputs.clone()
                key=|spec| spec.key.clone()
                children=move |spec| view! {
                    <PatternInput pattern_name=name_for_inputs.clone() spec=spec />
                }
            />
            <button class:danger=is_running on:click=on_click>
                {move || if is_running() { "Stop" } else { "Run" }}
            </button>
        </div>
    }
}

#[component]
fn PatternInput(pattern_name: String, spec: InputSpec) -> impl IntoView {
    let ctx = use_context::<PatternsCtx>().expect("PatternsCtx must be provided by <App/>");
    let label = spec.label.clone();
    let kind = spec.kind.clone();
    let stored_pattern = StoredValue::new(pattern_name);
    let stored_key = StoredValue::new(spec.key.clone());
    let stored_default = StoredValue::new(spec.default.clone());

    let get_value = move || -> serde_json::Value {
        let p = stored_pattern.get_value();
        let k = stored_key.get_value();
        ctx.values
            .get()
            .get(&p)
            .and_then(|m| m.get(&k))
            .cloned()
            .unwrap_or_else(|| stored_default.get_value())
    };

    let set_value = move |new_val: serde_json::Value| {
        let p = stored_pattern.get_value();
        let k = stored_key.get_value();
        ctx.values.update(|m| {
            m.entry(p.clone()).or_default().insert(k, new_val);
        });

        // Live-update: if this pattern is the running one, push the new
        // params to the backend. The pattern reads params each frame, so
        // it picks up the change without a restart.
        if ctx.running.get_untracked().as_deref() == Some(p.as_str()) {
            let params = current_params_for(ctx.values, &p);
            spawn_local(async move {
                let body = serde_json::json!({ "name": p, "params": params });
                if let Ok(req) = Request::post("/api/pattern/run").json(&body) {
                    let _ = req.send().await;
                }
            });
        }
    };

    match kind.as_str() {
        "color" => {
            let on_change = move |ev: web_sys::Event| {
                let Some(target) = ev.target() else { return };
                let Ok(input) = target.dyn_into::<HtmlInputElement>() else {
                    return;
                };
                set_value(serde_json::Value::String(input.value()));
            };
            view! {
                <div class="pattern-input">
                    <label>{label}</label>
                    <input
                        type="color"
                        prop:value=move || get_value().as_str().unwrap_or("#000000").to_string()
                        on:input=on_change
                    />
                </div>
            }
            .into_any()
        }
        _ => view! { <span></span> }.into_any(),
    }
}

#[derive(Clone, Copy)]
struct ImgInfo {
    width: u32,
    height: u32,
}

#[derive(Clone, Copy, Default, PartialEq)]
struct Crop {
    x: f64,
    y: f64,
    size: f64,
}

const VIEWPORT_MAX: f64 = 400.0;
const MIN_CROP: f64 = 16.0;

#[component]
fn ImagePage() -> impl IntoView {
    let (status, set_status) = signal(String::from("Pick an image to display."));
    let (img_url, set_img_url) = signal(None::<String>);
    let (info, set_info) = signal(None::<ImgInfo>);
    let (crop, set_crop) = signal(Crop::default());
    let (bg_color, set_bg_color) = signal(String::from("#000000"));

    // Drag tracking: (client_x_at_press, client_y_at_press, crop_at_press)
    let drag = StoredValue::new(None::<(i32, i32, Crop)>);

    let scale = move || -> f64 {
        info.get()
            .map(|i| VIEWPORT_MAX / (i.width.max(i.height) as f64))
            .unwrap_or(1.0)
    };
    let viewport_w = move || -> f64 {
        info.get()
            .map(|i| i.width as f64 * scale())
            .unwrap_or(0.0)
    };
    let viewport_h = move || -> f64 {
        info.get()
            .map(|i| i.height as f64 * scale())
            .unwrap_or(0.0)
    };

    let on_file = move |ev: web_sys::Event| {
        let Some(target) = ev.target() else { return };
        let Ok(input) = target.dyn_into::<HtmlInputElement>() else {
            return;
        };
        let Some(files) = input.files() else { return };
        let Some(file) = files.get(0) else { return };

        set_status.set("Loading...".into());
        spawn_local(async move {
            match load_image_info(file).await {
                Ok((url, w, h)) => {
                    let s = w.min(h) as f64;
                    set_crop.set(Crop {
                        x: (w as f64 - s) / 2.0,
                        y: (h as f64 - s) / 2.0,
                        size: s,
                    });
                    set_img_url.set(Some(url));
                    set_info.set(Some(ImgInfo { width: w, height: h }));
                    set_status.set("Drag to move; scroll to resize.".into());
                }
                Err(e) => set_status.set(format!("Error: {e}")),
            }
        });
    };

    let on_pointer_down = move |ev: web_sys::PointerEvent| {
        ev.prevent_default();
        if let Some(target) = ev.target().and_then(|t| t.dyn_into::<web_sys::Element>().ok()) {
            let _ = target.set_pointer_capture(ev.pointer_id());
        }
        drag.set_value(Some((ev.client_x(), ev.client_y(), crop.get_untracked())));
    };

    let on_pointer_move = move |ev: web_sys::PointerEvent| {
        let Some((sx, sy, start)) = drag.get_value() else {
            return;
        };
        let s = scale();
        if s == 0.0 {
            return;
        }
        let dx = (ev.client_x() - sx) as f64 / s;
        let dy = (ev.client_y() - sy) as f64 / s;
        if let Some(i) = info.get_untracked() {
            let max_x = (i.width as f64 - start.size).max(0.0);
            let max_y = (i.height as f64 - start.size).max(0.0);
            set_crop.set(Crop {
                x: (start.x + dx).clamp(0.0, max_x),
                y: (start.y + dy).clamp(0.0, max_y),
                size: start.size,
            });
        }
    };

    let on_pointer_up = move |_ev: web_sys::PointerEvent| {
        drag.set_value(None);
    };

    let on_wheel = move |ev: web_sys::WheelEvent| {
        ev.prevent_default();
        let Some(i) = info.get_untracked() else {
            return;
        };
        let current = crop.get_untracked();
        let factor = (ev.delta_y() * 0.002).exp();
        let max_size = i.width.min(i.height) as f64;
        let new_size = (current.size * factor).clamp(MIN_CROP, max_size);
        let cx = current.x + current.size / 2.0;
        let cy = current.y + current.size / 2.0;
        let new_x = (cx - new_size / 2.0).clamp(0.0, i.width as f64 - new_size);
        let new_y = (cy - new_size / 2.0).clamp(0.0, i.height as f64 - new_size);
        set_crop.set(Crop {
            x: new_x,
            y: new_y,
            size: new_size,
        });
    };

    let on_size_change = move |ev: web_sys::Event| {
        let Some(target) = ev.target() else { return };
        let Ok(input) = target.dyn_into::<HtmlInputElement>() else {
            return;
        };
        let Ok(new_size) = input.value().parse::<f64>() else {
            return;
        };
        let Some(i) = info.get_untracked() else { return };
        let current = crop.get_untracked();
        let cx = current.x + current.size / 2.0;
        let cy = current.y + current.size / 2.0;
        let new_x = (cx - new_size / 2.0).clamp(0.0, i.width as f64 - new_size);
        let new_y = (cy - new_size / 2.0).clamp(0.0, i.height as f64 - new_size);
        set_crop.set(Crop {
            x: new_x,
            y: new_y,
            size: new_size,
        });
    };

    let max_size = move || -> i32 {
        info.get()
            .map(|i| i.width.min(i.height) as i32)
            .unwrap_or(16)
    };

    let on_color_change = move |ev: web_sys::Event| {
        let Some(target) = ev.target() else { return };
        let Ok(input) = target.dyn_into::<HtmlInputElement>() else {
            return;
        };
        set_bg_color.set(input.value());
    };

    let on_send = move |_| {
        let Some(url) = img_url.get_untracked() else {
            return;
        };
        let c = crop.get_untracked();
        let bg = bg_color.get_untracked();
        set_status.set("Sending...".into());
        spawn_local(async move {
            match send_cropped(&url, c, &bg).await {
                Ok(()) => set_status.set("Sent to matrix.".into()),
                Err(e) => set_status.set(format!("Error: {e}")),
            }
        });
    };

    view! {
        <h2>"Upload Image"</h2>
        <input type="file" accept="image/*" on:change=on_file />

        <Show when=move || info.get().is_some()>
            <div
                class="cropper-container"
                style:width=move || format!("{}px", viewport_w())
                style:height=move || format!("{}px", viewport_h())
            >
                <img
                    src=move || img_url.get().unwrap_or_default()
                    style:width=move || format!("{}px", viewport_w())
                    style:height=move || format!("{}px", viewport_h())
                    style:background-color=move || bg_color.get()
                    draggable="false"
                />
                <div
                    class="crop-box"
                    on:pointerdown=on_pointer_down
                    on:pointermove=on_pointer_move
                    on:pointerup=on_pointer_up
                    on:wheel=on_wheel
                    style:left=move || format!("{}px", crop.get().x * scale())
                    style:top=move || format!("{}px", crop.get().y * scale())
                    style:width=move || format!("{}px", crop.get().size * scale())
                    style:height=move || format!("{}px", crop.get().size * scale())
                ></div>
            </div>
            <div class="crop-controls">
                <label>"Size"</label>
                <input
                    type="range"
                    min="16"
                    max=move || max_size().to_string()
                    prop:value=move || (crop.get().size as i32).to_string()
                    on:input=on_size_change
                />
            </div>
            <div class="crop-controls">
                <label>"Background"</label>
                <input
                    type="color"
                    prop:value=move || bg_color.get()
                    on:input=on_color_change
                />
            </div>
            <div>
                <button on:click=on_send>"Send to matrix"</button>
            </div>
        </Show>

        <p>{move || status.get()}</p>
    }
}

async fn load_image_info(file: File) -> Result<(String, u32, u32), String> {
    let url = web_sys::Url::create_object_url_with_blob(&file)
        .map_err(|e| format!("create url: {e:?}"))?;
    let img = HtmlImageElement::new().map_err(|e| format!("new image: {e:?}"))?;
    img.set_src(&url);
    JsFuture::from(img.decode())
        .await
        .map_err(|e| format!("decode: {e:?}"))?;
    Ok((url, img.natural_width(), img.natural_height()))
}

async fn send_cropped(url: &str, crop: Crop, bg_color: &str) -> Result<(), String> {
    let img = HtmlImageElement::new().map_err(|e| format!("new image: {e:?}"))?;
    img.set_src(url);
    JsFuture::from(img.decode())
        .await
        .map_err(|e| format!("decode: {e:?}"))?;

    let document = web_sys::window()
        .ok_or_else(|| "no window".to_string())?
        .document()
        .ok_or_else(|| "no document".to_string())?;
    let canvas: HtmlCanvasElement = document
        .create_element("canvas")
        .map_err(|e| format!("create canvas: {e:?}"))?
        .dyn_into()
        .map_err(|_| "not a canvas".to_string())?;
    canvas.set_width(MATRIX_WIDTH);
    canvas.set_height(MATRIX_HEIGHT);

    let ctx: CanvasRenderingContext2d = canvas
        .get_context("2d")
        .map_err(|e| format!("get context: {e:?}"))?
        .ok_or_else(|| "no 2d context".to_string())?
        .dyn_into()
        .map_err(|_| "not a 2d context".to_string())?;

    // Fill the destination with the background color first; transparent image
    // pixels will then composite over this color instead of black.
    ctx.set_fill_style_str(bg_color);
    ctx.fill_rect(0.0, 0.0, MATRIX_WIDTH as f64, MATRIX_HEIGHT as f64);

    ctx.draw_image_with_html_image_element_and_sw_and_sh_and_dx_and_dy_and_dw_and_dh(
        &img,
        crop.x,
        crop.y,
        crop.size,
        crop.size,
        0.0,
        0.0,
        MATRIX_WIDTH as f64,
        MATRIX_HEIGHT as f64,
    )
    .map_err(|e| format!("draw: {e:?}"))?;

    let data = ctx
        .get_image_data(0.0, 0.0, MATRIX_WIDTH as f64, MATRIX_HEIGHT as f64)
        .map_err(|e| format!("get image data: {e:?}"))?;
    let Clamped(rgba) = data.data();

    let mut rgb = Vec::with_capacity((MATRIX_WIDTH * MATRIX_HEIGHT * 3) as usize);
    for chunk in rgba.chunks_exact(4) {
        rgb.extend_from_slice(&chunk[..3]);
    }

    let array = js_sys::Uint8Array::from(rgb.as_slice());
    let resp = Request::post("/api/image")
        .header("Content-Type", "application/octet-stream")
        .body(array)
        .map_err(|e| format!("build req: {e}"))?
        .send()
        .await
        .map_err(|e| format!("send: {e}"))?;

    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    Ok(())
}

#[component]
fn SimulatorView() -> impl IntoView {
    let canvas_ref = NodeRef::<Canvas>::new();
    let (enabled, set_enabled) = signal(true);

    let toggle = move |_| set_enabled.update(|e| *e = !*e);

    Effect::new(move |_| {
        let Some(canvas_el) = canvas_ref.get() else {
            return;
        };
        let Ok(canvas) = (*canvas_el).clone().dyn_into::<HtmlCanvasElement>() else {
            return;
        };
        let Ok(Some(ctx)) = canvas.get_context("2d") else {
            return;
        };
        let Ok(ctx) = ctx.dyn_into::<CanvasRenderingContext2d>() else {
            return;
        };

        let width = canvas.width() as usize;
        let height = canvas.height() as usize;

        let Some(url) = websocket_url() else { return };
        let Ok(ws) = WebSocket::new(&url) else { return };
        ws.set_binary_type(BinaryType::Arraybuffer);

        let on_message = Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
            if !enabled.get_untracked() {
                return;
            }
            let Ok(buffer) = event.data().dyn_into::<js_sys::ArrayBuffer>() else {
                return;
            };
            let array = js_sys::Uint8Array::new(&buffer);
            let bytes = array.to_vec();
            if bytes.len() != width * height * 3 {
                return;
            }

            let mut rgba = Vec::with_capacity(width * height * 4);
            for chunk in bytes.chunks_exact(3) {
                rgba.extend_from_slice(&[chunk[0], chunk[1], chunk[2], 255]);
            }

            if let Ok(data) = ImageData::new_with_u8_clamped_array_and_sh(
                Clamped(&rgba),
                width as u32,
                height as u32,
            ) {
                let _ = ctx.put_image_data(&data, 0.0, 0.0);
            }
        });

        ws.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        on_message.forget();

        on_cleanup(move || {
            let _ = ws.close();
        });
    });

    view! {
        <div class="sim-canvas-wrap" class:disabled=move || !enabled.get()>
            <canvas node_ref=canvas_ref width="32" height="32" class="sim-canvas" />
            <button
                class="preview-toggle"
                on:click=toggle
                aria-label=move || if enabled.get() { "Hide preview" } else { "Show preview" }
                title=move || if enabled.get() { "Hide preview" } else { "Show preview" }
            >
                {move || if enabled.get() {
                    view! { <EyeIcon /> }.into_any()
                } else {
                    view! { <EyeOffIcon /> }.into_any()
                }}
            </button>
        </div>
    }
}

#[component]
fn EyeIcon() -> impl IntoView {
    view! {
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z" />
            <circle cx="12" cy="12" r="3" />
        </svg>
    }
}

#[component]
fn EyeOffIcon() -> impl IntoView {
    view! {
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19m-6.72-1.07a3 3 0 1 1-4.24-4.24" />
            <line x1="1" y1="1" x2="23" y2="23" />
        </svg>
    }
}

fn websocket_url() -> Option<String> {
    let location = web_sys::window()?.location();
    let proto = if location.protocol().ok()? == "https:" {
        "wss"
    } else {
        "ws"
    };
    let host = location.host().ok()?;
    Some(format!("{proto}://{host}/api/sim/stream"))
}
