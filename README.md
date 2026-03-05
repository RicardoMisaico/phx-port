# phx-port

> Stop memorizing port numbers. One command, consistent ports for every Phoenix project.

When you work on multiple [Phoenix](https://www.phoenixframework.org/) projects, they all default to port 4000. `phx-port` gives each project its own stable port — automatically — so you never have collisions and never have to remember which port goes where.

```bash
~/projects/my_app $ PORT=$(phx-port) iex -S mix phx.server
# → always starts on the same port, every time
```

## Install

```bash
cargo install --git https://github.com/chgeuer/phx-port
```

Or build from source:

```bash
git clone https://github.com/chgeuer/phx-port
cd phx-port
cargo build --release
cp target/release/phx-port ~/.local/bin/
```

## How it works

`phx-port` maintains a simple TOML registry at `~/.config/phx-ports.toml`.

Override the config location with the `PHX_PORT_CONFIG` environment variable:

```bash
export PHX_PORT_CONFIG="$HOME/.phx-ports.toml"       # Linux/macOS alternative
export PHX_PORT_CONFIG="C:\Users\me\.phx-ports.toml"  # Windows
```

Default config:

```toml
[ports]
"/home/user/projects/my_app" = 4001
"/home/user/projects/api_gateway" = 4002
"/home/user/projects/admin_dashboard" = 4003
```

- **First run in a project** → allocates the next available port (starting at 4001), saves it, and prints it
- **Subsequent runs** → prints the saved port instantly
- **Port 4000 stays free** for ad-hoc or unmanaged projects

## Usage

### In scripts and shell wrappers (piped mode)

When stdout is not a terminal, `phx-port` prints just the port number — perfect for command substitution:

```bash
PORT=$(phx-port) iex -S mix phx.server
PORT=$(phx-port) mix phx.server
```

Put this in a project's `run` script and never think about ports again.

### Managing registrations

```bash
# List all registered projects and their ports
phx-port --list

# Explicitly register the current directory
phx-port --register

# Remove a mapping — by port number, directory name, or current directory
phx-port --delete 4003
phx-port --delete admin_dashboard
phx-port --delete .
```

### Interactive mode

Running `phx-port` with no arguments in a terminal shows the help text. This way it never accidentally auto-registers when you're just exploring.

## Example workflow

```
~/projects/shop $ phx-port --list
 4001  /home/user/projects/api
 4002  /home/user/projects/admin

~/projects/shop $ PORT=$(phx-port) iex -S mix phx.server
Registered /home/user/projects/shop → port 4003    # ← stderr, first time only
[info] Running ShopWeb.Endpoint on http://localhost:4003

~/projects/shop $ phx-port --list
 4001  /home/user/projects/api
 4002  /home/user/projects/admin
 4003  /home/user/projects/shop
```

## License

MIT
