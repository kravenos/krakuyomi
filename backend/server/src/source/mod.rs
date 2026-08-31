mod diagnosis;
mod routes;

use axum::Router;

use crate::state::State;

/// Returns all source-management and source-diagnosis routes.
pub fn routes() -> Router<State> {
    Router::new()
        .merge(routes::routes())
        .merge(diagnosis::routes())
}
