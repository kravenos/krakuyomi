# [1.39.0](https://github.com/kravenos/krakuyomi/compare/v1.38.0...v1.39.0) (2026-07-16)


### Bug Fixes

* **database:** explicit i64 override for total_chapters_count subquery ([29986fc](https://github.com/kravenos/krakuyomi/commit/29986fca34301ce359a185fd7456aa423a874bab))
* **database:** explicit type overrides for computed library/playlist columns ([dbd66ca](https://github.com/kravenos/krakuyomi/commit/dbd66cadffd5fe8dfe9532d40b2dba4b86a1bfd7))
* **database:** order unread sorting variants by the aggregate expression ([510d95d](https://github.com/kravenos/krakuyomi/commit/510d95dacf0ec86bd4458a87cafdc134166126af))
* **database:** tolerate applied migrations missing from the source tree ([1762610](https://github.com/kravenos/krakuyomi/commit/176261011ab3eead1158c75777ce845521f91894))
* **settings:** stop top-zone gesture from hijacking setting row taps ([f041aaf](https://github.com/kravenos/krakuyomi/commit/f041aafc3e4d34194f92f7d0733d264673b85e50))


### Features

* library UX improvements, auto-delete, storage stats, reader prefs ([b284d40](https://github.com/kravenos/krakuyomi/commit/b284d40ce855c207b7b90c3bbff08b4ee957e1b7))
* **library:** read/total chapter badge in grid mode; settings value cleanup ([01ed045](https://github.com/kravenos/krakuyomi/commit/01ed045a0cf9fb9425571789db5f02e9f42dc0e5))
* **library:** show total downloaded size in library menu ([75536dc](https://github.com/kravenos/krakuyomi/commit/75536dcfab69a96f717282d74171a565fe8eef5f))

# [1.33.0](https://github.com/kravenos/krakuyomi/compare/v1.32.0...v1.33.0) (2026-07-16)


### Bug Fixes

* add list changes button in context menu if mode tap to continue enable ([#183](https://github.com/kravenos/krakuyomi/issues/183)) ([eabebfa](https://github.com/kravenos/krakuyomi/commit/eabebfa661c34a5ed10b5ff654fbe3260877caf6))
* callback assignment for zen home tab item ([#208](https://github.com/kravenos/krakuyomi/issues/208)) ([4b6d1d0](https://github.com/kravenos/krakuyomi/commit/4b6d1d0e253635e303c35f481dd7ace418539330))
* close_range file not found lua ([3886625](https://github.com/kravenos/krakuyomi/commit/3886625127572e50a555c55a9fe1fb83beeda155))
* correct property name for on_return_callback in MangaSearchResults.lua ([#229](https://github.com/kravenos/krakuyomi/issues/229)) ([686d809](https://github.com/kravenos/krakuyomi/commit/686d809d5dd9315925cfab79fad08ab22304e8ae))
* crash on open Rakuyomi in SimpleUI or Zen UI ([11ad526](https://github.com/kravenos/krakuyomi/commit/11ad526fc1952a138bcd2de4b16f8fd947696d6a))
* **l10n:** add update-trans Makefile target ([93eb38c](https://github.com/kravenos/krakuyomi/commit/93eb38cb8f1a0203508f0f6cc7a5874b3cfb50cc))
* **library:** wrap chapter fetch in Trapper ([#191](https://github.com/kravenos/krakuyomi/issues/191)) ([3f57091](https://github.com/kravenos/krakuyomi/commit/3f570914f6eb345d707b055700adcd60f91f62e1))
* method call to use Shared namespace ([487e396](https://github.com/kravenos/krakuyomi/commit/487e3967df880f884a10c4c3996387c5f8e59a43))
* OTA update never shows the "Restart Now" dialog on old Kindles ([#187](https://github.com/kravenos/krakuyomi/issues/187)) ([f38596e](https://github.com/kravenos/krakuyomi/commit/f38596e81e6c38c87b2b4d427b7a69568de27160))
* **platform:** close FDs in child processes ([#216](https://github.com/kravenos/krakuyomi/issues/216)) ([f53c2f2](https://github.com/kravenos/krakuyomi/commit/f53c2f2d6eaf1c75862be06ae269b7d9ad591cd0))
* **rakuyomi:** prevent network ops when offline ([#190](https://github.com/kravenos/krakuyomi/issues/190)) ([d70b126](https://github.com/kravenos/krakuyomi/commit/d70b126f4f6b71f6c6af2c0075ba2ed12d72634d))
* replace system TLS with manual rustls implementation for ce… ([#225](https://github.com/kravenos/krakuyomi/issues/225)) ([f5a8bd2](https://github.com/kravenos/krakuyomi/commit/f5a8bd24e30e2b2915f561ce9702af38d5a5a518))
* resolve race conditions by capturing chapter ID during preloadin… ([#218](https://github.com/kravenos/krakuyomi/issues/218)) ([0eb10ff](https://github.com/kravenos/krakuyomi/commit/0eb10ff52e0cb28e41748f12fc9f7923b3e8e33a))
* table insertion for context menu buttons ([e73c45d](https://github.com/kravenos/krakuyomi/commit/e73c45d156c40f990c55cc76f42747e0417972b4))
* **tls:** use owned ClientConfig for use_preconfigured_tls and route … ([#246](https://github.com/kravenos/krakuyomi/issues/246)) ([ac8c74a](https://github.com/kravenos/krakuyomi/commit/ac8c74a0559feb3163203d90de5883e732491271))


### Features

* Add backward navigation through chapters ([#212](https://github.com/kravenos/krakuyomi/issues/212)) ([b22523e](https://github.com/kravenos/krakuyomi/commit/b22523e30219ec373d560b5ded0d48fe653a3c6d))
* add configurable chapter title format for ComicInfo.xml metadata ([#253](https://github.com/kravenos/krakuyomi/issues/253)) ([ba56169](https://github.com/kravenos/krakuyomi/commit/ba561696fb6db8173b557f2c085bb6df5fe7f42e))
* add configurable visibility settings for title and metadata in grid mode ([#211](https://github.com/kravenos/krakuyomi/issues/211)) ([4b6cb10](https://github.com/kravenos/krakuyomi/commit/4b6cb10206500b0ca1d2105999628cdc79ac23fa))
* add mode write to ram for protect emmc ([#213](https://github.com/kravenos/krakuyomi/issues/213)) ([9d883a9](https://github.com/kravenos/krakuyomi/commit/9d883a9f28527d8501b7176223d1e175357a6408))
* add new js apis from aidoku-rs SDK ([#238](https://github.com/kravenos/krakuyomi/issues/238)) ([09a972d](https://github.com/kravenos/krakuyomi/commit/09a972d6c6942249b35a01c474580d22a774ff2e))
* add top-zone tap/swipe to open KOReader native top bar across a… ([#252](https://github.com/kravenos/krakuyomi/issues/252)) ([8432777](https://github.com/kravenos/krakuyomi/commit/8432777878e1c477711af097bcee20d0c398fd26))
* **download:** add chapter download progress ([#197](https://github.com/kravenos/krakuyomi/issues/197)) ([a61a2d9](https://github.com/kravenos/krakuyomi/commit/a61a2d9d3d9d6939eb77c4869fe4b4830a513d5f))
* implement Telegram bot for cookie management ([#233](https://github.com/kravenos/krakuyomi/issues/233)) ([148a069](https://github.com/kravenos/krakuyomi/commit/148a06930b1eb72476c07d830ac4f1f5ce82ed2a))
* library UX improvements, auto-delete, storage stats, reader prefs ([b284d40](https://github.com/kravenos/krakuyomi/commit/b284d40ce855c207b7b90c3bbff08b4ee957e1b7))
* **library:** show total downloaded size in library menu ([75536dc](https://github.com/kravenos/krakuyomi/commit/75536dcfab69a96f717282d74171a565fe8eef5f))
* **logging:** add option to disable plugin logging ([#195](https://github.com/kravenos/krakuyomi/issues/195)) ([161f44a](https://github.com/kravenos/krakuyomi/commit/161f44a660c22070f2d74a5da23c10e17857543e))
* luacheck ([#199](https://github.com/kravenos/krakuyomi/issues/199)) ([63b0412](https://github.com/kravenos/krakuyomi/commit/63b041223cf7fbf249195e68736a374e44f756d7))
* **manga:** add per-manga viewer preference ([#241](https://github.com/kravenos/krakuyomi/issues/241)) ([2553704](https://github.com/kravenos/krakuyomi/commit/2553704bbc8bfbca868ecf9f2684e6091d463515))
* **proxy:** add global proxy support ([#239](https://github.com/kravenos/krakuyomi/issues/239)) ([21a73ae](https://github.com/kravenos/krakuyomi/commit/21a73aef5a65235f12f2a23839761fd1d380ab14))
* release ([992bf9e](https://github.com/kravenos/krakuyomi/commit/992bf9ea6d505acdb07c1fdf52d115b73448e598))
* **server:** add auto-stop server on rakuyomi close ([#196](https://github.com/kravenos/krakuyomi/issues/196)) ([afd5d83](https://github.com/kravenos/krakuyomi/commit/afd5d836acab5bfdfb0bf6be3032f95b047056d5))
* **wasm:** register print and abort in std ([b372d7f](https://github.com/kravenos/krakuyomi/commit/b372d7f76e558989c703648fd1a9663071c3350e)), closes [#179](https://github.com/kravenos/krakuyomi/issues/179)


### Performance Improvements

* add file path support to chapters to enable direct access to preloaded content ([ff5c85b](https://github.com/kravenos/krakuyomi/commit/ff5c85b288b59c9ee325be24d4a04e60ede420db))
* add hideTopClose option to LibraryView and refactor backend initialization logic ([8d4337f](https://github.com/kravenos/krakuyomi/commit/8d4337f9a2980c17be7b7f215298403091d42d8e))
* add test cases to rust ([#248](https://github.com/kravenos/krakuyomi/issues/248)) ([cecd3be](https://github.com/kravenos/krakuyomi/commit/cecd3be2f65237cea0319f2ad54aa72038cde0a7))
* implement navigation to specific manga and chapters via file metadata and refactor backend state management ([#231](https://github.com/kravenos/krakuyomi/issues/231)) ([f33e750](https://github.com/kravenos/krakuyomi/commit/f33e750ac94fa178473188ca85cb6415f13395e8))
* maintain hideTopClose state when refreshing LibraryView after callbacks ([8b31fa9](https://github.com/kravenos/krakuyomi/commit/8b31fa973094e43907fde394927008c943ca7f5f))
* optimize server ([#210](https://github.com/kravenos/krakuyomi/issues/210)) ([8917d5e](https://github.com/kravenos/krakuyomi/commit/8917d5ee27ba7365d7cd7b09c32a2afab3e01805))
* **process:** Use FFI for binary execution ([#202](https://github.com/kravenos/krakuyomi/issues/202)) ([98dd669](https://github.com/kravenos/krakuyomi/commit/98dd669434197de37d4dbf2912f1ef402120f4dc))
* revert fix fork because koreader fixed ([#221](https://github.com/kravenos/krakuyomi/issues/221)) ([dcdb820](https://github.com/kravenos/krakuyomi/commit/dcdb8201b7101e646ae92a4c612093585c4add19)), closes [#216](https://github.com/kravenos/krakuyomi/issues/216)
* **unix:** replace fork with posix_spawn ([#242](https://github.com/kravenos/krakuyomi/issues/242)) ([cef20bc](https://github.com/kravenos/krakuyomi/commit/cef20bc1af2b14af645b33dce301696788975ec0))
* update Rust dependencies, implement ZIP comment metadata for chapter origin, and enforce SQL query safety ([#219](https://github.com/kravenos/krakuyomi/issues/219)) ([3f3b2f4](https://github.com/kravenos/krakuyomi/commit/3f3b2f4b384ef61a473c87ae39d6236118566ba4))

# [1.38.0](https://github.com/tachibana-shin/rakuyomi/compare/v1.37.2...v1.38.0) (2026-07-15)


### Features

* add configurable chapter title format for ComicInfo.xml metadata ([#253](https://github.com/tachibana-shin/rakuyomi/issues/253)) ([ba56169](https://github.com/tachibana-shin/rakuyomi/commit/ba561696fb6db8173b557f2c085bb6df5fe7f42e))
* add top-zone tap/swipe to open KOReader native top bar across a… ([#252](https://github.com/tachibana-shin/rakuyomi/issues/252)) ([8432777](https://github.com/tachibana-shin/rakuyomi/commit/8432777878e1c477711af097bcee20d0c398fd26))

## [1.37.2](https://github.com/tachibana-shin/rakuyomi/compare/v1.37.1...v1.37.2) (2026-07-14)


### Performance Improvements

* add test cases to rust ([#248](https://github.com/tachibana-shin/rakuyomi/issues/248)) ([cecd3be](https://github.com/tachibana-shin/rakuyomi/commit/cecd3be2f65237cea0319f2ad54aa72038cde0a7))

## [1.37.1](https://github.com/tachibana-shin/rakuyomi/compare/v1.37.0...v1.37.1) (2026-07-14)


### Bug Fixes

* **tls:** use owned ClientConfig for use_preconfigured_tls and route … ([#246](https://github.com/tachibana-shin/rakuyomi/issues/246)) ([ac8c74a](https://github.com/tachibana-shin/rakuyomi/commit/ac8c74a0559feb3163203d90de5883e732491271))

# [1.37.0](https://github.com/tachibana-shin/rakuyomi/compare/v1.36.11...v1.37.0) (2026-07-13)


### Features

* add new js apis from aidoku-rs SDK ([#238](https://github.com/tachibana-shin/rakuyomi/issues/238)) ([09a972d](https://github.com/tachibana-shin/rakuyomi/commit/09a972d6c6942249b35a01c474580d22a774ff2e))
* implement Telegram bot for cookie management ([#233](https://github.com/tachibana-shin/rakuyomi/issues/233)) ([148a069](https://github.com/tachibana-shin/rakuyomi/commit/148a06930b1eb72476c07d830ac4f1f5ce82ed2a))
* **manga:** add per-manga viewer preference ([#241](https://github.com/tachibana-shin/rakuyomi/issues/241)) ([2553704](https://github.com/tachibana-shin/rakuyomi/commit/2553704bbc8bfbca868ecf9f2684e6091d463515))
* **proxy:** add global proxy support ([#239](https://github.com/tachibana-shin/rakuyomi/issues/239)) ([21a73ae](https://github.com/tachibana-shin/rakuyomi/commit/21a73aef5a65235f12f2a23839761fd1d380ab14))


### Performance Improvements

* **unix:** replace fork with posix_spawn ([#242](https://github.com/tachibana-shin/rakuyomi/issues/242)) ([cef20bc](https://github.com/tachibana-shin/rakuyomi/commit/cef20bc1af2b14af645b33dce301696788975ec0))

## [1.36.11](https://github.com/tachibana-shin/rakuyomi/compare/v1.36.10...v1.36.11) (2026-07-09)


### Bug Fixes

* correct property name for on_return_callback in MangaSearchResults.lua ([#229](https://github.com/tachibana-shin/rakuyomi/issues/229)) ([686d809](https://github.com/tachibana-shin/rakuyomi/commit/686d809d5dd9315925cfab79fad08ab22304e8ae))


### Performance Improvements

* implement navigation to specific manga and chapters via file metadata and refactor backend state management ([#231](https://github.com/tachibana-shin/rakuyomi/issues/231)) ([f33e750](https://github.com/tachibana-shin/rakuyomi/commit/f33e750ac94fa178473188ca85cb6415f13395e8))

## [1.36.10](https://github.com/tachibana-shin/rakuyomi/compare/v1.36.9...v1.36.10) (2026-07-09)


### Bug Fixes

* replace system TLS with manual rustls implementation for ce… ([#225](https://github.com/tachibana-shin/rakuyomi/issues/225)) ([f5a8bd2](https://github.com/tachibana-shin/rakuyomi/commit/f5a8bd24e30e2b2915f561ce9702af38d5a5a518))

## [1.36.9](https://github.com/tachibana-shin/rakuyomi/compare/v1.36.8...v1.36.9) (2026-07-07)


### Bug Fixes

* resolve race conditions by capturing chapter ID during preloadin… ([#218](https://github.com/tachibana-shin/rakuyomi/issues/218)) ([0eb10ff](https://github.com/tachibana-shin/rakuyomi/commit/0eb10ff52e0cb28e41748f12fc9f7923b3e8e33a))


### Performance Improvements

* revert fix fork because koreader fixed ([#221](https://github.com/tachibana-shin/rakuyomi/issues/221)) ([dcdb820](https://github.com/tachibana-shin/rakuyomi/commit/dcdb8201b7101e646ae92a4c612093585c4add19)), closes [#216](https://github.com/tachibana-shin/rakuyomi/issues/216)

## [1.36.8](https://github.com/tachibana-shin/rakuyomi/compare/v1.36.7...v1.36.8) (2026-07-07)


### Bug Fixes

* method call to use Shared namespace ([487e396](https://github.com/tachibana-shin/rakuyomi/commit/487e3967df880f884a10c4c3996387c5f8e59a43))

## [1.36.7](https://github.com/tachibana-shin/rakuyomi/compare/v1.36.6...v1.36.7) (2026-07-06)


### Performance Improvements

* update Rust dependencies, implement ZIP comment metadata for chapter origin, and enforce SQL query safety ([#219](https://github.com/tachibana-shin/rakuyomi/issues/219)) ([3f3b2f4](https://github.com/tachibana-shin/rakuyomi/commit/3f3b2f4b384ef61a473c87ae39d6236118566ba4))

## [1.36.6](https://github.com/tachibana-shin/rakuyomi/compare/v1.36.5...v1.36.6) (2026-07-03)


### Bug Fixes

* close_range file not found lua ([3886625](https://github.com/tachibana-shin/rakuyomi/commit/3886625127572e50a555c55a9fe1fb83beeda155))

## [1.36.5](https://github.com/tachibana-shin/rakuyomi/compare/v1.36.4...v1.36.5) (2026-07-02)


### Bug Fixes

* **platform:** close FDs in child processes ([#216](https://github.com/tachibana-shin/rakuyomi/issues/216)) ([f53c2f2](https://github.com/tachibana-shin/rakuyomi/commit/f53c2f2d6eaf1c75862be06ae269b7d9ad591cd0))

## [1.36.4](https://github.com/tachibana-shin/rakuyomi/compare/v1.36.3...v1.36.4) (2026-06-29)


### Performance Improvements

* maintain hideTopClose state when refreshing LibraryView after callbacks ([8b31fa9](https://github.com/tachibana-shin/rakuyomi/commit/8b31fa973094e43907fde394927008c943ca7f5f))

## [1.36.3](https://github.com/tachibana-shin/rakuyomi/compare/v1.36.2...v1.36.3) (2026-06-29)


### Performance Improvements

* add hideTopClose option to LibraryView and refactor backend initialization logic ([8d4337f](https://github.com/tachibana-shin/rakuyomi/commit/8d4337f9a2980c17be7b7f215298403091d42d8e))

## [1.36.2](https://github.com/tachibana-shin/rakuyomi/compare/v1.36.1...v1.36.2) (2026-06-28)


### Performance Improvements

* add file path support to chapters to enable direct access to preloaded content ([ff5c85b](https://github.com/tachibana-shin/rakuyomi/commit/ff5c85b288b59c9ee325be24d4a04e60ede420db))


### Reverts

* Revert "fix(manga-reader): apply file manager override to zen UI ([#198](https://github.com/tachibana-shin/rakuyomi/issues/198))" ([012fff7](https://github.com/tachibana-shin/rakuyomi/commit/012fff7ac4f1f31865330888f6f69ef05185b8d5))

## [1.36.1](https://github.com/tachibana-shin/rakuyomi/compare/v1.36.0...v1.36.1) (2026-06-27)


### Bug Fixes

* **l10n:** add update-trans Makefile target ([93eb38c](https://github.com/tachibana-shin/rakuyomi/commit/93eb38cb8f1a0203508f0f6cc7a5874b3cfb50cc))

# [1.36.0](https://github.com/tachibana-shin/rakuyomi/compare/v1.35.2...v1.36.0) (2026-06-27)


### Features

* Add backward navigation through chapters ([#212](https://github.com/tachibana-shin/rakuyomi/issues/212)) ([b22523e](https://github.com/tachibana-shin/rakuyomi/commit/b22523e30219ec373d560b5ded0d48fe653a3c6d))
* add configurable visibility settings for title and metadata in grid mode ([#211](https://github.com/tachibana-shin/rakuyomi/issues/211)) ([4b6cb10](https://github.com/tachibana-shin/rakuyomi/commit/4b6cb10206500b0ca1d2105999628cdc79ac23fa))
* add mode write to ram for protect emmc ([#213](https://github.com/tachibana-shin/rakuyomi/issues/213)) ([9d883a9](https://github.com/tachibana-shin/rakuyomi/commit/9d883a9f28527d8501b7176223d1e175357a6408))

## [1.35.2](https://github.com/tachibana-shin/rakuyomi/compare/v1.35.1...v1.35.2) (2026-06-25)


### Performance Improvements

* optimize server ([#210](https://github.com/tachibana-shin/rakuyomi/issues/210)) ([8917d5e](https://github.com/tachibana-shin/rakuyomi/commit/8917d5ee27ba7365d7cd7b09c32a2afab3e01805))

## [1.35.1](https://github.com/tachibana-shin/rakuyomi/compare/v1.35.0...v1.35.1) (2026-06-25)


### Bug Fixes

* callback assignment for zen home tab item ([#208](https://github.com/tachibana-shin/rakuyomi/issues/208)) ([4b6d1d0](https://github.com/tachibana-shin/rakuyomi/commit/4b6d1d0e253635e303c35f481dd7ace418539330))

# [1.35.0](https://github.com/tachibana-shin/rakuyomi/compare/v1.34.1...v1.35.0) (2026-06-19)


### Bug Fixes

* **manga-reader:** apply file manager override to zen UI ([#198](https://github.com/tachibana-shin/rakuyomi/issues/198)) ([215f224](https://github.com/tachibana-shin/rakuyomi/commit/215f2245d0487a37a9d697aee49ca676b2f73455))
* OTA update never shows the "Restart Now" dialog on old Kindles ([#187](https://github.com/tachibana-shin/rakuyomi/issues/187)) ([f38596e](https://github.com/tachibana-shin/rakuyomi/commit/f38596e81e6c38c87b2b4d427b7a69568de27160))


### Features

* **download:** add chapter download progress ([#197](https://github.com/tachibana-shin/rakuyomi/issues/197)) ([a61a2d9](https://github.com/tachibana-shin/rakuyomi/commit/a61a2d9d3d9d6939eb77c4869fe4b4830a513d5f))
* **logging:** add option to disable plugin logging ([#195](https://github.com/tachibana-shin/rakuyomi/issues/195)) ([161f44a](https://github.com/tachibana-shin/rakuyomi/commit/161f44a660c22070f2d74a5da23c10e17857543e))
* luacheck ([#199](https://github.com/tachibana-shin/rakuyomi/issues/199)) ([63b0412](https://github.com/tachibana-shin/rakuyomi/commit/63b041223cf7fbf249195e68736a374e44f756d7))
* **server:** add auto-stop server on rakuyomi close ([#196](https://github.com/tachibana-shin/rakuyomi/issues/196)) ([afd5d83](https://github.com/tachibana-shin/rakuyomi/commit/afd5d836acab5bfdfb0bf6be3032f95b047056d5))


### Performance Improvements

* **process:** Use FFI for binary execution ([#202](https://github.com/tachibana-shin/rakuyomi/issues/202)) ([98dd669](https://github.com/tachibana-shin/rakuyomi/commit/98dd669434197de37d4dbf2912f1ef402120f4dc))
