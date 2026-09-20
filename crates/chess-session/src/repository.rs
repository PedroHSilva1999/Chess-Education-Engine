use async_trait::async_trait;
use dashmap::DashMap;
use thiserror::Error;
use uuid::Uuid;

use crate::ChessSession;

#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("session storage failed: {0}")]
    Storage(String),
}

#[async_trait]
pub trait SessionRepository: Send + Sync {
    async fn get(&self, id: Uuid) -> Result<Option<ChessSession>, RepositoryError>;
    async fn save(&self, session: ChessSession) -> Result<(), RepositoryError>;
    async fn delete(&self, id: Uuid) -> Result<bool, RepositoryError>;
}

#[derive(Debug, Default)]
pub struct InMemorySessionRepository {
    sessions: DashMap<Uuid, ChessSession>,
}

#[async_trait]
impl SessionRepository for InMemorySessionRepository {
    async fn get(&self, id: Uuid) -> Result<Option<ChessSession>, RepositoryError> {
        Ok(self.sessions.get(&id).map(|entry| entry.clone()))
    }

    async fn save(&self, session: ChessSession) -> Result<(), RepositoryError> {
        self.sessions.insert(session.id, session);
        Ok(())
    }

    async fn delete(&self, id: Uuid) -> Result<bool, RepositoryError> {
        Ok(self.sessions.remove(&id).is_some())
    }
}
