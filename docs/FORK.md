# Fork guide

This fork builds on [logando-al/ani-tui](https://github.com/logando-al/ani-tui) by Logan. The original MIT license and copyright are preserved. Playback still relies on the external [ani-cli](https://github.com/pystardust/ani-cli) program.

## Visual theme

All screens use `src/ui/theme.rs`. The palette is based on the Amp Alacritty theme; the app embeds these colors and does not read an Alacritty configuration at runtime.

| Role | Color |
| --- | --- |
| Background | `#0F0F0F` |
| Surface | `#161616` |
| Selection surface | `#3A2614` |
| Text | `#F2ECDD` |
| Muted text | `#7A7367` |
| Accent | `#E7894C` |
| Border | `#1A1A1A` |
| Rating / warning | `#E3A25A` |
| Watched / success | `#7C9B96` |

Dialogs share rounded borders and padding. Inactive episodes blend into the surface; the active episode uses orange. Loading placeholders use quiet theme colors rather than generated saturated blocks. Cached synopsis HTML and common entities are converted to readable text.

## Cover images

Home, search, and detail use `PosterCache` in `src/ui/components/posters.rs`. Portrait cards preserve a 2:3 ratio using terminal cell dimensions, with title and rating beneath the image.

Kitty-compatible terminals use native images. Other terminals use actual image pixels represented by halfblocks. Halfblocks have lower resolution than native images. A native poster intersecting a dialog temporarily uses halfblocks so the dialog can safely clear part of the image; posters outside the dialog remain native. Closing the dialog restores native rendering. Kitty image rows restore terminal colors explicitly to prevent accidental background strips.

Downloads run in the background, with at most four concurrent requests. The in-memory cache holds up to 128 anime and persists cover bytes in SQLite. Requests time out after 20 seconds; image data is limited to 4 MiB, decoded dimensions to 4096 pixels per axis, and decode allocation to 64 MiB. Failed covers retry after 60 seconds. Missing images show neutral placeholders.

## Navigation

On home, `h/l` or left/right select cards without moving the viewport until selection leaves its visible edges. `j/k` or up/down switch collections, skipping empty ones.

Search accepts text normally; arrow keys move sequentially through results. The portrait grid changes pages only when selection leaves the current page. Enter opens detail, and Escape closes search.

Detail uses `h/l` or left/right for adjacent episodes and `j/k` or up/down for grid rows. Movement uses the rendered column count, handles a shorter final row, and scrolls only as needed to keep selection visible. Resizing updates the grid geometry. Tab switches focus between episodes and related anime; left/right and Enter then operate on related cards. The detail layout reserves space for episode navigation on shorter terminals.

## Aired episodes versus planned totals

A season's metadata episode count is its planned total, not a list of released streams. The detail grid now uses airing evidence separately: for a 12-episode season with two aired episodes, it displays `2 aired / 12 planned` and offers only E1 and E2. Completed titles with a known positive total retain their full list. Unreleased titles have no selectable episodes; missing counts never fabricate E1. Cancelled titles still use their schedule because some episodes may already have aired.

`src/api/airing.rs` checks the latest past airing schedule and the next future scheduled episode through [AniList's Media API](https://docs.anilist.co/reference/object/media). Successful counts are cached in a separate SQLite `episode_airing` table for fifteen minutes. Queries run in the background with a ten-second timeout. If the request fails, previously confirmed counts remain usable; without confirmation, the app shows a retry message and blocks playback rather than guessing from the planned total. Reopen details to retry.

Resume waits for the airing check, and playback and next-episode actions stay within the confirmed list. Detail history preserves aired counts. Request generations keep late responses from an earlier visit from overwriting reopened details or consuming a new resume request.

These counts describe broadcast airings, not guaranteed stream availability on the current ani-cli provider or dub releases. Provider uploads and translations may lag behind the airing schedule.

## Build and checks

```bash
cargo build --release --locked
cargo check --all-targets --locked
cargo test --locked
cargo clippy --all-targets --locked -- -D warnings
```

The screenshot examples in the README are real captures from this fork in Kitty. No terminal theme file, user database, or personal runtime configuration is bundled.
