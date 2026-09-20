//! HTTP and gRPC transport adapters sharing one application service.

pub mod grpc;

mod error;
mod rest;
mod static_assets;

use std::sync::Arc;

use chess_core::{ChessRulesEngine, ShakmatyRules};
use chess_education::{DefaultExerciseFactory, DefaultObjectiveEvaluator};
use chess_engine::SimpleEngine;
use chess_session::{InMemorySessionRepository, SessionService};

pub use rest::{AppState, router};

#[must_use]
pub fn application_service() -> Arc<SessionService> {
    let rules: Arc<dyn ChessRulesEngine> = Arc::new(ShakmatyRules);
    Arc::new(SessionService::new(
        Arc::new(InMemorySessionRepository::default()),
        Arc::clone(&rules),
        Arc::new(DefaultExerciseFactory::new(Arc::clone(&rules))),
        Arc::new(DefaultObjectiveEvaluator),
        Arc::new(SimpleEngine::new(rules)),
    ))
}
