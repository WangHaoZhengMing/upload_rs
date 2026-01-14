mod api;
mod app;
mod config;

use anyhow::Result;
use app::state::AppState;
use tracing::info;

use crate::app::workflow::pipeline::run;

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志
    app::logger::init();
    
    info!("正在初始化应用状态...");
    
    // 创建共享状态
    let state = AppState::new().await?;
    
    
    run(state).await?;

    Ok(())
}


