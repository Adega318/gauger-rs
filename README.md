# Gauger

A lightweight Rust terminal UI for monitoring Docker and Podman containers.

Gauger connects to the Docker and Podman engines on your machine, shows every container in a single live table, and keeps it up to date from the engines' event streams. CPU, memory, network, and disk I/O monitoring is the next milestone and is not implemented yet (see [Roadmap](#roadmap)).

## Features

- Docker and Podman side by side in one table, with automatic socket discovery
- Live updates from the engine event streams, with no manual polling
- Per-container ID, name, runtime, state, health, and image
- Filtering by name, image, ID prefix, or runtime
- Sorting by runtime, name, state, or health, in either direction
- Container details popup, including the last lifecycle event seen
- Automatic reconnection with backoff; rows from a disconnected engine are dimmed and marked as stale
- Plain-text mode (`--no-tui`) that streams container listings and lifecycle events to stdout for piping

## Roadmap

- [x] Docker and Podman support
- [x] Interactive terminal interface
- [x] Container filtering and sorting
- [x] Plain-text output mode
- [ ] Live resource usage (CPU, memory, network, disk I/O)
- [ ] Resource usage statistics and history
- [ ] Compose project support

_Gauger is under active development. Planned features may not be implemented yet._

## Installation

### With Nix

The repository is a [Nix flake](https://nixos.wiki/wiki/Flakes) (with flakes enabled). Run Gauger without installing it:

```bash
nix run github:Adega318/gauger-rs
```

Install it into your profile:

```bash
nix profile install github:Adega318/gauger-rs
```

Or build it from a local checkout; the binary is linked at `result/bin/gauger`:

```bash
nix build
```

The flake supports `x86_64-linux`, `aarch64-linux`, `x86_64-darwin`, and `aarch64-darwin`.

### From source

Requires a Rust toolchain with support for the 2024 edition (Rust 1.85 or newer).

```bash
git clone https://github.com/Adega318/gauger-rs.git
cd gauger-rs
cargo build --release
```

The executable will be available at `target/release/gauger`. To install it into your Cargo bin directory instead:

```bash
cargo install --path .
```

## Usage

```bash
gauger
```

Gauger starts the interactive interface and connects to every engine it can find. It only needs one: if Docker or Podman is unreachable, the other still works, and the unavailable engine is reported in the footer. If neither can be reached, Gauger exits with an error.

### Options

| Option            | Description                                                        |
| ----------------- | ------------------------------------------------------------------ |
| `--no-tui`        | Stream container changes as plain text instead of starting the TUI |
| `-h`, `--help`    | Print help                                                         |
| `-V`, `--version` | Print version                                                      |

### Engine connection

- **Docker** uses the local defaults of the Docker client, so `DOCKER_HOST` is honored.
- **Podman** looks for its API socket in this order:
  1. `CONTAINER_HOST` (must be a `unix://` address)
  2. `$XDG_RUNTIME_DIR/podman/podman.sock`
  3. `/run/user/<uid>/podman/podman.sock`
  4. `/run/podman/podman.sock`

The Podman API socket must be running. For a rootless setup, enable it with:

```bash
systemctl --user enable --now podman.socket
```

### Filtering

Press `/` and type to filter the table as you go. A container matches when the query is found (case-insensitively) in its name, its image, or its runtime, or when the query is a prefix of its ID.

### Details

Press `Enter` on a container to open a popup with its full ID, image, runtime, state, health, and the last lifecycle event received for it. If the engine is disconnected, the popup warns that the values may be stale.

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

While typing a filter, `Enter` applies it, `Esc` cancels, and `Ctrl+u` clears it. In the details and help popups, `Esc`, `Enter`, or `q` closes them.

### Exit codes

| Code | Meaning                                                       |
| ---- | ------------------------------------------------------------- |
| `0`  | Normal exit                                                   |
| `1`  | Runtime error, including no reachable Docker or Podman engine |
| `2`  | Invalid command-line arguments                                |

## Development

Build and run the project in development mode:

```bash
cargo run
```

Run the plain-text mode in development:

```bash
cargo run -- --no-tui
```

Run tests:

```bash
cargo test
```

### Nix development shell

The flake provides a development shell with the Rust toolchain, `clippy`, `rustfmt`, and `rust-analyzer`:

```bash
nix develop
```

With [direnv](https://direnv.net), the included `.envrc` (`use flake`) loads it automatically.

Other flake commands:

```bash
nix build       # build the package
nix flake check # evaluate and check the flake
nix fmt         # format the Nix files
```

### Project layout

| Path              | Purpose                                                                        |
| ----------------- | ------------------------------------------------------------------------------ |
| `flake.nix`       | Nix package and development shell                                              |
| `src/runtime/`    | Docker and Podman clients built on [bollard](https://crates.io/crates/bollard) |
| `src/collector/`  | Per-engine background tasks: snapshots, event streams, reconnects              |
| `src/app/`        | TUI state: selection, filtering, sorting, input handling                       |
| `src/ui/`         | [ratatui](https://ratatui.rs) rendering: table, details, help                  |
| `src/headless.rs` | Plain-text output mode                                                         |

## License

Licensed under the MIT License. See [LICENSE](LICENSE.md) for details.
