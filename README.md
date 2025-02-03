# Logger to Client

A Rust application that provides logging capabilities with multiple outputs (console, file) and allows real-time log streaming through gRPC.

## Features

- Multiple output modes:
  - Console logging
  - File logging with rotation
  - gRPC streaming
- Custom timestamp formatting
- Thread ID tracking
- File and line number tracking
- Log level filtering

## Configuration

Create a `config.yaml` file:

```
output: file  # can be: console, file, grpc
level: info   # can be: trace, debug, info, warn, error
file_path: logs  # optional, for file output
file_name: app.log  # optional, for file output
grpc:  # optional, for grpc
  address: "127.0.0.1"
  port: 50051
```

## Usage

Start the server (produces logs every 10 seconds):
```
cargo run
```

Start the test client in another terminal to receive logs:
```
cargo run --bin test_client
```

## Example Output

When running the client, you'll see logs in this format:
```
2024-02-03 10:30:15 [INFO] Test log message from server (main.rs:42)
2024-02-03 10:30:25 [INFO] Test log message from server (main.rs:42)
```
