# ani-tui

A poster-focused terminal UI for anime, powered by [ani-cli](https://github.com/pystardust/ani-cli).

This is [ll931217's fork](https://github.com/ll931217/ani-tui) of [logando-al/ani-tui](https://github.com/logando-al/ani-tui), originally created by Logan. It retains the original MIT license and history, with a redesigned Amp interface, real portrait posters, and improved keyboard navigation.

`ani-tui` is a keyboard-first desktop terminal experience for browsing anime, resuming watch progress, and handing playback off to an externally installed player through `ani-cli`.

It is built for local-first use: metadata is cached in SQLite, recommendations are generated from your own watch behavior, and the app stays usable even when network access is limited.

## Changes in This Fork

- One Amp palette across home, search, detail, playback, help, settings, and setup: dark surfaces, cream text, and warm orange selection accents.
- Portrait covers on home cards, search results, and anime details, with background downloads and persistent caching.
- Visible cover art behind dialogs; partially covered images use real halfblock pixels to avoid losing native image transmissions.
- Consistent padding, quiet loading placeholders, and restored terminal colors after Kitty image rendering.
- Home selection scrolls only when it crosses a visible row edge.
- Episode navigation supports all four arrow keys and `h/j/k/l`, using the actual grid width and keeping selection visible on resize.
- Airing shows list aired episodes separately from planned season totals; unreleased and unconfirmed episodes cannot be selected or launched.
- Cached synopsis HTML is cleaned up, and detail layout preserves episode navigation on smaller terminals.

See [the fork guide](docs/FORK.md) for rendering behavior, palette values, and implementation details.

## What It Does

- Browse a curated anime home screen with continue watching, watchlist, seasonal, trending, and recommendation rows
- Resume directly into the next episode with persisted watch history
- Surface local heuristic recommendations with explainable reasons such as shared genres and popularity signals
- Hand playback off to `ani-cli` while keeping the TUI responsive

## Stack

- Rust
- `ratatui` + `crossterm`
- `sqlx` with SQLite
- `reqwest`
- `ani-cli` as an external playback runtime dependency

## Features

- **Amp visual theme** — black surfaces, cream text, warm orange accents, rounded dialogs, and consistent spacing matching the Alacritty Amp palette.
- **Curated home screen** — featured banner + category rows (Continue Watching, Watchlist, Recommended, Trending, Popular, Top Rated, Seasonal)
- **Detail screen** — cover art, metadata, scrollable episode pills with watched indicators
- **Real cover images** — portrait posters in home rows, search, and details using Kitty Graphics Protocol on supported terminals (Kitty, Ghostty); actual image pixels via halfblocks elsewhere. Visible posters load in the background with four concurrent downloads, SQLite persistence, and a bounded in-memory cache.
- **Heuristic recommendation engine** — local-first `Because You Watched` and `More Like This` rows built from watch history, genres, format, recency, and cached popularity signals
- **Playback via ani-cli** — detached external-player handoff with next-episode (`n`) directly from the detail screen
- **Watch history** — episodes marked watched on play, persist across sessions
- **Watchlist** — add/remove with `+`, updates the home row immediately
- **Search** — instant SQLite local search + AniList network fallback for full catalogue
- **Offline-capable** — SQLite cache with TTL-based staleness; home screen loads from cache with no network needed
- **Production overlays** — help, search, settings, and setup/dependency checks are available in-app

## Screenshots

Captured from this fork running in Kitty with native cover images.

### Home

![Amp home screen with portrait covers](docs/screenshots/amp-home.png)

### Search

![Poster search results with visible covers behind the dialog](docs/screenshots/amp-search.png)

### Anime Details

![Amp detail screen with vertical episode navigation selecting E30](docs/screenshots/amp-detail.png)

## Prerequisites

Before running `ani-tui`, make sure these runtime dependencies are installed:

- [ani-cli](https://github.com/pystardust/ani-cli) must be installed and available on `$PATH`
- A supported video player must be installed
  - `mpv` is the default player
  - on macOS, `ani-tui` can use `iina` directly and will also fall back to it when `mpv` is missing
  - `vlc` is also supported
- A terminal with truecolor support is recommended
  - home, search, and detail images share the same poster renderer
  - posters use native Kitty images when available and halfblock image pixels otherwise; missing or failed images show quiet Amp placeholders
  - poster frames keep a 2:3 portrait ratio based on terminal cell dimensions, with titles and ratings beneath them

### Verify Prerequisites

Run these commands before launching the app:

```bash
which ani-cli
which mpv
```

If you plan to use VLC instead of mpv:

```bash
which vlc
```

If any command prints `not found`, install that dependency first and make sure it is on your shell `$PATH`.

## Requirements

- Rust toolchain (`cargo`) for building from source
- The runtime prerequisites above

## Installation

Build this fork from source with the Rust toolchain installed:

```bash
git clone https://github.com/ll931217/ani-tui.git
cd ani-tui
cargo build --release --locked
mkdir -p ~/.local/bin
install -m755 target/release/ani-tui ~/.local/bin/ani-tui
```

Or install directly with Cargo:

```bash
cargo install --git https://github.com/ll931217/ani-tui.git --locked ani-tui-app
```

Install `ani-cli` and a supported player separately. On Arch Linux, `mpv` can be installed with `sudo pacman -S mpv`.

The upstream crates.io package, upstream Homebrew tap, and upstream release binaries do not include this fork's changes. This fork currently builds from source; its own release assets will appear on [GitHub Releases](https://github.com/ll931217/ani-tui/releases) when published.

## Running Tests

Run the standard validation commands before opening a pull request or cutting a release:

```bash
cargo check --all-targets --locked
cargo test --locked
cargo clippy --all-targets --locked -- -D warnings
```

## Usage

```bash
ani-tui
```

If playback does not start, the most common cause is a missing `ani-cli` or missing player binary.

On first run, `ani-tui` will open the in-app setup screen automatically if playback dependencies are missing.

### Keybindings

#### Home
| Key | Action |
|-----|--------|
| `j` / `k` | Move between category rows |
| `h` / `l` | Select cards left / right; scroll only past the visible row edges |
| `Enter` | Open detail screen |
| `/` | Search |
| `s` | Open settings |
| `!` | Open setup / dependency checks |
| `?` | Help overlay |
| `r` | Resume highlighted anime |
| `Shift+R` | Refresh home data |
| `q` / `Esc` | Quit |

#### Detail
| Key | Action |
|-----|--------|
| `h` / `l` or `←` / `→` | Move one episode left / right |
| `j` / `k` or `↓` / `↑` | Move one episode row down / up |
| `Tab` | Toggle focus between Episodes / More Like This |
| `Enter` | Start / continue selected episode, or open focused related anime |
| `+` | Toggle watchlist |
| `n` | Play next episode |
| `/` | Search |
| `s` | Open settings |
| `!` | Open setup / dependency checks |
| `?` | Help overlay |
| `Esc` / `q` | Back |

#### Search
| Key | Action |
|-----|--------|
| Type | Update search query |
| Arrow keys | Previous / next result; change page only past visible page edges |
| `Enter` | Open detail |
| `Esc` | Close |

#### Settings
| Key | Action |
|-----|--------|
| `j` / `k` | Move between preference rows |
| `h` / `l` | Change selected preference |
| `Enter` | Advance the selected preference |
| `Esc` | Close |

#### Setup
| Key | Action |
|-----|--------|
| `r` | Refresh dependency checks |
| `s` | Open settings |
| `Esc` | Close |

## Configuration

Config file is created automatically at first run:

- **Linux**: `~/.config/ani-tui/config.toml`
- **macOS**: `~/Library/Application Support/ani-tui/config.toml`

```toml
quality    = "best"      # best | 1080p | 720p | 480p | 360p
audio_mode = "sub"       # sub | dub
player     = "mpv"       # preferred player: mpv | iina | vlc

[cache]
trending_ttl = 86400     # seconds (24h)
stable_ttl   = 604800    # seconds (7 days)
```

## Architecture

```
src/
  api/
    anilist.rs    — AniList GraphQL client (trending, popular, top rated, seasonal, search)
    airing.rs     — Aired episode evidence and persistent availability cache
    player.rs     — ani-cli subprocess wrapper
  db/
    mod.rs        — SQLite init + migrations
    cache.rs      — Anime metadata model + read/write
    user.rs       — Watch history, continue watching, watchlist
    sync.rs       — TTL-based cache staleness
  services/
    sync.rs       — Orchestrates AniList → SQLite sync + heuristic recommendations
  ui/
    home.rs       — Curated home screen
    detail.rs     — Anime detail + episode list
    playback.rs   — Log stream + controls
    search.rs     — Portrait result grid
    help.rs       — Help overlay + toast notifications
    settings.rs   — Settings overlay
    setup.rs      — Dependency / onboarding overlay
    theme.rs      — Shared Amp colors, dialog geometry, and readable synopsis text
    components/
      cover.rs    — Quiet Amp placeholders for unavailable images
      posters.rs  — Background poster loading, bounded cache, and native/halfblock rendering
  state/mod.rs    — AppState, Screen enum, navigation helpers
  config.rs       — Config loading/saving
  error.rs        — AppError + Result type
  main.rs         — Event loop, input handlers, background task coordination
migrations/
  001_initial.sql — Database schema
```

## Data Sources

- **Metadata**: [AniList GraphQL API](https://anilist.co/graphql) — no API key required
- **Playback**: [ani-cli](https://github.com/pystardust/ani-cli) — streams from supported providers

## Distribution Notes

- `ani-tui` ships as an MIT-licensed Rust application
- `ani-cli` remains an external runtime dependency and is not bundled with this project
- The crates.io package name is `ani-tui-app`, while the installed executable remains `ani-tui`

## Contributing

Contributions are welcome.

If you want to contribute:

- Open an issue for bugs, regressions, or feature proposals
- Fork the repository and create a focused branch for your change
- Keep pull requests small and clearly scoped
- Include screenshots or terminal recordings for UI changes when possible
- Run the project checks before opening a pull request:

```bash
cargo check --all-targets --locked
cargo test --locked
cargo clippy --all-targets --locked -- -D warnings
```

For larger changes, open an issue first so the direction can be agreed before implementation.

## Acknowledgements

- [logando-al/ani-tui](https://github.com/logando-al/ani-tui), created by Logan, is the original project this fork builds on. Its source history and MIT copyright notice are preserved.

- [`ani-cli`](https://github.com/pystardust/ani-cli) powers the playback handoff used by `ani-tui`
- [`AniList`](https://anilist.co/) provides the metadata used for browsing, search, and cached catalog views

`ani-tui` builds its own local UI, persistence, and heuristic recommendation flow, but playback depends on the external `ani-cli` tool. Credit to the `ani-cli` project for the playback engine this app integrates with.

## Releases

Source builds are the current installation path for this fork. Upstream distribution instructions are available in [the original project](https://github.com/logando-al/ani-tui). The executable is still named `ani-tui`, and the Rust package is `ani-tui-app`.

## License

MIT. See `LICENSE`.
