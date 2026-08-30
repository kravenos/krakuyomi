use std::path::PathBuf;

use anyhow::Result;

use crate::{
    settings::SourceList,
    source_catalog::{CatalogCandidate, CatalogStore},
};

/// Refreshes configured source catalogs and returns all exact candidates.
/// Each candidate records whether deterministic selection prefers it.
pub async fn list_available_sources(
    source_lists: &[SourceList],
    catalog_cache_path: PathBuf,
) -> Result<Vec<CatalogCandidate>> {
    CatalogStore::new(catalog_cache_path)
        .refresh(source_lists)
        .await
}
