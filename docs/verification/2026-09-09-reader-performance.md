# Kindle reader and performance retest

Status: candidate build pending. Not a release. Fork PRs #34 (reading preferences)
and #35 (cover rendering) remain separate; use their combined preview package.

Candidate: `1.41.4+ci.93.fffe1c3`, from commit `fffe1c3` in
[combined build 34356659856](https://github.com/kravenos/krakuyomi/actions/runs/34356659856).
Wait for its `build (kindlehf, ubuntu-latest)` job to pass before installing.

## Manual installation

1. Keep your current working package and recovery backups. Fully exit KOReader.
2. Download the **kindlehf build** artifact from the combined preview run linked
   in the task. Do not install either individual PR's package: each has only one fix.
3. Extract the artifact's outer ZIP, then its plugin ZIP. Check that `BUILD_INFO.json`
   contains version `1.41.4+ci.93.fffe1c3` and build `kindlehf`.
4. Manually back up the current `Internal Storage/koreader/plugins/rakuyomi.koplugin`
   folder to your PC. Then replace its plugin contents with the candidate contents.
   Avoid nesting one `rakuyomi.koplugin` folder inside another.
5. Do not alter `Internal Storage/koreader/rakuyomi`, your library database,
   downloaded chapters, settings, source packages, or preserved recovery folders.
6. Safely disconnect and start KOReader. All copying is performed by Corvin.

## Reading direction and page style

1. RakuYomi Settings: explicitly select **Left to right**. Read across two chapter
   boundaries; use previous chapter too. Direction must remain left to right.
2. Close/reopen the manga and restart KOReader. Repeat with **Right to left**.
3. Repeat a chapter change with **Continuous scroll**. It must stay scrolling.
4. **Follow viewer mode** means no RakuYomi preference overrides the existing
   source/global/per-manga viewer mode. Previously an unset value misleadingly
   displayed Left to right or Paginated. Existing saved explicit values are kept.
5. Check reading progress, chapter list, Back to library, search, cancellation,
   Sources and diagnosis. The earlier search-fix acceptance remains separate.

## Library UI comparison

Use the same library and grid size. Time opening the library after restart, then
opening it again; page forward/back several times. Check portrait and landscape
covers, border alignment, metadata, and placeholders. Repeat opening/closing to
check for growing sluggishness. Record approximate seconds before/after rather
than assuming the reduced decode count means the entire UI is twice as fast.

## MangaKatana download comparison

1. Sources: open MangaKatana's source settings and note the current **Image Server**.
2. Try **Server 3**, keeping concurrent page requests at the existing value (the
   preserved settings used four). Do not change image optimization or other settings.
3. Download a short chapter that is not already downloaded. Record server choice,
   elapsed time, page count, errors and whether the UI remains responsive.
4. If needed, compare Server 1 with a similar-sized undownloaded chapter. A chapter
   already downloaded is served locally and is not a network speed test. Do not
   delete existing reading material solely for this comparison.
5. Keep the faster reliable server on your device; restore the noted setting if
   another server fails. No automatic source switching or package updates occur.

The source author's public example was sampled once per server on a PC:

| Server | Page-list time | Image headers | First image (589,032 bytes) |
|---|---:|---:|---:|
| 1 | 0.44 s | 1.33 s | 1.36 s |
| 2 | 0.41 s | 6.73 s | 7.09 s |
| 3 | 0.36 s | 0.12 s | 0.16 s |

These samples motivate a Kindle comparison, not a permanent server ranking or a
claim that the source is fixed. No user library identifiers were sent in this test.
Public source references: [server selection](https://github.com/Skittyblock/aidoku-community-sources/blob/main/src/rust/en.mangakatana/src/lib.rs)
and [documented example URL](https://github.com/Skittyblock/aidoku-community-sources/blob/main/src/rust/en.mangakatana/src/helper.rs).

## Rollback

Exit KOReader. Manually restore the plugin folder backed up in step 4; do not
restore or replace the live database just to roll back plugin code. Restore the
previous MangaKatana image-server selection separately if it was changed.
