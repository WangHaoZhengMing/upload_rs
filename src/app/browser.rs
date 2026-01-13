use anyhow::Result;
use chromiumoxide::page::ScreenshotParams;
use tracing::info;

use crate::app::state::AppState;

/// 使用浏览器访问页面并截图
pub async fn navigate_and_screenshot(
    state: &AppState,
    url: &str,
    output_path: &str,
) -> Result<()> {
    info!("正在访问页面: {}", url);
    
    let page = state.page.write().await;
    
    // 导航到指定 URL
    page.goto(url).await?;
    
    // 等待页面加载
    page.wait_for_navigation().await?;
    
    info!("页面加载完成，正在截图...");
    
    // 截图
    let screenshot = page.screenshot(ScreenshotParams::default()).await?;
    
    // 保存到文件
    tokio::fs::write(output_path, screenshot).await?;
    
    info!("截图已保存到: {}", output_path);
    
    Ok(())
}

/// 获取页面内容
pub async fn get_page_content(state: &AppState, url: &str) -> Result<String> {
    info!("正在获取页面内容: {}", url);
    
    let page = state.page.write().await;
    
    page.goto(url).await?;
    page.wait_for_navigation().await?;
    
    let content = page.content().await?;
    
    Ok(content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::logger;
    use crate::app::state::AppState;

    #[tokio::test]
    async fn test_navigate() {
        logger::init_test();
        
        let state = AppState::new().await.expect("创建状态失败");
        
        let result = get_page_content(&state, "https://example.com").await;
        
        assert!(result.is_ok());
        let content = result.unwrap();
        assert!(content.contains("Example Domain"));
    }
}
