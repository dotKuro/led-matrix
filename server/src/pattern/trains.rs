use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::sleep;
use std::time::{Duration, Instant};

use chrono::{DateTime, Local};
use serde_json::Value;

use super::{DisplayState, is_pattern, params_for, parse_hex_color};
use crate::matrix::{Color, Matrix};

pub const NAME: &str = "next s-bahnen";
const FRAME_DELAY: Duration = Duration::from_millis(500);
const FETCH_INTERVAL: Duration = Duration::from_secs(30);
const FETCH_RETRY: Duration = Duration::from_secs(5);
const MAX_DEPARTURES: usize = 4;

// Offenbach Marktplatz (Hessen). We filter to S-Bahn trains whose route includes
// "Frankfurt" *after* this station — i.e., heading toward Frankfurt Hbf.
const STATION_EVA: &str = "8070090";

const DEFAULT_LINE_COLOR: Color = Color::new(220, 220, 220);
const DEFAULT_LINE_HEX: &str = "#dcdcdc";
const DEFAULT_MINUTES_COLOR: Color = Color::new(120, 200, 220);
const DEFAULT_MINUTES_HEX: &str = "#78c8dc";

pub fn info() -> serde_json::Value {
    serde_json::json!({
        "name": NAME,
        "inputs": [
            { "key": "line_color", "label": "Train", "type": "color", "default": DEFAULT_LINE_HEX },
            { "key": "minutes_color", "label": "Minutes", "type": "color", "default": DEFAULT_MINUTES_HEX },
        ],
    })
}

struct Departure {
    line_digit: u8,
    minutes: i64,
}

// --- Superjson-like flat-array resolver -----------------------------------
//
// bahn.expert's tRPC response interns every value into one big array. Object
// values are integer indices back into the same array; -1 means absent.
// `arr[9]` is `false`, `arr[10]` is `0`, etc. To read a field we look up the
// referenced index and follow it; recursion unfolds the whole structure.

fn field_idx(arr: &[Value], obj_idx: usize, field: &str) -> Option<usize> {
    let i = arr.get(obj_idx)?.get(field)?.as_i64()?;
    if i < 0 {
        return None;
    }
    Some(i as usize)
}

fn at_str<'a>(arr: &'a [Value], idx: usize) -> Option<&'a str> {
    arr.get(idx)?.as_str()
}

fn at_bool(arr: &[Value], idx: usize) -> Option<bool> {
    arr.get(idx)?.as_bool()
}

fn parse_date_at(arr: &[Value], idx: usize) -> Option<DateTime<Local>> {
    let pair = arr.get(idx)?.as_array()?;
    if pair.first()?.as_str()? != "Date" {
        return None;
    }
    DateTime::parse_from_rfc3339(pair.get(1)?.as_str()?)
        .ok()
        .map(|dt| dt.with_timezone(&Local))
}

/// True if the train's `route` lists a "Frankfurt..." stop *after* our station.
fn route_goes_to_frankfurt(arr: &[Value], dep_idx: usize) -> bool {
    let Some(route_arr_idx) = field_idx(arr, dep_idx, "route") else {
        return false;
    };
    let Some(route) = arr.get(route_arr_idx).and_then(|v| v.as_array()) else {
        return false;
    };

    // Find Offenbach Marktplatz in the ordered route.
    let our_pos = route.iter().position(|stop_v| {
        let Some(stop_idx) = stop_v.as_i64().map(|n| n as usize) else {
            return false;
        };
        let Some(name_idx) = field_idx(arr, stop_idx, "name") else {
            return false;
        };
        at_str(arr, name_idx)
            .map(|n| n.contains("Offenbach") && n.contains("Marktplatz"))
            .unwrap_or(false)
    });
    let Some(our_pos) = our_pos else {
        return false;
    };

    // Anything Frankfurt-prefixed after us means we're going west toward
    // Frankfurt Hbf. Eastbound trains pass through Offenbach Ost / Bieber /
    // Waldhof / Rodgau / Hanau — none of those contain "Frankfurt".
    route.iter().skip(our_pos + 1).any(|stop_v| {
        let Some(stop_idx) = stop_v.as_i64().map(|n| n as usize) else {
            return false;
        };
        let Some(name_idx) = field_idx(arr, stop_idx, "name") else {
            return false;
        };
        at_str(arr, name_idx)
            .map(|n| n.contains("Frankfurt"))
            .unwrap_or(false)
    })
}

fn fetch_departures() -> Result<Vec<Departure>, String> {
    // tRPC superjson input: position 0 is a shape map, positions 1..3 are the
    // actual args (evaNumber, lookahead, lookbehind). The whole array is then
    // JSON-stringified once more (so the URL `input` is a quoted string).
    let input_json = serde_json::json!([
        {"evaNumber": 1, "lookahead": 2, "lookbehind": 3},
        STATION_EVA,
        150,
        10
    ]);
    let input_str = serde_json::to_string(&input_json).map_err(|e| format!("encode: {e}"))?;
    let wrapped = serde_json::to_string(&input_str).map_err(|e| format!("wrap: {e}"))?;
    let url = format!(
        "https://bahn.expert/rpc/iris.departures?input={}",
        urlencoding::encode(&wrapped)
    );

    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(20))
        .build();
    let body: Value = agent
        .get(&url)
        .call()
        .map_err(|e| format!("request: {e}"))?
        .into_json()
        .map_err(|e| format!("parse outer: {e}"))?;

    let data_str = body
        .pointer("/result/data")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "missing result.data".to_string())?;
    let data: Value =
        serde_json::from_str(data_str).map_err(|e| format!("parse inner: {e}"))?;
    let arr = data.as_array().ok_or_else(|| "expected array".to_string())?;

    let dep_list_idx = arr
        .first()
        .and_then(|v| v.get("departures"))
        .and_then(|v| v.as_i64())
        .ok_or_else(|| "missing departures key".to_string())?;
    let dep_indices = arr
        .get(dep_list_idx as usize)
        .and_then(|v| v.as_array())
        .ok_or_else(|| "expected departure index list".to_string())?;

    let now = Local::now();
    let mut deps = Vec::new();

    for di in dep_indices {
        let Some(dep_idx) = di.as_i64().map(|i| i as usize) else {
            continue;
        };

        // Line name from train.name, e.g. "S 1"
        let Some(train_idx) = field_idx(arr, dep_idx, "train") else {
            continue;
        };
        let Some(name_idx) = field_idx(arr, train_idx, "name") else {
            continue;
        };
        let Some(line_name) = at_str(arr, name_idx) else {
            continue;
        };
        let mut chars = line_name.chars().filter(|c| c.is_ascii_alphanumeric());
        if chars.next() != Some('S') {
            continue;
        }
        let Some(digit_char) = chars.next() else {
            continue;
        };
        let Some(line_digit) = digit_char.to_digit(10) else {
            continue;
        };

        // Skip cancelled trains
        if let Some(c_idx) = field_idx(arr, dep_idx, "cancelled") {
            if at_bool(arr, c_idx).unwrap_or(false) {
                continue;
            }
        }

        // Direction filter via route
        if !route_goes_to_frankfurt(arr, dep_idx) {
            continue;
        }

        // Time: prefer realtime ("time") over scheduled
        let Some(info_idx) = field_idx(arr, dep_idx, "departure") else {
            continue;
        };
        let t_idx = field_idx(arr, info_idx, "time")
            .or_else(|| field_idx(arr, info_idx, "scheduledTime"));
        let Some(t_idx) = t_idx else { continue };
        let Some(local_dt) = parse_date_at(arr, t_idx) else {
            continue;
        };

        let diff = (local_dt - now).num_minutes();
        if diff < 0 {
            continue;
        }

        deps.push(Departure {
            line_digit: line_digit as u8,
            minutes: diff,
        });
    }

    deps.sort_by_key(|d| d.minutes);
    deps.truncate(MAX_DEPARTURES);
    Ok(deps)
}

// --- Drawing --------------------------------------------------------------

// 4x6 font. 4 cols stored in the low nibble of each row byte (MSB-leftmost).
const GLYPH_W: usize = 4;
const GLYPH_H: usize = 6;

const GLYPH_S: [u8; 6] = [0b0111, 0b1000, 0b0110, 0b0001, 0b0001, 0b1110];

const DIGIT_FONT: [[u8; 6]; 10] = [
    // 0
    [0b0110, 0b1001, 0b1001, 0b1001, 0b1001, 0b0110],
    // 1
    [0b0010, 0b0110, 0b0010, 0b0010, 0b0010, 0b0111],
    // 2
    [0b0110, 0b1001, 0b0001, 0b0010, 0b0100, 0b1111],
    // 3
    [0b1110, 0b0001, 0b0110, 0b0001, 0b1001, 0b0110],
    // 4
    [0b0010, 0b0110, 0b1010, 0b1111, 0b0010, 0b0010],
    // 5
    [0b1111, 0b1000, 0b1110, 0b0001, 0b1001, 0b0110],
    // 6
    [0b0110, 0b1000, 0b1110, 0b1001, 0b1001, 0b0110],
    // 7
    [0b1111, 0b0001, 0b0010, 0b0100, 0b0100, 0b0100],
    // 8
    [0b0110, 0b1001, 0b0110, 0b1001, 0b1001, 0b0110],
    // 9
    [0b0110, 0b1001, 0b1001, 0b0111, 0b0001, 0b0110],
];

fn draw_glyph<M: Matrix>(matrix: &mut M, glyph: &[u8; 6], x: usize, y: usize, color: Color) {
    for row in 0..GLYPH_H {
        for col in 0..GLYPH_W {
            if (glyph[row] >> (GLYPH_W - 1 - col)) & 1 == 1 {
                matrix.set(x + col, y + row, color);
            }
        }
    }
}

fn draw_departure<M: Matrix>(
    matrix: &mut M,
    dep: &Departure,
    y: usize,
    line_color: Color,
    minutes_color: Color,
) {
    // Train: "S<n>" left-aligned (cols 4..12)
    draw_glyph(matrix, &GLYPH_S, 4, y, line_color);
    draw_glyph(matrix, &DIGIT_FONT[dep.line_digit as usize], 9, y, line_color);

    // Minutes: 1 or 2 digits, right-aligned (ones at 23..26, tens at 18..21)
    let mins = dep.minutes.clamp(0, 99) as u8;
    let tens = mins / 10;
    let ones = mins % 10;
    if tens > 0 {
        draw_glyph(matrix, &DIGIT_FONT[tens as usize], 18, y, minutes_color);
    }
    draw_glyph(matrix, &DIGIT_FONT[ones as usize], 23, y, minutes_color);
}

fn draw_all<M: Matrix>(
    matrix: &mut M,
    deps: &[Departure],
    line_color: Color,
    minutes_color: Color,
) {
    matrix.clear();
    for (i, dep) in deps.iter().take(MAX_DEPARTURES).enumerate() {
        // 6-tall glyphs with 2 px between rows: 6+2 = 8 rows per entry,
        // 4 entries × 8 = 32 → fills the matrix.
        let y = 1 + i * 8;
        draw_departure(matrix, dep, y, line_color, minutes_color);
    }
}

fn current_color(state: &DisplayState, key: &str, default: Color) -> Color {
    params_for(state, NAME)
        .and_then(|p| p.get(key).and_then(|v| v.as_str()).map(str::to_string))
        .and_then(|hex| parse_hex_color(&hex))
        .unwrap_or(default)
}

pub fn run<M: Matrix>(matrix: &mut M, state: &DisplayState, shutdown: &Arc<AtomicBool>) {
    let mut last_fetch: Option<Instant> = None;
    let mut next_attempt = Instant::now();
    let mut departures: Vec<Departure> = Vec::new();

    while !shutdown.load(Ordering::SeqCst) {
        if !is_pattern(state, NAME) {
            return;
        }

        let now = Instant::now();
        let due = match last_fetch {
            Some(t) => now.duration_since(t) > FETCH_INTERVAL,
            None => true,
        };
        if due && now >= next_attempt {
            match fetch_departures() {
                Ok(deps) => {
                    departures = deps;
                    last_fetch = Some(now);
                }
                Err(e) => {
                    eprintln!("trains fetch failed: {e}");
                    next_attempt = now + FETCH_RETRY;
                }
            }
        }

        let line_color = current_color(state, "line_color", DEFAULT_LINE_COLOR);
        let minutes_color = current_color(state, "minutes_color", DEFAULT_MINUTES_COLOR);
        draw_all(matrix, &departures, line_color, minutes_color);
        let _ = matrix.flush();
        sleep(FRAME_DELAY);
    }
}
