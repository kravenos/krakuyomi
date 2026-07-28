# rakuyomi fork - working plan and divergence record

Fork: `kravenos/krakuyomi` &middot; Upstream: `tachibana-shin/rakuyomi`
Device: Kindle Paperwhite 12 SE, firmware 5.17.1, `kindlehf` build.

This file is the recovery record for the fork. If the working copy is ever
lost, everything needed to rebuild the fork-only work is described here.
Keep it out of upstream pull requests.

---

## 1. Rules of engagement

1. **Never publish a release before on-device testing.** Work lands on a
   branch, CI builds the artifacts, the `kindlehf` zip is staged into
   `dist/rakuyomi.koplugin`, and only after the device is confirmed working
   does anything merge to `main` (where semantic-release publishes).
2. The `release` job is gated to `refs/heads/main`, so branch builds and
   pull requests are safe. `workflow_dispatch` is enabled, so any branch can
   be built on demand without releasing.
3. Keep every platform zip on the release page: `aarch64`, `android`,
   `desktop`, `kindle`, `kindlea9`, `kindlehf`.
4. Fork versions follow "upstream version + fork features". The upstream
   base is tagged locally so semantic-release numbers past it.

## 2. Current state

- Working branch: `fix/settings-corruption-recovery` (settings atomic write
  plus corruption recovery). Built and verified in CI; **awaiting device test**.
- Base: upstream `v1.39.3` merged in `11e4390`.
- Backend suite: 128 tests passing. Lua lint clean apart from one
  pre-existing upstream warning in `PlaylistDialog.lua`.

## 3. Work merged INTO upstream (no longer fork divergence)

Submitted as four focused pull requests after upstream rejected one large
one, and shipped in upstream 1.39.0:

| PR | Content |
|----|---------|
| #256 | Settings and error-handling robustness: non-JSON error bodies, gettext `_` shadowing, settings-screen guards, whole-row taps, top-zone gesture fix, storage-size accounting on delete |
| #257 | `GET /storage-stats` plus the "Total downloaded" settings row |
| #258 | Optional auto-delete of downloaded chapters on remove / on read |
| #259 | Clearer library sorting labels |

Lessons from that review, worth respecting in future submissions:

- The maintainer rejects defensive guards he considers unnecessary, prefers
  short code, and does not want raw server output shown to users.
- Do not modify `default-settings.json`; use serde defaults instead.
- Database schema, SQL, and `.sqlx` metadata are treated as sensitive - avoid
  touching them in upstream work.
- eMMC is very slow. Per-file scanning of the downloads folder is
  unacceptable; prefer counters already held in memory.
- Deletion paths must keep the `chapter_storage` lock; read-only paths may
  clone out of it.

## 4. Fork-only divergence (rebuild list)

Everything below exists only in the fork. Grouped by feature, newest last.

### 4.1 Database resilience
- `aca9ba6` - tolerate applied migrations that are missing from the source
  tree. Shared `Database::migrator()` helper with `set_ignore_missing(true)`,
  reused by `new()` and `hot_replace()`.
  Files: `backend/shared/src/database.rs`.
  Reason: development builds left orphaned migration rows that bricked startup.

### 4.2 Library and reader UX (rejected upstream, kept locally)
- `68e9d27` - tap a manga opens a Continue Reading / Chapter List dialog;
  the context menu always offers Chapter List. Replaces the
  `rakuyomi_tap_manga_action` setting.
- `c8442be` - "Hide fully read manga" toggle (library menu and Settings).
- `8c09157` - reader reading-direction and page-turn-style preferences plus a
  "Back to library" reader menu item.
- `6d692f4` - total downloaded size shown as the library menu title.
  Files: `LibraryView.lua`, `Settings.lua`, `MangaReader.lua`,
  `ChapterListing.lua`.

### 4.3 Configurable tile metadata
- `b6e804e` then reworked in `70d7deb` - a `rakuyomi_tile_metadata`
  multi-enum setting choosing which metadata appears on library tiles
  (last read / unread / read / total; read plus total renders as `12/57`).
  Backend: `CachedChapterCounts { total, read }` filled by ONE grouped query
  in `Database::get_cached_chapter_counts()`, merged in the
  `get_manga_library` and `get_mangas_in_playlist` use cases.
  **Design constraint:** read counts come from an explicit `chapter_state`
  join, never from `total - unread`; those diverge. No per-row subqueries, no
  compile-time query changes, no `.sqlx` regeneration. Measured about 7 ms on
  a 100-manga / 17k-chapter database.
  Files: `database.rs`, `model.rs` (shared and server), `get_manga_library.rs`,
  `get_mangas_in_playlist.rs`, `search_mangas.rs`, `LibraryView.lua`,
  `Settings.lua`, `Backend.lua`, `tests/database_chapter_counts.rs`.

### 4.4 Settings robustness
- `7d50deb`, `c2a254e` - nil-safe rendering: multi-enum values default
  correctly on first open, and enum labels tolerate a value with no matching
  option (this crashed the whole Settings screen, because it is built in one
  pass).
- `c36dd60`, `059fde9`, `befc938` - settings are written atomically (temp file,
  flush, fsync, rename) and an unreadable settings file no longer prevents
  startup: it is preserved and configuration is recovered rather than reset.
  Files: `backend/shared/src/settings/implementation.rs`,
  `backend/server/src/settings_recovery.rs`, `backend/server/src/app.rs`.
  Reason: `File::create` truncated the target before writing, so an
  interrupted save (the auto-kill-server delay makes this routine) left a
  half-written file and the plugin refused to start.

### 4.5 Infrastructure
- `94d8494` - `workflow_dispatch` on the Build workflow.
- `804a83d` - translations for the new reader settings across all locales.

## 5. Planned work

### 5.1 Source management (priority - caused real data trouble)
Removing a source leaves its manga in `manga_library`, so they vanish from the
library but still exist in the database and fail refresh with a migration
error. There is no in-app way to see or fix this.

- Detect and surface orphaned library entries: library rows whose
  `source_id` has no matching `<source_id>.aix` in the sources folder.
- Warn before uninstalling a source that still has library entries, naming
  how many manga would be affected.
- Offer recovery: reinstall the source, or migrate entries to another source,
  or remove them deliberately.
- Until this exists, `dist/diagnose-library.py` reports the same information
  from a copy of the database plus a listing of the sources folder.

### 5.2 Collections / folders (ZenUI style)
Group series by genre and browse a collection through the normal library
layout, each collection shown as a labelled cover tile.
Most of this exists already: `Playlist { id, name }` plus the
`playlist_mangas` join table, and `LibraryView:fetchAndShow(playlist, ...)`
already renders a playlist through the standard grid/cover layout. Remaining
work is a cover image per playlist and rendering the chooser
(`PlaylistDialog.lua`, currently a plain text list) as a cover grid.

### 5.3 Long titles under covers
Grid mode (`patch/MenuItemGrid.lua:57`) uses a single-line `TextWidget` with
`max_width`, so long titles are truncated. Cover mode already solves this in
`patch/MenuItemCover.lua` using
`TextBoxWidget:getFontSizeToFitHeight(max_item_height, 2)` - wrap to two lines
and shrink the font to fit. Port that approach to grid mode.

### 5.4 Outstanding
- Device-test `fix/settings-corruption-recovery`, then merge and release.
- Repair the corrupt `settings.json` on the device, or let the recovery build
  preserve it and start from defaults.

## 6. Build and verify

The backend is Linux-only; build and test through Docker.

**Always export with `git archive`, never mount the working tree.** Windows
`core.autocrlf=true` gives the container CRLF files, which changes the SHA-384
checksums sqlx computes over migrations and produces spurious "migration was
modified" failures.

```sh
git archive <branch> | tar -x -C /tmp/src
cd /tmp/src/backend
SQLX_OFFLINE=true cargo check -p shared -p server
SQLX_OFFLINE=true cargo test  -p shared -p server
```

Lua lint: `docker run --rm -v <export>:/work -w /work pipelinecomponents/luacheck luacheck frontend/`

If `cargo sqlx prepare` is ever needed (avoid it - prefer runtime queries),
pin sqlx-cli 0.9.0, export with `git archive` as above, and give every
computed column an explicit type override such as `AS "count: i64"`.

### Docker Desktop failure specific to this machine
Docker dies at startup with `... <socket>: The file cannot be accessed by the
system`. On this Windows build a unix-socket file cannot be deleted through
normal APIs, so any unclean shutdown leaves one behind and the affected
component crashes the app. Recovery: stop every `*ocker*` process, rename the
parent directory aside (`AppData\Local\Docker\run`,
`AppData\Local\docker-secrets-engine`), relaunch, then confirm the engine
stays up for about 90 seconds before trusting it. `EnableDockerAI: false` in
`AppData\Roaming\Docker\settings-store.json` removes one recurring offender.

## 7. Device notes

- Staged build lives at `dist/rakuyomi.koplugin`; copy it over
  `koreader/plugins/rakuyomi.koplugin`.
- Plugin data on device: `koreader/rakuyomi/` containing `database.db`,
  `settings.json`, `sources/*.aix`, and `downloads/`.
- Library membership is `manga_library`; read state is `chapter_state`.
  Removing a source file does not touch either, so reinstalling the same
  source id restores the library entries intact.
