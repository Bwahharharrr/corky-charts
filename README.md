# Corky Charts

A high-performance chart rendering service for financial data built in Rust. This application listens for chart data via ZeroMQ, generates candlestick charts with customizable styling, and saves them to disk.

## Features

- Candlestick charts with wicks showing high/low prices
- Colored volume bars with customizable per-bar colors
- Current price display with custom styling
- Information table with key statistics
- Logarithmic price scale for better visibility of price movements
- Clean, modern styling with customizable colors
- High-performance rendering using the Plotters library
- Marker annotations for signal indicators (triangles above/below candles)
- Zone rendering for resistance/support visualization (semi-transparent rectangles)
- Vertical line indicators for alert timestamps
- Telegram integration for notifications
- Configurable output directory via config file

## Installation

### Prerequisites

- Rust and Cargo (install via [rustup](https://rustup.rs/))
- ZeroMQ library (libzmq)

### Building

```bash
git clone https://github.com/yourusername/corky-charts.git
cd corky-charts
cargo build --release
```

## Configuration

Corky Charts requires a configuration file at `~/.corky/config.toml`. The application will not start without this file.

### Config File Format

```toml
[charts]
directory = "/path/to/output/directory"
```

### Configuration Options

| Option | Description |
|--------|-------------|
| `charts.directory` | The directory where chart images will be saved (required) |

## Usage

Run the application to start the ZeroMQ server:

```bash
cargo run --release
```

The server will listen on `tcp://127.0.0.1:6565` by default.

## JSON Input Format

Chart data is sent to the application as a JSON object via ZeroMQ. The application expects a specific format with the following fields:

```json
{
  "title": "BTCUSD 15m Chart",
  "ticker": "BTCUSD",
  "timeframe": "15m",
  "cols": ["timestamp", "open", "high", "low", "close", "volume"],
  "data": [
    [1618531200000, 63500.0, 63800.0, 62900.0, 63100.0, 1200.5],
    [1618530300000, 63100.0, 63400.0, 62800.0, 63500.0, 980.2]
  ],
  "candle_colors": ["#FF0000", "#00FF00"],
  "volume_colors": ["#FF000080", "#00FF0080"],
  "plots": {
    "marks": [
      {
        "time": 1618531200000,
        "position": "above",
        "color": "#FF0000",
        "text": "4h",
        "size": 1.0
      }
    ],
    "zones": [
      {
        "x1": 1618530300000,
        "x2": 1618531200000,
        "y1": 62800.0,
        "y2": 63000.0,
        "color": "#00FF0030"
      }
    ],
    "vlines": [
      {
        "time": 1618531200000,
        "color": "#0000FF"
      }
    ]
  },
  "desc": "BTC/USD 15-minute chart from Exchange XYZ",
  "chat_id": 123456789,
  "subscriber_list": "crypto_alerts",
  "image_filename": "BTCUSD_alert_12345.png"
}
```

### Field Descriptions

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `title` | String | Yes | Chart title displayed at the top |
| `ticker` | String | Yes | Trading pair or symbol |
| `timeframe` | String | Yes | Chart timeframe (e.g., "1m", "5m", "1h", "1d") |
| `cols` | Array of Strings | Yes | Column names (should match the data format) |
| `data` | Array of Arrays | Yes | Each inner array represents one candle with [timestamp, open, high, low, close, volume] |
| `candle_colors` | Array of Strings | Yes | Hex color codes for each candle (must match the length of `data`) |
| `volume_colors` | Array of Strings | No | Hex color codes for each volume bar (defaults to gray if not provided) |
| `plots` | Object | Yes | Container for additional plot configurations (marks, zones, vlines) |
| `desc` | String | Yes | Description of the chart (used in Telegram notifications) |
| `chat_id` | Integer | No | Telegram chat ID for direct message delivery |
| `subscriber_list` | String | No | Name of Telegram subscriber list for broadcast |
| `image_filename` | String | No | Custom output filename (prevents race condition overwrites when multiple alerts fire) |

#### Data Format Details

- `timestamp`: milliseconds since Unix epoch
- `open`: Opening price for the period
- `high`: Highest price during the period
- `low`: Lowest price during the period
- `close`: Closing price for the period
- `volume`: Trading volume for the period

#### Candle Colors

Each candle can have a custom color defined in the `candle_colors` array. Colors should be specified as hex values (e.g., "#FF0000" for red).

#### Volume Colors

Each volume bar can have a custom color defined in the `volume_colors` array. Colors should be specified as hex values. If not provided, volume bars default to gray.

## Plot Types

The `plots` object supports three types of overlays: markers, zones, and vertical lines.

### Markers (`marks`)

Markers are triangle indicators that appear above or below candles, useful for showing signal points.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `time` | Integer | Yes | Timestamp in milliseconds (must match a candle timestamp) |
| `position` | String | Yes | `"above"` or `"below"` the candle |
| `color` | String | Yes | Hex color code (e.g., "#FF0000") |
| `text` | String | No | Optional label text displayed near the marker (e.g., "4h") |
| `size` | Float | No | Relative size multiplier (default: 1.0) |

Markers pointing `"above"` render as downward-pointing triangles (▼) above the candle's high. Markers pointing `"below"` render as upward-pointing triangles (▲) below the candle's low.

### Zones (`zones`)

Zones are semi-transparent rectangular areas, useful for showing support/resistance levels.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `x1` | Integer | Yes | Left timestamp in milliseconds |
| `x2` | Integer | Yes | Right timestamp in milliseconds |
| `y1` | Float | Yes | Bottom price level |
| `y2` | Float | Yes | Top price level |
| `color` | String | Yes | Hex color with optional alpha (e.g., "#FF000030" for 30/255 opacity) |

Zone colors support RGBA format (`#RRGGBBAA`). If only RGB is provided (`#RRGGBB`), zones default to 30% opacity.

### Vertical Lines (`vlines`)

Vertical lines span the full chart height at a specific timestamp, useful for marking alert fire times or events.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `time` | Integer | Yes | Timestamp in milliseconds |
| `color` | String | Yes | Hex color code (e.g., "#0000FF") or with alpha (e.g., "#0000FF80") |

## ZeroMQ Communication

The application uses ZeroMQ's DEALER socket to receive chart requests.

### Connection Details

- **Protocol**: TCP
- **Default Address**: 127.0.0.1
- **Default Port**: 6565
- **Socket Type**: DEALER
- **Identity**: "rustcharts"

### Sending Data

To send data to the application, you need to use a ZeroMQ client with a matching socket type (typically a ROUTER socket). The message should be sent as a multipart message with the following frames:

1. Empty frame (for DEALER compatibility)
2. JSON payload as described above

#### Example Client (Python)

```python
import zmq
import json
import time

# Prepare data
chart_data = {
    "title": "BTCUSD 15m Chart",
    "ticker": "BTCUSD",
    "timeframe": "15m",
    "cols": ["timestamp", "open", "high", "low", "close", "volume"],
    "data": [
        # Array of [timestamp, open, high, low, close, volume]
        [int(time.time() * 1000) - 900000, 63500.0, 63800.0, 62900.0, 63100.0, 1200.5],
        [int(time.time() * 1000), 63100.0, 63400.0, 62800.0, 63500.0, 980.2],
    ],
    "candle_colors": ["#FF0000", "#00FF00"],
    "volume_colors": ["#FF0000", "#00FF00"],  # Optional: colored volume bars
    "plots": {
        "marks": [
            {
                "time": int(time.time() * 1000),
                "position": "above",
                "color": "#FFAA00",
                "text": "Signal",
                "size": 1.2
            }
        ],
        "zones": [
            {
                "x1": int(time.time() * 1000) - 900000,
                "x2": int(time.time() * 1000),
                "y1": 62800.0,
                "y2": 63000.0,
                "color": "#00FF0030"  # Green with 30/255 opacity
            }
        ],
        "vlines": [
            {
                "time": int(time.time() * 1000) - 450000,
                "color": "#0000FF"
            }
        ]
    },
    "desc": "Example chart with all features",
    "chat_id": 123456789,  # Optional: Telegram chat ID
    "image_filename": "BTCUSD_example_chart.png"  # Optional: custom filename
}

# Prepare ZeroMQ connection
context = zmq.Context()
socket = context.socket(zmq.ROUTER)
socket.bind("tcp://127.0.0.1:6565")

# Send message to the rustcharts client
socket.send_multipart([
    b"rustcharts",  # Destination identity
    b"",            # Empty frame
    json.dumps(["chart", "request", chart_data]).encode()  # JSON payload
])

print("Chart request sent")
```

## Output

Charts are saved to the directory specified in `~/.corky/config.toml` under `[charts].directory`.

### Output Filename

- If `image_filename` is provided in the request, the chart is saved with that exact filename
- Otherwise, the filename defaults to `{ticker}_{timeframe}.png` (e.g., `BTCUSD_15m.png`)

Using `image_filename` is recommended when multiple alerts may fire simultaneously to prevent race condition overwrites.

### Canvas Dimensions

Charts are rendered at 1280x960 pixels.

### Drawing Order (Z-Order)

Elements are drawn in this order (back to front):
1. Background (white)
2. Grid lines
3. Zones (semi-transparent rectangles)
4. Vertical lines
5. Volume bars
6. Candlestick wicks
7. Candlestick bodies
8. Markers (triangles and labels)
9. Current price line
10. Information table

## Example Output

```
[INIT] Using output directory: /home/user/charts
[INIT] Connecting to tcp://127.0.0.1:6565 as 'rustcharts'…
[READY] Awaiting incoming chart messages…
╔ ══════════════════════════════════════════════════════════════════════
[2025-05-20 21:58:30] ▶ New Chart Request for BTCUSD @ 15m [120 candles]
       120 candles from 2025-05-20 20:00:00 to 2025-05-20 21:58:00
       Desc: BTC/USD 15-minute chart
Price range: $62,900 - $63,800
[2025-05-20 21:58:30] 🖼️  Processing chart: 'BTCUSD 15m Chart' with 120 candles
[2025-05-20 21:58:32] ✅ Chart processing complete. Saved to: /home/user/charts/BTCUSD_15m.png
[2025-05-20 21:58:32] 📲 Telegram notification sent to chat_id: 123456789
```

## Advanced Features

### Candlestick Wicks

Each candlestick displays a wick that extends from the high to the low price of that period. The wicks are rendered as dark gray rectangles behind the main candle body, ensuring they're visible regardless of candle color.

### Current Price Indicator

The current price (last candle's close) is displayed prominently in the information table with color coding:
- Green if the last candle closed up
- Red if the last candle closed down

A horizontal line is drawn across the chart at the current price level.

### Price Statistics Table

A table above the chart displays key statistics:
- Current Price
- Highest Price in the chart
- Percentage from the high price

### Telegram Integration

After generating a chart, the application sends a notification via ZeroMQ to a Telegram service. The notification includes:
- The chart description (`desc` field)
- The image path
- Optional `chat_id` for direct delivery
- Optional `subscriber_list` for broadcast to a subscriber group

## Customization

The chart styling is currently hard-coded in the application. For customizations, you'll need to modify the code in `src/main.rs` and recompile the application.

## License

MIT License
