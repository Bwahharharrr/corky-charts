use chrono::{DateTime, Local, TimeZone, Utc};
use serde::Deserialize;
use serde_json::from_str;
use std::{error::Error, fs, str, thread};
use std::process::exit;
use zmq;
use colored::*;

// Add plotters
use plotters::prelude::*;
use plotters::style::{RGBColor, TextStyle, Color, IntoFont};

/// Configuration structure for the charts section of the config file
#[derive(Debug, Deserialize)]
struct ChartsConfig {
    directory: Option<String>,
}

/// Overall configuration structure
#[derive(Debug, Deserialize)]
struct Config {
    charts: Option<ChartsConfig>,
}

/// Gets the output directory from the configuration file
/// Returns an error if the directory is not specified in the config
fn get_output_directory() -> Result<String, Box<dyn Error>> {
    // Get the home directory using the dirs crate
    let home_dir = dirs::home_dir().ok_or("Could not find home directory")?;
    let config_path = home_dir.join(".corky").join("config.toml");
    
    // Check if the config file exists
    if !config_path.exists() {
        return Err("Config file ~/.corky/config.toml not found".into());
    }
    
    // Read and parse the TOML file
    let config_content = fs::read_to_string(config_path)?;
    let config: Config = toml::from_str(&config_content)?;
    
    // Check if the charts section exists and has a directory
    match config.charts {
        Some(charts_config) => {
            match charts_config.directory {
                Some(dir) => Ok(dir),
                None => Err("Output directory not specified in [charts] section of config.toml".into())
            }
        },
        None => Err("[charts] section not found in config.toml".into())
    }
}

// ─── Data Structures ────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ChartRequest(
    #[serde(rename = "0")] pub String,
    #[serde(rename = "1")] pub String,
    #[serde(rename = "2")] pub ChartData,
);

fn parse_hex_color(hex: &str) -> RGBColor {
    let hex = hex.trim_start_matches('#');
    if hex.len() == 6 {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&hex[0..2], 16),
            u8::from_str_radix(&hex[2..4], 16),
            u8::from_str_radix(&hex[4..6], 16),
        ) {
            return RGBColor(r, g, b);
        }
    }
    // Default to gray if parsing fails
    RGBColor(128, 128, 128)
}

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
    /// Optional colors for each volume bar, e.g. `["#FF0000", "#00FF00", ...]`
    #[serde(default)]
    pub volume_colors: Option<Vec<String>>,
    pub plots: Plots,
    pub desc: String,
    /// Optional chat ID for telegram message
    #[serde(default)]
    pub chat_id: Option<i64>,
    /// Optional subscriber list name for telegram message
    #[serde(default)]
    pub subscriber_list: Option<String>,
}

/// Marker to be drawn on the chart (e.g., signal indicators)
#[derive(Debug, Deserialize, Clone)]
pub struct Mark {
    /// Timestamp in milliseconds (candle timestamp)
    pub time: i64,
    /// "above" or "below" the candle
    pub position: String,
    /// Hex color "#RRGGBB"
    pub color: String,
    /// Optional label text (e.g., "4h")
    #[serde(default)]
    pub text: Option<String>,
    /// Relative size (default 1.0)
    #[serde(default = "default_mark_size")]
    pub size: f64,
}

fn default_mark_size() -> f64 {
    1.0
}

#[derive(Debug, Deserialize, Clone)]
pub struct Plots {
    #[serde(default)]
    pub marks: Vec<Mark>,
}

// ─── Main Logic ─────────────────────────────────────────────────────────────────

fn main() -> Result<(), Box<dyn Error>> {
    // Get the output directory from config file
    // If it fails, print the error message and exit
    let output_dir = match get_output_directory() {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!("ERROR: {}", e);
            exit(1);
        }
    };
    
    println!("{} {}", "[INIT]".blue().bold(), format!("Using output directory: {}", output_dir).white());
    
    let context = zmq::Context::new();
    let socket = context.socket(zmq::DEALER)?;
    socket.set_identity(b"rustcharts")?;
    let endpoint = "tcp://127.0.0.1:6565";
    println!("{} {}", "[INIT]".blue().bold(), format!("Connecting to {} as 'rustcharts'…", endpoint).white());
    socket.connect(endpoint)?;

    println!("{} {}", "[READY]".green().bold(), "Awaiting incoming chart messages…".white());

    loop {
        let frames = socket.recv_multipart(0)?;
        let now = Local::now().format("%Y-%m-%d %H:%M:%S");

        match frames.get(1).and_then(|f| str::from_utf8(f).ok()) {
            Some(json_str) => {
                match from_str::<ChartRequest>(json_str) {
                    Ok(req) => {
                        // Add a yellow separator line before each new chart request
                        println!("{} {}", 
                            "╔".yellow(), 
                            "══════════════════════════════════════════════════════════════════════".yellow()
                        );
                        
                        println!(
                            "{} {} {}",
                            format!("[{}]", now).dimmed(),
                            "▶".cyan().bold(),
                            format!("New Chart Request for {} @ {} [{} candles]", 
                                req.2.ticker.bold().yellow(), 
                                req.2.timeframe.bold(), 
                                req.2.data.len().to_string().bold().green()
                            )
                        );
                        log_data_summary(&req.2);

                        // Spawn a thread for chart generation and pass the output directory
                        let chart_data = req.2.clone();
                        let output_dir_clone = output_dir.clone();
                        thread::spawn(move || {
                            handle_chart_request(chart_data, &output_dir_clone);
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
        println!("       Desc: {}", data.desc);
    } else {
        println!("       No candle data available.");
    }
}

// ─── Actual Chart Handler with Plotters ─────────────────────────────────────────

fn handle_chart_request(data: ChartData, output_dir: &str) {
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

    // Ensure output directory exists
    fs::create_dir_all(output_dir).ok();

    // We'll build a file name using the ticker + timeframe + ".png"
    let file_path = format!("{}/{}_{}.png", output_dir, data.ticker, data.timeframe);

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

    // Increase plot dimensions for a larger chart
    let plot_width = 1280;
    let plot_height = 960;
    let root_area = BitMapBackend::new(&file_path, (plot_width, plot_height)).into_drawing_area();
    root_area.fill(&WHITE).unwrap();
    
    // Log price range in a clean format
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
        "{} ${} - ${}",
        "Price range:".dimmed(),
        format_with_commas(lowest_price).bold().green(),
        format_with_commas(highest_price).bold().green()
    );
    
    // Allocate more height for the table area and include title space
    let title_height = 40; // Dedicated space for the title
    let table_height = 100; // More height for the table
    let header_height = title_height + table_height;
    
    // Split the drawing area into three parts: title, table, and chart
    let (header_area, chart_area) = root_area.split_vertically(header_height);
    let (title_area, table_area) = header_area.split_vertically(title_height);
    
    // Apply horizontal margin to the table area (inset from left and right)
    let margin_percent = 0.15; // 15% margin on each side
    let left_margin = (plot_width as f64 * margin_percent) as u32;
    let right_margin = plot_width - (plot_width as f64 * margin_percent) as u32;
    // Split into left margin, table area, and right margin
    let (left_area, rest) = table_area.split_horizontally(left_margin);
    let (table_area, right_area) = rest.split_horizontally(right_margin - left_margin);
    
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
    // Instead of calculating the exact milliseconds per pixel (which can cause overflow),
    // we'll use a safer approach with relative positioning
    let total_time_span_millis = tight_end_dt.timestamp_millis() - start_dt.timestamp_millis();
    
    // Safety check to avoid potential overflow
    if total_time_span_millis <= 0 || total_time_span_millis > i64::MAX / 2 {
        eprintln!("Warning: Time span is too large or invalid, adjusting calculations");
    }
    
    // Use f64 for all calculations to avoid integer overflow
    let millis_per_pixel = total_time_span_millis as f64 / effective_chart_area_width;

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

    // Create a narrower time range for the chart to avoid datetime calculations that cause integer overflow
    // The issue is in the Plotters library's datetime range handling when dealing with large time spans
    
    // Instead of using the full DateTime objects directly, we'll create a modified time range
    // that uses smaller time units and won't trigger the overflow in Plotters' internal calculations
    
    // Create a simpler time range - use milliseconds since start as the x-axis unit
    // This avoids any potential issues with the Plotters library's datetime handling
    let millis_since_start = |dt: DateTime<Local>| -> i64 {
        dt.timestamp_millis() - start_dt.timestamp_millis()
    };
    
    // Calculate millisecond values for start and end points
    let start_millis = 0; // 0 milliseconds since start
    let end_millis = millis_since_start(end_dt);
    
    // Calculate the duration of one candle in milliseconds
    let total_candles = processed_data.len() as f64;
    let candle_duration_ms = (end_millis - start_millis) as f64 / total_candles;
    
    // Add 3 candles worth of space to the end
    let padded_end_millis = end_millis as f64 + (candle_duration_ms * 3.0);
    
    println!("  - Time range converted to milliseconds: {} to {} ms (with padding: {} ms)", 
        start_millis, end_millis, padded_end_millis);
    
    // Build the chart using milliseconds since start instead of DateTime objects or hours
    // Move price axis to right side as requested and maximize vertical space
    let mut chart_context = ChartBuilder::on(&chart_area) // Use chart_area instead of root_area
        // Reduce margins to maximize plot space
        .margin(10) // Keep top/left/right margin small
        .margin_bottom(20) // Significantly reduce bottom margin (was 60)
        // Remove left label area and move to right side
        .set_label_area_size(LabelAreaPosition::Left, 0)
        .set_label_area_size(LabelAreaPosition::Right, 80) // Make right side wider for price labels
        .set_label_area_size(LabelAreaPosition::Bottom, 40) // Reduced from 60
        .build_cartesian_2d((start_millis as f64)..padded_end_millis, min_log_for_chart..max_log_for_chart)
        .unwrap();
        
    // Title is centered in its own dedicated area at the very top of the canvas
    // Need to center it properly on the full canvas width
    let title_style = TextStyle::from(("sans-serif", 24))
        .color(&BLACK);
        
    // Calculate the exact center of the entire canvas width (not just title area)
    let title_pos = ((plot_width/2) as i32, title_height/2); // Center X coordinate based on full width
    
    // Draw a white background for the title area
    title_area.fill(&WHITE).unwrap();
    
    // Draw the title text - we need to use draw_text with horizontal alignment
    // to ensure it's properly centered, taking into account the Y-axis width
    let text_width = data.title.len() as i32 * 15; // Estimate text width
    let y_axis_width = 80; // The width of the Y-axis on the right side
    
    // Add a shift to compensate for the Y-axis on the right
    // We shift by y_axis_width/2 to center the title on the plot area, not including the y-axis
    let centered_x = (plot_width as i32 / 2) - (text_width / 2) + (y_axis_width / 2);
    
    title_area.draw_text(
        &data.title,
        &title_style,
        (centered_x, title_height/2), // Adjusted position to account for Y-axis
    ).unwrap();
    
    // Clear the table area with white before we begin
    table_area.fill(&WHITE).unwrap();
        
    // Create a formatter to convert milliseconds back to readable dates
    let millis_to_datetime = |millis: &f64| -> String {
        let dt = start_dt + chrono::Duration::milliseconds(*millis as i64);
        dt.format("%m-%d %H:%M").to_string()
    };
    
    chart_context
        .configure_mesh()
        .light_line_style(&RGBColor(235, 235, 235))
        .axis_style(&RGBColor(150, 150, 150))
        .x_labels(16)
        .x_label_formatter(&millis_to_datetime)
        .y_labels(8)
        .disable_mesh()
        .x_label_style(TextStyle::from(("sans-serif", 12)))
        .y_label_style(("sans-serif", 15))
        .y_desc("Price")
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
                vec![(start_millis as f64, y_pos), (end_millis as f64, y_pos)],
                line_style,
            )))
            .unwrap();
    }

    // Add a few vertical grid lines (using hours-based coordinates)
    let x_range = (end_millis as f64) - (start_millis as f64);
    let x_step = x_range / 5.0;
    for i in 0..6 {
        let x_pos = (start_millis as f64) + (x_step * i as f64);
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
                .map(|(idx, (dt, _o, _h, _l, _c, v, _color_hex))| {
                    // Use the same hours-based approach as for candlesticks
                    let dt_hours = millis_since_start(*dt) as f64;
                    let candle_width_in_hours = ((end_millis as f64) - (start_millis as f64)) / processed_data.len() as f64 * 0.8;
                    
                    let x0 = dt_hours - (candle_width_in_hours / 2.0);
                    let x1 = dt_hours + (candle_width_in_hours / 2.0);

                    let y_bottom = volume_visible_bottom;
                    let y_top = volume_to_log_scale(*v);

                    // Use volume color if available, otherwise default to gray
                    let volume_color = data.volume_colors
                        .as_ref()
                        .and_then(|colors| colors.get(idx).cloned())
                        .map(|color| parse_hex_color(&color))
                        .unwrap_or_else(|| RGBColor(130, 130, 130));

                    Rectangle::new(
                        [(x0, y_bottom), (x1, y_top)],
                        volume_color.mix(0.8).filled(),
                    )
                }),
        )
        .unwrap();

    // Sort data by timestamp to ensure correct order for candle drawing
    processed_data.sort_by(|a, b| a.0.cmp(&b.0));

    // Format price with commas for better readability is now done earlier in the code

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

    // No longer need the old price line and label here, as we've redesigned the price display

    // Draw a single horizontal line at the current price level for reference
    chart_context
        .draw_series(std::iter::once(PathElement::new(
            vec![(start_millis as f64, current_price_log), (end_millis as f64, current_price_log)],
            RGBColor(100, 100, 100).stroke_width(1)
        )))
        .unwrap();
        
    // Now draw the table in table_area instead of showing the price on the chart
    // Find the highest price in the visible plot
    let highest_price = processed_data.iter()
        .map(|(_, _, h, _, _, _, _)| *h)
        .fold(f64::NEG_INFINITY, f64::max);
        
    // Calculate percentage from high
    let percent_from_high = if highest_price > 0.0 {
        ((highest_price - current_price) / highest_price) * 100.0
    } else {
        0.0
    };
    
    // Table setup
    let rows = vec!["Current Price", "High (in plot)", "% from High"];
    let cols = vec!["Value"];
    let table_data = vec![
        vec![format!("${}", formatted_current_price)],
        vec![format!("${}", format_with_commas(highest_price))],
        vec![format!("{:.2}%", percent_from_high)],
    ];

    // Compute cell sizes
    let cell_w = plot_width as f64 / 2.0; // Make table take half the width
    let cell_h = table_height as f64 / (rows.len() + 1) as f64;
    
    // First, fill the table area with white for a clean background
    // This creates the white spacing between cells
    table_area.fill(&WHITE).unwrap();
    
    // Setup for cell drawing
    let cell_padding = 5; // Space between cells in pixels (increased padding)
    let section_width = (table_area.get_pixel_range().0.end - table_area.get_pixel_range().0.start) as i32;
    let mid_point = section_width / 2;
    
    // Calculate optimal row height with spacing
    let row_spacing = (cell_h * 0.15) as i32; // 15% of row height as spacing
    let effective_row_height = cell_h as i32 - row_spacing;
    
    // Cell background color - medium grey for good visibility
    let cell_bg_color = RGBColor(220, 220, 220);
    
    // Additional padding for bottom of cells
    let bottom_padding = 6; // Extra bottom padding for cells
    
    for (ri, row_label) in rows.iter().enumerate() {
        // Calculate position with proper spacing between rows
        let row_pos = (ri as i32 * (effective_row_height + row_spacing + bottom_padding)) + cell_padding;
        let row_height = effective_row_height;
        let row_center = row_pos + (row_height / 2);
        
        // Text color based on data
        let text_color = if ri == 0 { &last_candle_color } else { &BLACK };
        
        // Draw left column cell with padding on all sides
        table_area.draw(&Rectangle::new(
            [(cell_padding, row_pos), 
             (mid_point - cell_padding, row_pos + row_height)],
            cell_bg_color.filled()
        )).unwrap();
        
        // Draw right column cell with padding on all sides
        table_area.draw(&Rectangle::new(
            [(mid_point + cell_padding, row_pos), 
             (section_width - cell_padding, row_pos + row_height)],
            cell_bg_color.filled()
        )).unwrap();
        
        // Calculate better vertical position to ensure text is centered in the cell
        // Move text up more from center for better vertical alignment and to add bottom padding effect
        let text_y_adjustment = 4; // Increased shift up for more bottom padding appearance
        let text_y_pos = row_center - text_y_adjustment;
        
        // Left column text (label) with proper vertical alignment
        table_area.draw(&Text::new(
            row_label.to_string(),
            (cell_padding*4, text_y_pos), // Left padding with adjusted vertical position
            ("sans-serif", 14).into_font().color(text_color),
        )).unwrap();
        
        // Right column text (value) with proper vertical alignment
        table_area.draw(&Text::new(
            table_data[ri][0].clone(),
            (mid_point + cell_padding*4, text_y_pos), // Left padding with adjusted vertical position
            ("sans-serif", 14).into_font().color(text_color),
        )).unwrap();
    }
    
    // Add a horizontal line at the current price level using the same color as the last candle
    chart_context
        .draw_series(std::iter::once(PathElement::new(
            vec![(start_millis as f64, current_price_log), (end_millis as f64, current_price_log)],
            last_candle_color.stroke_width(1)
        )))
        .unwrap();
    
    // We'll skip drawing the rectangle and text overlay since we're now
    // highlighting the price directly in the y-axis labels

    // We're no longer drawing the rectangle and text here since we're highlighting
    // the price directly in the y-axis labels
        
    // --- Draw the candlestick bodies (no wicks) with consistent spacing ---
    // Debug information removed for cleaner output
        
    // First draw the wicks (thin dark grey rectangles) so they appear behind the candle bodies
    chart_context
        .draw_series(
            processed_data
                .iter()
                .enumerate()
                .map(|(idx, (dt, _o, h, l, _c, _v, _color_hex))| {
                    // Convert to milliseconds since start for x-axis positioning
                    let dt_millis = millis_since_start(*dt) as f64;
                    
                    // Calculate candle and wick widths
                    let total_millis = (end_millis as f64) - (start_millis as f64);
                    let candle_width = total_millis / processed_data.len() as f64 * 0.8; // 80% of available space
                    let wick_width = candle_width * 0.15; // 15% of candle width for the wick
                    
                    // Calculate wick position (center of the candle)
                    let wick_left = dt_millis - (wick_width / 2.0);
                    let wick_right = dt_millis + (wick_width / 2.0);
                    
                    // Convert high and low to log scale for plotting
                    let high_log = h.ln();
                    let low_log = l.ln();
                    
                    // Return a dark grey rectangle for the wick
                    Rectangle::new(
                        [(wick_left, high_log), (wick_right, low_log)],
                        RGBColor(70, 70, 70).filled()
                    )
                })
        )
        .unwrap();
    
    // Next draw the candle bodies on top of the wicks
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

                    // Determine candle body top and bottom (based on open/close)
                    let (body_top, body_bottom) = if open_log <= close_log {
                        (close_log, open_log)
                    } else {
                        (open_log, close_log)
                    };

                    // Calculate candle width and position
                    let total_millis = (end_millis as f64) - (start_millis as f64);
                    let candle_width = total_millis / processed_data.len() as f64 * 0.8; // 80% of available space
                    let dt_millis = millis_since_start(*dt) as f64;
                    let body_left = dt_millis - (candle_width / 2.0);
                    let body_right = dt_millis + (candle_width / 2.0);
                    
                    // Debug output removed for cleaner logs
                    
                    // Return the rectangle for the candle body
                    Rectangle::new(
                        [(body_left, body_top), (body_right, body_bottom)],
                        candle_color.filled()
                    )
                })
        )
        .unwrap();

    // --- Draw markers from plots.marks ---
    let candle_duration_millis = if processed_data.len() > 1 {
        let total_millis = (end_millis as f64) - (start_millis as f64);
        total_millis / processed_data.len() as f64
    } else {
        60000.0 // Default 1 minute if only one candle
    };
    let marker_candle_width = candle_duration_millis * 0.8;

    for mark in &data.plots.marks {
        // Convert mark timestamp to milliseconds since chart start
        let mark_time_millis = mark.time - start_dt.timestamp_millis();
        let x = mark_time_millis as f64;

        // Find candle at this timestamp to get high/low for positioning
        // Allow small tolerance for timestamp matching (within half a candle duration)
        let candle_idx = processed_data.iter().position(|(dt, _, _, _, _, _, _)| {
            let dt_millis = millis_since_start(*dt);
            (dt_millis - mark_time_millis).abs() < (candle_duration_millis as i64 / 2)
        });

        if let Some(idx) = candle_idx {
            let (_, _, h, l, _, _, _) = &processed_data[idx];
            let size = mark.size;
            let log_range = max_log_for_chart - min_log_for_chart;
            let offset = log_range * 0.02 * size; // 2% of price range per size unit

            let y = if mark.position == "above" {
                h.ln() + offset
            } else {
                l.ln() - offset
            };

            let color = parse_hex_color(&mark.color);
            let triangle_half_width = marker_candle_width / 3.0 * size;
            let triangle_height = offset / 2.0;

            // Draw triangle marker
            if mark.position == "above" {
                // Downward-pointing triangle (▼) above candle, pointing at the high
                chart_context
                    .draw_series(std::iter::once(Polygon::new(
                        vec![
                            (x, y - triangle_height),           // Bottom point (pointing down)
                            (x - triangle_half_width, y + triangle_height), // Top left
                            (x + triangle_half_width, y + triangle_height), // Top right
                        ],
                        color.filled(),
                    )))
                    .unwrap();
            } else {
                // Upward-pointing triangle (▲) below candle, pointing at the low
                chart_context
                    .draw_series(std::iter::once(Polygon::new(
                        vec![
                            (x, y + triangle_height),           // Top point (pointing up)
                            (x - triangle_half_width, y - triangle_height), // Bottom left
                            (x + triangle_half_width, y - triangle_height), // Bottom right
                        ],
                        color.filled(),
                    )))
                    .unwrap();
            }

            // Draw label text if provided
            if let Some(text) = &mark.text {
                let text_y = if mark.position == "above" {
                    y + offset * 1.2
                } else {
                    y - offset * 1.2
                };
                let font_size = (12.0 * size).max(8.0) as i32;
                chart_context
                    .draw_series(std::iter::once(Text::new(
                        text.clone(),
                        (x, text_y),
                        TextStyle::from(("sans-serif", font_size)).color(&color),
                    )))
                    .unwrap();
            }
        }
    }

// Instead of drawing custom labels, use the built-in x-axis labels with rotation
// We're now using simpler numerical hours as the x-axis values, which won't cause overflow

    // Present and save the result
    root_area.present().unwrap();

    println!(
        "{} {} {}",
        format!("[{}]", now).dimmed(),
        "✅".green().bold(),
        format!("Chart processing complete. Saved to: {}", file_path.bold())
    );
    
    // Send notification to telegram service via ZMQ
    if let Err(e) = send_telegram_notification(&data, &file_path) {
        eprintln!("[{}] ❌ Failed to send telegram notification: {}", now, e);
    } else {
        // Log where the notification was sent
        let destination = match (&data.chat_id, &data.subscriber_list) {
            (Some(chat_id), _) => format!("chat_id: {}", chat_id),
            (_, Some(list)) => format!("subscriber_list: {}", list),
            _ => "default destination".to_string()
        };
        println!(
            "{} {} {}",
            format!("[{}]", now).dimmed(),
            "📲".blue().bold(),
            format!("Telegram notification sent to {}", destination.bold())
        );
    }
}

/// Send a notification to the telegram service via ZMQ with the chart details and image path
fn send_telegram_notification(data: &ChartData, image_path: &str) -> Result<(), Box<dyn Error>> {
    let context = zmq::Context::new();
    let socket = context.socket(zmq::DEALER)?;
    
    // Connect to the same endpoint as the main application
    let endpoint = "tcp://127.0.0.1:6565";
    socket.connect(endpoint)?;
    
    // Create the payload with chat_id and subscriber_list if available
    let payload = serde_json::json!([
        "ok",
        "send_message",
        {
            "text": data.desc,
            "image_path": image_path,
            "chat_id": data.chat_id,
            "subscriber_list": data.subscriber_list
        }
    ]);
    
    // Convert to string
    let json_message = payload.to_string();
    
    // Send the multipart message
    socket.send_multipart(&[b"telegram", json_message.as_bytes()], 0)?;
    
    Ok(())
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
