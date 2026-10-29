# Gauger

A Rust TUI for monitoring resource consumption across Docker and Podman containers.

Gauger aims to provide a lightweight, terminal-based overview of container resource usage, making it easy to inspect CPU, memory, network, and disk I/O from a single interface.

## Features

- [ ] Live container resource monitoring
  - [ ] Container filtering
  - [ ] Compose support
- [ ] Engine support
  - [x] Docker
  - [x] Podman
- [ ] Interactive terminal interface
- [ ] Resource usage statistics and history

_Gauger is under active development. Features are planned and may not yet be implemented._

## Installation

### From source

Requires a Rust toolchain.

```bash
git clone https://github.com/Adega318/gauger.git
cd gauger
cargo build --release
```

The executable will be available at `target/release/gauger`.

## Usage

```bash
gauger
```

Gauger will provide an interactive interface for inspecting container resource consumption.

## Development

Build and run the project in development mode:

```bash
cargo run
```

Run tests:

```bash
cargo test
```

## License

Licensed under the MIT License. See [LICENSE](LICENSE.md) for details.
