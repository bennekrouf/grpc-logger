# grpc-logger
A Rust crate providing logging capabilities with configurable outputs (console, file) and real-time log streaming through gRPC.



cargo run --bin grpc_server

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
  - Selective log field inclusion
- Connection Management
  - Automatic reconnection handling (with exponential backoff)
  - Configurable retry strategies (separate client/server policies)
  - Stream management (automatic recovery on disconnection)
- Debug Mode
  - Test message generation
  - Configurable test intervals

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
| log_fields.include_thread_id | boolean | Include thread ID in logs | No | false |
| log_fields.include_target | boolean | Include target module in logs | No | false |
| log_fields.include_file | boolean | Include source file name | No | false |
| log_fields.include_line | boolean | Include source line number | No | false |
| log_fields.include_timestamp | boolean | Include timestamp in logs | No | false |
| debug_mode.enabled | boolean | Enable debug test messages | No | false |
| debug_mode.test_interval_secs | number | Interval for test messages | No | 10 |

## Example Configuration
```yaml
server_id: "DB Webservice"
output: grpc # console, file
level: info
grpc:
  address: "127.0.0.1"
  port: 50052
log_fields:
  include_thread_id: false
  include_target: false
  include_file: false
  include_line: false
  include_timestamp: false
debug_mode:
  enabled: false
  test_interval_secs: 10
```

## Usage
### Quick test of the repository without any integration
Clone the repository https://github.com/bennekrouf/grpc-logger. Then open a terminal and run:
```
cargo run
```
Open another terminal and run: 
```
cargo run --example client
```
You should see logs produced by the first terminal, appearing in the second terminal.

**Important**: If the server is running on port 0.0.0.0, the client should try connecting on 127.0.0.1.

### Server Integration
Adding the crate to a rust program will allow to do info!("blabla") log broadcasted through a grpc server that is instantiated by the crate. Then you can have a web client or a rust client receiving all the logs in a stream way.

See `examples/basic.rs` for minimal server setup and `examples/retry.rs` for server with retry logic.

Run server examples:
```
cargo run --example basic
cargo run --example retry
```

### Log Field Selection
You can selectively include or exclude log fields using the `log_fields` configuration. This helps reduce log size and improve readability by only including the fields you need.

### Debug Mode
Debug mode allows you to generate test messages at configurable intervals, which is useful for testing log streaming and client connections without needing actual application logs.

### Web Client Usage
See this repo which is a React implementation of the client:
[https://github.com/bennekrouf/grpc-logger-web-client](Client react)

### Rust Client Usage
See `examples/client.rs` for a complete client implementation with retry and reconnection handling.

Run client:
```
cargo run --example client
```

## License
MIT License

## Contributing
Contributions welcome! Please feel free to submit a Pull Request.
