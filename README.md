# grpc-logger

A Rust crate providing logging capabilities with configurable outputs (console, file) and real-time log streaming through gRPC.

## Features

- Multiple Output Modes
  - Console logging (direct terminal output)
  - File logging (with automatic rotation)
  - gRPC streaming (for real-time log aggregation)
- Rich Logging Context
  - Custom timestamp formatting (RFC3339)
  - Thread ID tracking (for concurrent operations)
  - File and line number tracking (source code location)
  - Configurable log level filtering (trace through error)
- Connection Management
  - Automatic reconnection handling (with exponential backoff)
  - Configurable retry strategies (separate client/server policies)
  - Stream management (automatic recovery on disconnection)

## Installation

Add to your Cargo.toml:
```
[dependencies]
grpc-logger = "0.1.0"
```

## Configuration Parameters

| Parameter | Type | Description | Required | Default |
|-----------|------|-------------|----------|---------|
| output | enum | Logging output type (Console/File/Grpc) | Yes | - |
| level | string | Log level (trace/debug/info/warn/error) | Yes | - |
| file_path | string | Directory path for log files | For File output | "logs" |
| file_name | string | Name of the log file | For File output | "app.log" |
| grpc.address | string | gRPC server address | For Grpc output | "0.0.0.0" |
| grpc.port | number | gRPC server port | For Grpc output | 50052 |
| client_retry.max_retries | number | Maximum connection attempts | No | 5000 |
| client_retry.base_delay_secs | number | Initial retry delay in seconds | No | 2 |
| client_retry.reconnect_delay_secs | number | Delay between reconnections | No | 2 |
| server_retry.max_retries | number | Server binding retry attempts | No | 5 |
| server_retry.base_delay_secs | number | Server retry delay in seconds | No | 1 |

## Usage

### Server Integration

See `examples/basic.rs` for minimal server setup and `examples/retry.rs` for server with retry logic.

Run server examples:
```
cargo run --example basic
cargo run --example retry
```

### Client Usage

See `examples/client.rs` for a complete client implementation with retry and reconnection handling.

Run client:
```
cargo run --bin client
```

## License

MIT License

## Contributing

Contributions welcome! Please feel free to submit a Pull Request.
