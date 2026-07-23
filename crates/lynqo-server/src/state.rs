use lynqo_core::AppConfig;
use lynqo_db::Database;
use crate::ws::WsHub;

#[derive(Clone)]
pub struct AppState {
    pub db: Database,
    pub ws_hub: WsHub,
    pub config: AppConfig,
}
