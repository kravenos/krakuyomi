# MangaFire KindleHF preview: manual install and test

Status: ready for manual Kindle testing. KindleHF packaging, Rust CI and Lua CI
passed. No merge or release. On-device acceptance is still pending.

Candidate: `1.41.4+ci.94.4f6190c`, build `kindlehf`, commit
`4f6190ce43b8d3f155c8d41ca5ea611b06b6a817`.
[Combined preview build](https://github.com/kravenos/krakuyomi/actions/runs/35637875528).

[Download kindlehf build](https://github.com/kravenos/krakuyomi/actions/runs/35637875528/artifacts/10657483211)
(outer artifact ZIP: 16,059,380 bytes; SHA-256
`1334be5a685ce80d9d346d89191b909cb599c918cc2fc937a685f185281169a0`).
CI logs confirm the hard-float target `arm-unknown-linux-musleabihf`, successful
static-binary packaging checks, and the version above. Codex verified GitHub's
metadata/logs without downloading or copying the archive; check `BUILD_INFO.json`
yourself during installation.

The wider platform run is not fully green: all three Android jobs failed during
SDK setup, before app compilation (the inspected job could not find package
`tools`). This does not invalidate the separate successful KindleHF job, but
Android readiness is not claimed. Other platform jobs were still running at
the Kindle handoff.

This preview combines the MangaFire saved-library fix with the reading-direction
and cover-rendering fixes already used in `1.41.4+ci.93.fffe1c3`.
It is not a release. Do not install an individual feature PR's build instead.

## Install manually

1. Fully exit KOReader, then connect the Kindle to your PC.
2. Back up both complete folders to a new dated PC folder:
   - `Internal Storage/koreader/plugins/rakuyomi.koplugin`
   - `Internal Storage/koreader/rakuyomi`
   Include any database WAL/SHM files and source packages. Keep these together;
   do not combine parts of different backups.
3. From the build page above, download **kindlehf build** under Artifacts.
4. Extract the outer artifact ZIP and then the plugin ZIP inside it. In the
   extracted plugin, open `BUILD_INFO.json` and confirm:
   `version` = `1.41.4+ci.94.4f6190c`; `build` = `kindlehf`.
5. Replace only the installed plugin folder's contents with the new plugin
   contents. Do not nest `rakuyomi.koplugin` inside another folder of that name.
6. Leave the live `Internal Storage/koreader/rakuyomi` data folder unchanged.
   Do not update/reinstall MangaFire, remove library titles, or re-add them.
7. Safely eject the Kindle, disconnect it, and start KOReader.

## Test in this order

1. From Library, open one previously failing MangaFire title directly. Open its
   chapter list and an unread chapter without going through Search.
2. Refresh that title. Confirm the chapter count and read markers remain correct.
3. Refresh the whole library. Check whether the previous 22 MangaFire failures
   are gone. Stop if progress vanishes, duplicates appear, or counts drop.
4. With Wi-Fi off, open a previously downloaded chapter. Turn Wi-Fi back on and
   download a new chapter from an existing MangaFire library title.
5. Search MangaFire, open a result and check that this still works. Also check
   another source; retain MangaKatana Server 3.
6. Restart KOReader. Confirm progress persists and reading direction stays the
   same across a chapter change.

Report whether direct opening, refresh, new downloads, old offline downloads,
search and restart persistence pass. If anything fails, send the exact message;
do not repeatedly refresh or remove/re-add titles to repair it.

## Roll back if needed

Fully exit KOReader. Restore the backed-up plugin folder manually. Leave the
data folder alone unless actual data damage occurred. If restoration of data is
needed, restore the entire matching snapshot while KOReader is closed, never
individual database files from different times. Keep the failing snapshot for
diagnosis rather than overwriting the only copy.

Codex has not copied any device files. Kindle acceptance is pending your results.
