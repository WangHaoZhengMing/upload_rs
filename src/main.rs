mod api;
mod app;
mod config;

use anyhow::Result;
use app::state::AppState;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志
    app::logger::init();
    
    info!("正在初始化应用状态...");
    
    // 创建共享状态
    let state = AppState::new().await?;
    
    info!("应用状态初始化成功");
    info!("浏览器已启动");
    info!("HTTP 客户端已就绪");
    
    // 示例：使用浏览器访问页面
    // browser::navigate_and_screenshot(&state, "https://example.com", "screenshot.png").await?;
    
    // 示例：上传 PDF
    // let url = api::upload::upload_pdf(&state, "test.pdf").await?;
    // let result = api::upload::upload_and_convert_pdf(&state, "doc.pdf").await?;
    
    info!("程序启动完成，按 Ctrl+C 退出");
    
    // 保持程序运行
    tokio::signal::ctrl_c().await?;
    
    info!("正在关闭...");
    Ok(())
}
