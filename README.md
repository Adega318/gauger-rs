# Gauger

A Rust TUI for monitoring resource consumption across Docker and Podman containers.

Gauger aims to provide a lightweight, terminal-based overview of container resource usage, making it easy to inspect CPU, memory, network, and disk I/O from a single interface.

## Features

- [ ] Live container resource monitoring
  - [x] Container filtering
  - [ ] Compose support
- [x] Engine support
  - [x] Docker
  - [x] Podman
- [x] Interactive terminal interface
- [x] Plain-text output mode
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

### Plain-text mode

Pass `--no-tui` to stream container listings and lifecycle events without starting the interactive interface:

```bash
gauger --no-tui
```

```text
12:00:00  docker   list     05ae208348f4  postgres                  running     healthy    postgres:18
12:00:04  docker   start    c1a2b3c4d5e6  worker
```

Each container line is written to stdout and starts with a UTC timestamp, the runtime, and the event type (`list`, `create`, `start`, `stop`, `remove`, …). Runtime connection messages go to stderr, so stdout remains suitable for piping. The command exits on `Ctrl+C` or `SIGTERM`.

### Keybindings

| Key                  | Action                         |
| -------------------- | ------------------------------ |
| `j` / `k`, `↓` / `↑` | Move selection                 |
| `g` / `G`            | Jump to first / last container |
| `Ctrl+u` / `Ctrl+d`  | Page up / down                 |
| `/`                  | Filter containers              |
| `c`, `Esc`           | Clear filter                   |
| `s` / `S`            | Cycle sort key / reverse order |
| `Enter`              | Open container details         |
| `r`                  | Refresh container lists        |
| `?`                  | Help                           |
| `q`, `Ctrl+c`        | Quit                           |

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
