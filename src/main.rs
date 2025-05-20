use chrono::{DateTime, Local, TimeZone, Utc};
use serde::Deserialize;
use serde_json::from_str;
use std::{error::Error, fs, str, thread};
use zmq;

// Add plotters
use plotters::prelude::*;


/// The directory where we will save our chart images.
/// Modify this path as needed.
const OUTPUT_DIR: &str = "/home/user/python-scripts/services/charts";

// ─── Data Structures ────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ChartRequest(
    #[serde(rename = "0")] pub String,
    #[serde(rename = "1")] pub String,
    #[serde(rename = "2")] pub ChartData,
);

#[derive(Debug, Deserialize, Clone)]
pub struct ChartData {
    pub title: String,
    pub ticker: String,
    pub timeframe: String,
    /// Columns describing the data. Typically something like:
    /// `["timestamp", "open", "high", "low", "close", "volume"]`
    pub cols: Vec<String>,
    /// Each inner `Vec<f64>` is a row of candle data: [timestamp_millis, open, high, low, close, volume]
    pub data: Vec<Vec<f64>>,
    /// Colors for each candle, e.g. `["#FF0000", "#00FF00", ...]`
    pub candle_colors: Vec<String>,
    pub plots: Plots,
    pub desc: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Plots {
    pub marks: Vec<serde_json::Value>,
}

// ─── Main Logic ─────────────────────────────────────────────────────────────────

fn main() -> Result<(), Box<dyn Error>> {
    let context = zmq::Context::new();
    let socket = context.socket(zmq::DEALER)?;
    socket.set_identity(b"rustcharts")?;
    let endpoint = "tcp://127.0.0.1:6565";
    println!("[INIT] Connecting to {} as 'rustcharts'…", endpoint);
    socket.connect(endpoint)?;

    println!("[READY] Awaiting incoming chart messages…");

    loop {
        let frames = socket.recv_multipart(0)?;
        let now = Local::now().format("%Y-%m-%d %H:%M:%S");

        match frames.get(1).and_then(|f| str::from_utf8(f).ok()) {
            Some(json_str) => {
                match from_str::<ChartRequest>(json_str) {
                    Ok(req) => {
                        println!(
                            "[{}] ▶ New Chart Request for {} @ {} [{} candles]",
                            now,
                            req.2.ticker,
                            req.2.timeframe,
                            req.2.data.len()
                        );
                        log_data_summary(&req.2);

                        // Spawn a thread for chart generation
                        let chart_data = req.2.clone();
                        thread::spawn(move || {
                            handle_chart_request(chart_data);
                        });
                    }
                    Err(e) => {
                        eprintln!("[{}] ✘ Failed to parse ChartRequest: {}", now, e);
                    }
                }
            }
            None => {
                eprintln!("[{}] ✘ Received invalid or missing JSON payload", now);
            }
        }
    }
}

// ─── Utility: Print Summary ─────────────────────────────────────────────────────

fn log_data_summary(data: &ChartData) {
    if let (Some(first), Some(last)) = (data.data.first(), data.data.last()) {
        let start_ts = first[0] as i64;
        let end_ts = last[0] as i64;
        let start_dt: DateTime<Local> =
            DateTime::<Utc>::from(Utc.timestamp_millis_opt(start_ts).unwrap())
                .with_timezone(&Local);
        let end_dt: DateTime<Local> =
            DateTime::<Utc>::from(Utc.timestamp_millis_opt(end_ts).unwrap())
                .with_timezone(&Local);

        println!(
            "       {} candles from {} to {}",
            data.data.len(),
            start_dt.format("%Y-%m-%d %H:%M:%S"),
            end_dt.format("%Y-%m-%d %H:%M:%S"),
        );
        println!("       Columns: {:?}", data.cols);
        println!("       Title: {}", data.title);
        println!("       Desc: {}", data.desc);
    } else {
        println!("       No candle data available.");
    }
}

// ─── Actual Chart Handler with Plotters ─────────────────────────────────────────

fn handle_chart_request(data: ChartData) {
    let now = Local::now().format("%Y-%m-%d %H:%M:%S");
    println!(
        "[{}] 🖼️  Processing chart: '{}' with {} candles",
        now,
        data.title,
        data.data.len()
    );

    // If there's no data, nothing to do
    if data.data.is_empty() {
        eprintln!("No data found for chart: {}", data.title);
        return;
    }

    // Ensure OUTPUT_DIR exists
    fs::create_dir_all(OUTPUT_DIR).ok();

    // We'll build a file name using the ticker + timeframe + ".png"
    let file_path = format!("{}/{}_{}.png", OUTPUT_DIR, data.ticker, data.timeframe);

    // --- 1) Parse Timestamps and OHLCV; find min & max for Y ---
    let mut min_price = f64::MAX;
    let mut max_price = f64::MIN;
    let mut max_volume = 0.0;

    // Convert timestamps to Local DateTime for the range
    let candle_count = data.data.len();
    let first_ts = data.data[0][0] as i64;
    let last_ts = data.data[candle_count - 1][0] as i64;

    let start_dt: DateTime<Local> =
        DateTime::<Utc>::from(Utc.timestamp_millis_opt(first_ts).unwrap())
            .with_timezone(&Local);
    let end_dt: DateTime<Local> =
        DateTime::<Utc>::from(Utc.timestamp_millis_opt(last_ts).unwrap())
            .with_timezone(&Local);

    // We will store the data in a vector of (DateTime<Local>, open, high, low, close, volume, color_hex)
    let mut processed_data = Vec::with_capacity(candle_count);

    for (i, row) in data.data.iter().enumerate() {
        // row: [ts, open, high, low, close, volume] (assuming exactly that structure)
        let ts = row[0] as i64;
        let o = row[1].max(1e-12);
        let h = row[2].max(1e-12);
        let l = row[3].max(1e-12);
        let c = row[4].max(1e-12);
        let v = row.get(5).cloned().unwrap_or(0.0);

        let local_min = o.min(h).min(l).min(c);
        let local_max = o.max(h).max(l).max(c);
        if local_min < min_price {
            min_price = local_min;
        }
        if local_max > max_price {
            max_price = local_max;
        }
        if v > max_volume {
            max_volume = v;
        }

        let dt_local: DateTime<Local> =
            DateTime::<Utc>::from(Utc.timestamp_millis_opt(ts).unwrap())
                .with_timezone(&Local);

        // If for some reason we have fewer colors than candles, fallback to black
        let color_hex = data
            .candle_colors
            .get(i)
            .cloned()
            .unwrap_or_else(|| "#000000".to_string());

        processed_data.push((dt_local, o, h, l, c, v, color_hex));
    }

    // Find highest and lowest price for reference
    let mut highest_price = 0.0;
    let mut lowest_price = f64::MAX;
    for (_, _, h, l, _, _, _) in &processed_data {
        if *h > highest_price {
            highest_price = *h;
        }
        if *l < lowest_price {
            lowest_price = *l;
        }
    }

    let log_highest = highest_price.ln();
    let log_lowest = lowest_price.ln();

    let min_log = lowest_price.ln();
    let max_log = highest_price.ln();

    // Add a bit of padding to the max price (0.2%) to ensure highest candle is visible
    let padding_factor = 1.002;
    let padded_max_price = highest_price * padding_factor;
    let padded_max_log = padded_max_price.ln();

    let min_log_for_chart = min_log;
    let max_log_for_chart = padded_max_log;

    let plot_width = 1024;
    let plot_height = 768;
    let root_area = BitMapBackend::new(&file_path, (plot_width, plot_height)).into_drawing_area();
    root_area.fill(&WHITE).unwrap();

    let effective_chart_area_width = plot_width as f64 * 0.85;
    let num_candles = processed_data.len();

    let pixel_gap_between_candles = 1.0;
    let candles_to_fit = num_candles as f64;

    let total_gap_space = pixel_gap_between_candles * (candles_to_fit - 1.0);
    let available_width = effective_chart_area_width - total_gap_space;
    let candle_width_pixels = (available_width / candles_to_fit).floor();

    // Tighter right edge in time
    let last_candle_time = processed_data
        .last()
        .map(|(dt, _, _, _, _, _, _)| *dt)
        .unwrap_or(end_dt);
    let tight_end_dt = last_candle_time + chrono::Duration::seconds(1);
    let total_time_span = tight_end_dt.timestamp_millis() - start_dt.timestamp_millis();
    let millis_per_pixel = total_time_span as f64 / effective_chart_area_width;

    let volume_visible_bottom = min_log_for_chart;
    let volume_visible_top = min_log_for_chart + (0.15 * (max_log_for_chart - min_log_for_chart));

    let log_to_price = |log_val: f64| -> f64 { log_val.exp() };
    let volume_to_log_scale = |vol: f64| -> f64 {
        if max_volume <= 0.0 {
            return volume_visible_bottom;
        }
        let normalized_vol = vol / max_volume;
        volume_visible_bottom + (normalized_vol * (volume_visible_top - volume_visible_bottom))
    };

    let mut chart_context = ChartBuilder::on(&root_area)
        .margin(10)
        .x_label_area_size(80)
        .y_label_area_size(0)
        .right_y_label_area_size(60)
        .caption(data.title.clone(), ("sans-serif", 20))
        .build_cartesian_2d(start_dt..end_dt, min_log_for_chart..max_log_for_chart)
        .unwrap();

    chart_context
        .configure_mesh()
        .light_line_style(&RGBColor(235, 235, 235))
        .axis_style(&RGBColor(150, 150, 150))
        .x_labels(16)
        .y_labels(8)
        .disable_mesh()
        .x_label_formatter(&|x| x.format("%m-%d %H:%M").to_string())
        .x_label_style(TextStyle::from(("sans-serif", 14)).transform(FontTransform::Rotate90))
        .y_label_style(("sans-serif", 15))
        .y_label_formatter(&|y| {
            let actual_price = log_to_price(*y);
            let rounded_price = if actual_price >= 100000.0 {
                (actual_price / 500.0).round() * 500.0
            } else if actual_price >= 10000.0 {
                (actual_price / 100.0).round() * 100.0
            } else if actual_price >= 1000.0 {
                (actual_price / 50.0).round() * 50.0
            } else {
                (actual_price / 10.0).round() * 10.0
            };
            let price_int = rounded_price as i64;
            let formatted = format!("{}", price_int)
                .as_bytes()
                .rchunks(3)
                .rev()
                .map(std::str::from_utf8)
                .collect::<Result<Vec<&str>, _>>()
                .unwrap()
                .join(",");
            format!("${}", formatted)
        })
        .y_desc("Price")
        .draw()
        .unwrap();

    // Add some horizontal grid lines
    let y_step = (max_log_for_chart - min_log_for_chart) / 8.0;
    for i in 0..17 {
        let y_pos = min_log_for_chart + (y_step * (i as f64 / 2.0));
        let line_style = if i % 2 == 0 {
            RGBColor(235, 235, 235).stroke_width(1)
        } else {
            RGBColor(240, 240, 240).stroke_width(1)
        };
        chart_context
            .draw_series(std::iter::once(PathElement::new(
                vec![(start_dt, y_pos), (end_dt, y_pos)],
                line_style,
            )))
            .unwrap();
    }

    // Add a few vertical grid lines
    let x_range = end_dt.timestamp() - start_dt.timestamp();
    let x_step = x_range / 5;
    for i in 0..6 {
        let x_pos = start_dt + chrono::Duration::seconds(x_step * i);
        chart_context
            .draw_series(std::iter::once(PathElement::new(
                vec![(x_pos, min_log_for_chart), (x_pos, max_log_for_chart)],
                RGBColor(245, 245, 245).stroke_width(1),
            )))
            .unwrap();
    }

    // --- Volume bars (draw behind candles) ---
    chart_context
        .draw_series(
            processed_data
                .iter()
                .enumerate()
                .map(|(idx, (_dt, _o, _h, _l, _c, v, _color_hex))| {
                    let candle_left_edge_pixel =
                        idx as f64 * (candle_width_pixels + pixel_gap_between_candles);
                    let candle_right_edge_pixel = candle_left_edge_pixel + candle_width_pixels;

                    let left_pos_millis = (candle_left_edge_pixel * millis_per_pixel) as i64;
                    let right_pos_millis = (candle_right_edge_pixel * millis_per_pixel) as i64;

                    let x0 = start_dt + chrono::Duration::milliseconds(left_pos_millis);
                    let x1 = start_dt + chrono::Duration::milliseconds(right_pos_millis);

                    let y_bottom = volume_visible_bottom;
                    let y_top = volume_to_log_scale(*v);

                    let volume_color = RGBColor(130, 130, 130);
                    Rectangle::new(
                        [(x0, y_bottom), (x1, y_top)],
                        volume_color.mix(0.8).filled(),
                    )
                }),
        )
        .unwrap();

    // Sort data by timestamp to ensure correct order for candle drawing
    processed_data.sort_by(|a, b| a.0.cmp(&b.0));

    // Log some details
    let format_with_commas = |price: f64| -> String {
        let price_int = price.round() as i64;
        format!("{}", price_int)
            .as_bytes()
            .rchunks(3)
            .rev()
            .map(std::str::from_utf8)
            .collect::<Result<Vec<&str>, _>>()
            .unwrap()
            .join(",")
    };

    println!(
        "Price range: ${} - ${}",
        format_with_commas(lowest_price),
        format_with_commas(highest_price)
    );
    println!("Log price range: {:.2} - {:.2}", log_lowest, log_highest);
    println!("Candle rendering details:");
    println!("  - Number of candles: {}", num_candles);
    println!("  - Fitting space for {} candles (no extra space)", candles_to_fit);
    println!(
        "  - Available chart width: {:.1} pixels",
        effective_chart_area_width
    );
    println!("  - Candle width: {:.1} pixels", candle_width_pixels);
    println!(
        "  - Gap between candles: {:.0} pixels (fixed)",
        pixel_gap_between_candles
    );
    println!(
        "  - Log price range: {:.2} - {:.2} (Y-axis shows these log values)",
        log_lowest, log_highest
    );

    // --- Draw the dotted line for current price on the last candle ---
    let last_candle = processed_data.last().cloned();
    let current_price = last_candle.as_ref().map(|(_, _, _, _, c, _, _)| *c).unwrap_or(0.0);
    let current_price_log = current_price.ln();

    let (is_green, last_candle_color) = if let Some((_, o, _, _, c, _, _)) = last_candle {
        let is_up = c >= o;
        if is_up {
            (true, RGBColor(0, 150, 0))
        } else {
            (false, RGBColor(180, 0, 0))
        }
    } else {
        (true, RGBColor(0, 150, 0))
    };

    let formatted_current_price = format_with_commas(current_price);

    chart_context
        .draw_series(std::iter::once(PathElement::new(
            vec![(start_dt, current_price_log), (end_dt, current_price_log)],
            last_candle_color
                .stroke_width(1)
                // Use stroke style directly without dasharray
        )))
        .unwrap()
        .label(format!("Current Price: ${}", formatted_current_price));

    // Draw black background box for that label
    let text_width = formatted_current_price.len() as f64 * 10.0;
    let padding_x = 8.0;
    let padding_y = 0.006;

    let rect_x0 = end_dt - chrono::Duration::milliseconds((text_width + padding_x * 2.0) as i64);
    let rect_x1 = end_dt;
    let rect_y0 = current_price_log - padding_y;
    let rect_y1 = current_price_log + padding_y;

    chart_context
        .draw_series(std::iter::once(Rectangle::new(
            [(rect_x0, rect_y0), (rect_x1, rect_y1)],
            BLACK.filled(),
        )))
        .unwrap();

    chart_context
        .draw_series(std::iter::once(Text::new(
            format!("${}", formatted_current_price),
            (
                rect_x0 + chrono::Duration::milliseconds(padding_x as i64),
                current_price_log,
            ),
            ("sans-serif", 15).into_font().color(&WHITE),
        )))
        .unwrap();

    // --- Draw the candlestick bodies (no wicks) with consistent spacing ---
    chart_context
        .draw_series(
            processed_data
                .iter()
                .enumerate()
                .map(|(idx, (dt, o, h, l, c, _v, color_hex))| {
                    let open_log = o.ln();
                    let close_log = c.ln();

                    // Parse the color from hex
                    let txt = color_hex.trim_start_matches('#');
                    let rgb = u32::from_str_radix(txt, 16).unwrap_or(0);
                    let r = ((rgb >> 16) & 0xFF) as u8;
                    let g = ((rgb >> 8) & 0xFF) as u8;
                    let b = (rgb & 0xFF) as u8;
                    let candle_color = RGBColor(r, g, b);

                    let (body_top, body_bottom) = if open_log <= close_log {
                        (close_log, open_log)
                    } else {
                        (open_log, close_log)
                    };

                    // Compute left/right time for the candle based on pixel spacing
                    let candle_left_edge_pixel =
                        idx as f64 * (candle_width_pixels + pixel_gap_between_candles);
                    let candle_right_edge_pixel = candle_left_edge_pixel + candle_width_pixels;

                    let left_pos_millis = (candle_left_edge_pixel * millis_per_pixel) as i64;
                    let right_pos_millis = (candle_right_edge_pixel * millis_per_pixel) as i64;

                    let body_left = start_dt + chrono::Duration::milliseconds(left_pos_millis);
                    let body_right = start_dt + chrono::Duration::milliseconds(right_pos_millis);

                    // Debug for the first few candles
                    if idx < 3 {
                        println!("  - Candle #{} body: {} to {}", idx, body_left, body_right);
                        println!("  - Candle #{} actual timestamp: {}", idx, dt);
                        println!("  - Candle #{} width: {} pixels", idx, candle_width_pixels);
                        println!(
                            "  - Candle #{} O/C: ${:.2}/${:.2}, H/L: ${:.2}/${:.2}",
                            idx, o, c, h, l
                        );
                    }

                    Rectangle::new(
                        [(body_left, body_top), (body_right, body_bottom)],
                        candle_color.filled(),
                    )
                }),
        )
        .unwrap();

    // Present and save the result
    root_area.present().unwrap();

    println!(
        "[{}] ✅ Chart '{}' processing complete. Saved to: {}",
        now, data.title, file_path
    );
}

/// A small helper extension for converting a string hex code into a `ShapeStyle`.
/// This is not in the standard plotters library, so we do a quick parse:
pub trait PaletteColor {
    fn pick_from_hex<S: AsRef<str>>(hex_str: S) -> Self;
}

impl PaletteColor for ShapeStyle {
    fn pick_from_hex<S: AsRef<str>>(hex_str: S) -> Self {
        let txt = hex_str.as_ref().trim_start_matches('#');
        if txt.len() == 6 {
            if let Ok(rgb) = u32::from_str_radix(txt, 16) {
                let r = ((rgb >> 16) & 0xFF) as u8;
                let g = ((rgb >> 8) & 0xFF) as u8;
                let b = (rgb & 0xFF) as u8;
                let color = RGBColor(r, g, b);
                return ShapeStyle::from(&color);
            }
        }
        // Fallback: black
        ShapeStyle::from(&BLACK)
    }
}
