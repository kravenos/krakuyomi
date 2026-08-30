use crate::{
    model::SourceInformation, source_collection::SourceCollection, source_manager::SourceManager,
};

pub fn list_installed_sources(source_manager: &SourceManager) -> Vec<SourceInformation> {
    let mut source_informations: Vec<SourceInformation> = source_manager
        .sources()
        .into_iter()
        .map(|source| {
            let mut information: SourceInformation = source.manifest().into();
            if let Some(provenance) = source_manager.source_provenance(&information.id) {
                information.source_of_source =
                    provenance.source_of_source.or(information.source_of_source);
                information.catalog_list_id = provenance.list_id;
                information.provider_url = provenance.provider_url;
                information.resolved_provider_url = provenance.resolved_provider_url;
                information.installed_version = provenance.version;
            }
            information
        })
        .collect();

    source_informations.sort_by_key(|source| source.name.clone());

    source_informations
}
