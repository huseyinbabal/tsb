
<p align="center">
  <img width="630" height="350" alt="tsb-logo" src="https://github.com/user-attachments/assets/4a8054dc-baa0-45e4-ad86-782d43fa6b76" />
</p>

# tsb - Terminal UI for Spring Boot

**tsb** (Terminal Spring Boot) is a modern, Terminal User Interface (TUI) application inspired by `k9s`, designed specifically for Spring Boot developers. It aims to streamline the process of bootstrapping new Spring Boot applications and managing/monitoring running Spring Boot instances via their Actuator endpoints directly from the terminal.

---

[![License](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)

---

## Showcase

<div align="center">
  <video src="https://github.com/user-attachments/assets/PLACEHOLDER_VIDEO_URL" 
         controls 
         autoplay 
         loop 
         muted
         width="600">
    Tarayıcınız video etiketini desteklemiyor.
  </video>
</div>

---

## Features

- **Project Generation** - Interactive TUI wizard to bootstrap new Spring Boot projects (like start.spring.io).
- **Auto-Discovery & Manual Addition** - Automatically scan local ports or manually add running Spring Boot apps.
- **Applications View** - A main dashboard listing all discovered/added Spring Boot apps and their health status.
- **Beans Explorer** - Browse the Spring application context to see all loaded beans and their dependencies.
- **Endpoints & Mappings** - View exposed Actuator endpoints and HTTP request mappings.
- **Environment Properties** - Inspect active environment variables, properties, and configuration details.
- **Logger Management** - View and dynamically update logger levels on the fly without restarting.
- **Thread Dumps** - Generate and view live thread dumps directly in the terminal.
- **Heap Dumps** - Download heap dumps locally for further analysis or profiling.
- **Keyboard-Driven** - Vim-like navigation (`j`, `k`, `gg`, `G`), search (`/`, `n`, `N`), and command palette (`:`).

---

## Installation

### Homebrew (macOS/Linux)

```bash
brew install huseyinbabal/tap/tsb
```

### Scoop (Windows)

```powershell
scoop bucket add huseyinbabal https://github.com/huseyinbabal/scoop-bucket
scoop install tsb
```

### Download Pre-built Binaries

Download the latest release from the [Releases page](https://github.com/huseyinbabal/tsb/releases/latest).

| Platform | Architecture | Download |
|----------|--------------|----------|
| **macOS** | Apple Silicon (M1/M2/M3) | `tsb-aarch64-apple-darwin.tar.gz` |
| **macOS** | Intel | `tsb-x86_64-apple-darwin.tar.gz` |
| **Linux** | x86_64 (musl) | `tsb-x86_64-unknown-linux-musl.tar.gz` |
| **Linux** | ARM64 (musl) | `tsb-aarch64-unknown-linux-musl.tar.gz` |
| **Windows** | x86_64 | `tsb-x86_64-pc-windows-msvc.zip` |

#### Quick Install (macOS/Linux)

```bash
# macOS Apple Silicon
curl -sL https://github.com/huseyinbabal/tsb/releases/latest/download/tsb-aarch64-apple-darwin.tar.gz | tar xz
sudo mv tsb /usr/local/bin/

# macOS Intel
curl -sL https://github.com/huseyinbabal/tsb/releases/latest/download/tsb-x86_64-apple-darwin.tar.gz | tar xz
sudo mv tsb /usr/local/bin/

# Linux x86_64 (musl - works on Alpine, Void, etc.)
curl -sL https://github.com/huseyinbabal/tsb/releases/latest/download/tsb-x86_64-unknown-linux-musl.tar.gz | tar xz
sudo mv tsb /usr/local/bin/

# Linux ARM64 (musl - works on Alpine, Void, etc.)
curl -sL https://github.com/huseyinbabal/tsb/releases/latest/download/tsb-aarch64-unknown-linux-musl.tar.gz | tar xz
sudo mv tsb /usr/local/bin/
```

#### Windows

1. Download `tsb-x86_64-pc-windows-msvc.zip` from the [Releases page](https://github.com/huseyinbabal/tsb/releases/latest)
2. Extract the zip file
3. Add the extracted folder to your PATH, or move `tsb.exe` to a directory in your PATH

### Using Cargo

```bash
cargo install tspring
```

### Using Docker

```bash
# Run interactively
docker run --rm -it huseyinbabal/tsb

# Build locally
docker build -t tsb .
docker run --rm -it tsb
```

> **Note:** Use `-it` flags for interactive terminal support (required for TUI).

### From Source

tsb is built with Rust. Make sure you have Rust 1.70+ installed, along with a C compiler and linker.

```bash
# Clone the repository
git clone https://github.com/huseyinbabal/tsb.git
cd tsb

# Build and run
cargo build --release
./target/release/tsb
```

---

## Quick Start

```bash
# Launch tsb (will prompt to discover or add an app if none configured)
tsb
```

### Adding an Application

When you first launch `tsb`, you can start discovering local apps. If auto-discovery fails or you want to connect to a remote instance, you can manually add an application. You will need to provide:
- **Name**: A friendly name for the server (e.g., "auth-service", "local-payment")
- **URL**: The Spring Boot Actuator base URL (e.g., `http://localhost:8080/actuator`)

> **Note**: Make sure your Spring Boot application has `spring-boot-starter-actuator` in its dependencies and the necessary endpoints are exposed (e.g. `management.endpoints.web.exposure.include=*`).

### Configuration File

Application configurations are stored in:

| Platform | Path |
|----------|------|
| **Linux** | `~/.config/tsb/config.yaml` |
| **macOS** | `~/.config/tsb/config.yaml` |
| **Windows** | `%APPDATA%\tsb\config.yaml` |

---

## Key Bindings

| Action | Key | Description |
|--------|-----|-------------|
| **Navigation** | | |
| Move up | `k` / `↑` | Move selection up |
| Move down | `j` / `↓` | Move selection down |
| Top | `gg` | Jump to first item |
| Bottom | `G` | Jump to last item |
| **Pagination** | | |
| Next page | `]` | Load next page of results |
| Previous page | `[` | Load previous page of results |
| **Views** | | |
| Resources | `:` | Open resource selector (command palette) |
| Describe | `Enter` / `d` | View detailed properties/JSON for the selected item |
| Back | `Esc` / `Backspace` | Go back to previous view |
| **Actions** | | |
| Refresh | `R` | Refresh current view |
| Search / Filter| `/` | Filter lists or search in describe view |
| Add App | `a` | Add a new Spring Boot application |
| Edit | `e` | Edit properties (e.g. log levels) |
| Delete | `Ctrl-d` | Delete selected application |
| New Project | `N` | Open Spring Initializr wizard to bootstrap a new app |
| Select | `Space` | Toggle selection |
| Quit | `Ctrl-c` / `q` | Exit tsb |
| **Search Search** | | |
| Next match | `n` | Jump to next match |
| Previous match | `N` | Jump to previous match |
| Clear search | `Esc` | Clear search and highlights |

---

## Resource Navigation

Press `:` to open the resource picker. Available resources:

| Resource | Description |
|----------|-------------|
| `apps` | Manage and view Spring Boot applications |
| `beans` | Browse Spring application context beans |
| `env` | View environment properties and configuration |
| `endpoints` | View exposed Actuator endpoints |
| `loggers` | View and edit logger levels |
| `mappings` | View HTTP request mappings |
| `threaddump` | Generate and view thread dumps |
| `heapdump` | Download and manage heap dumps |

---

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

---

## Acknowledgments

- Inspired by [k9s](https://github.com/derailed/k9s) - the awesome Kubernetes CLI
- Inspired by [tredis](https://github.com/huseyinbabal/tredis) - Terminal UI for Redis
- Built with [Ratatui](https://github.com/ratatui-org/ratatui) - Rust TUI library

---

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

---

<p align="center">
  Made with ❤️ for the Spring Boot community
</p>
