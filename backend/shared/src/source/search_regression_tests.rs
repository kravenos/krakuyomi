use std::{
    fs::File,
    io::Write,
    sync::{mpsc, Arc},
    thread,
    time::Duration,
};

use tempfile::{tempdir, TempDir};
use zip::{write::SimpleFileOptions, ZipWriter};

use super::{Source, SourceBackend};
use crate::{settings::Settings, source_manager::SourceManager};

fn aidoku_fixture() -> (TempDir, Source) {
    let directory = tempdir().expect("create source fixture directory");
    let package = directory.path().join("fixture.aix");
    let mut archive = ZipWriter::new(File::create(&package).expect("create source fixture"));
    archive
        .start_file("Payload/source.json", SimpleFileOptions::default())
        .expect("start source manifest");
    archive
        .write_all(br#"{"info":{"id":"fixture.busy","name":"Busy fixture","version":1}}"#)
        .expect("write source manifest");
    archive
        .start_file("Payload/main.wasm", SimpleFileOptions::default())
        .expect("start empty WASM module");
    archive
        .write_all(b"\0asm\x01\0\0\0")
        .expect("write valid empty WASM module");
    archive.finish().expect("finish source fixture");

    let manager = Arc::new(tokio::sync::Mutex::new(
        SourceManager::from_folder(directory.path().to_path_buf(), Settings::default())
            .expect("create source manager"),
    ));
    let source = Source::from_aix_file(&package, &manager.blocking_lock(), &manager)
        .expect("load real Aidoku source");
    (directory, source)
}

#[test]
fn manifest_remains_available_while_aidoku_worker_is_busy() {
    let (_directory, source) = aidoku_fixture();
    let SourceBackend::Aidoku(worker) = &source.backend else {
        panic!("fixture must load the Aidoku backend");
    };

    // A timed-out blocking call can still hold this lock. Metadata used to
    // assemble its error response must not wait for that worker to finish.
    let worker_guard = worker.lock().expect("occupy the source worker");
    let source_reader = source.clone();
    let (started_tx, started_rx) = mpsc::channel();
    let (manifest_tx, manifest_rx) = mpsc::channel();
    let reader = thread::spawn(move || {
        started_tx.send(()).expect("signal metadata reader started");
        let manifest = source_reader.manifest();
        let _ = manifest_tx.send(manifest);
    });

    let started = started_rx.recv_timeout(Duration::from_secs(5));
    let manifest_while_busy = manifest_rx.recv_timeout(Duration::from_secs(5));

    // Release before asserting, including on failure, so the old implementation
    // exits cleanly instead of leaving a blocked test thread behind.
    drop(worker_guard);
    reader.join().expect("metadata reader must not panic");
    started.expect("metadata reader should start within the test deadline");
    let manifest = manifest_while_busy
        .expect("manifest retrieval must complete before the busy worker releases its lock");
    assert_eq!(manifest.info.id, "fixture.busy");
    assert_eq!(manifest.info.name, "Busy fixture");
}

#[cfg(feature = "all")]
#[test]
fn cancelled_search_returns_while_worker_is_busy_without_recording_source_failure() {
    use crate::{
        chapter_storage::ChapterStorage, database::Database, model::SourceId,
        source_health::SourceHealthStore, usecases::search_mangas,
    };
    use tokio_util::sync::CancellationToken;

    let (directory, source) = aidoku_fixture();
    let runtime = tokio::runtime::Runtime::new().expect("create search test runtime");
    let db = runtime
        .block_on(Database::new(&directory.path().join("fixture.sqlite")))
        .expect("create search test database");
    let chapter_storage = ChapterStorage::new(
        directory.path().join("downloads"),
        size::Size::from_megabytes(1),
        false,
    )
    .expect("create search test storage");
    let health = SourceHealthStore::open(directory.path().join("source_health.json"));
    let settings = Settings::default();
    let sources = SourceManager::new(
        directory.path().to_path_buf(),
        [(SourceId::new("fixture.busy".to_owned()), source.clone())]
            .into_iter()
            .collect(),
        settings.clone(),
    );
    let token = CancellationToken::new();
    token.cancel();
    let SourceBackend::Aidoku(worker) = &source.backend else {
        panic!("fixture must load the Aidoku backend");
    };
    let worker_guard = worker.lock().expect("occupy the source worker");
    let (started_tx, started_rx) = mpsc::channel();
    let (finished_tx, finished_rx) = mpsc::channel();
    let search = thread::spawn(move || {
        runtime.block_on(async move {
            started_tx.send(()).expect("signal search started");
            let result = search_mangas(
                &sources,
                &db,
                &chapter_storage,
                &settings,
                token,
                "fixture query".to_owned(),
                &None,
                1,
                1,
                &health,
            )
            .await;
            let _ = finished_tx.send((result, health.summaries().await));
        });
    });
    let started = started_rx.recv_timeout(Duration::from_secs(5));
    let result_while_busy = finished_rx.recv_timeout(Duration::from_secs(5));

    // Release even on failure, allowing the real worker and runtime to shut down.
    drop(worker_guard);
    search.join().expect("search thread must not panic");
    started.expect("search should start within the test deadline");
    let (_result, summaries) = result_while_busy
        .expect("cancelled search must return before the busy worker releases its lock");
    assert!(
        summaries.is_empty(),
        "user cancellation must not record a source success or failure"
    );
}
