# Corky Charts

A high-performance chart rendering service for financial data built in Rust. This application listens for chart data via ZeroMQ, generates candlestick charts with customizable styling, and saves them to disk.

## Features

- Candlestick charts with wicks showing high/low prices
- Current price display with custom styling
- Information table with key statistics
- Logarithmic price scale for better visibility of price movements
- Clean, modern styling with customizable colors
- High-performance rendering using the Plotters library

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
    [1618530300000, 63100.0, 63400.0, 62800.0, 63500.0, 980.2],
    ...
  ],
  "candle_colors": ["#FF0000", "#00FF00", ...],
  "plots": {
    "additional_plots": []
  },
  "desc": "BTC/USD 15-minute chart from Exchange XYZ"
}
```

### Field Descriptions

| Field | Type | Description |
|-------|------|-------------|
| `title` | String | Chart title displayed at the top |
| `ticker` | String | Trading pair or symbol |
| `timeframe` | String | Chart timeframe (e.g., "1m", "5m", "1h", "1d") |
| `cols` | Array of Strings | Column names (should match the data format) |
| `data` | Array of Arrays | Each inner array represents one candle with [timestamp, open, high, low, close, volume] |
| `candle_colors` | Array of Strings | Hex color codes for each candle (must match the length of `data`) |
| `plots` | Object | Container for additional plot configurations |
| `desc` | String | Description of the chart (currently not displayed) |

#### Data Format Details

- `timestamp`: milliseconds since Unix epoch
- `open`: Opening price for the period
- `high`: Highest price during the period
- `low`: Lowest price during the period
- `close`: Closing price for the period
- `volume`: Trading volume for the period

#### Candle Colors

Each candle can have a custom color defined in the `candle_colors` array. Colors should be specified as hex values (e.g., "#FF0000" for red).

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
    "plots": {"additional_plots": []},
    "desc": "Example chart"
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

Charts are saved to the `output` directory in the project folder with filenames based on the ticker and timestamp.

## Example Output

```
[INIT] Connecting to tcp://127.0.0.1:6565 as 'rustcharts'…
[READY] Awaiting incoming chart messages…
[2025-05-20 21:58:30] ▶ New Chart Request for BTCUSD @ 15m [120 candles]
[2025-05-20 21:58:30] 🖼️ Processing chart: 'BTCUSD 15m Chart' with 120 candles
[2025-05-20 21:58:32] ✅ Chart processing complete. Saved to: output/BTCUSD_20250520_215832.png
```

## Advanced Features

### Candlestick Wicks

Each candlestick displays a wick that extends from the high to the low price of that period. The wicks are rendered as dark gray rectangles behind the main candle body, ensuring they're visible regardless of candle color.

### Current Price Indicator

The current price (last candle's close) is displayed prominently on the right side of the chart with a black background and white text for clear visibility.

### Price Statistics Table

A table above the chart displays key statistics:
- Current Price
- Highest Price in the chart
- Percentage from the high price

## Customization

The chart styling is currently hard-coded in the application. For customizations, you'll need to modify the code in `src/main.rs` and recompile the application.

## License

MIT License
