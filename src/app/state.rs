use anyhow::Result;
use chromiumoxide::browser::Browser;
use chromiumoxide::Page;
use futures::StreamExt;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 应用程序共享状态
#[derive(Clone)]
pub struct AppState {
    /// HTTP 客户端（用于 API 请求和文件上传）
    pub http_client: reqwest::Client,
    
    /// 浏览器实例
    pub browser: Arc<Browser>,
    
    /// 页面实例（使用 RwLock 支持并发访问）
    pub page: Arc<RwLock<Page>>,
}

impl AppState {
    /// 创建新的应用状态
    pub async fn new() -> Result<Self> {
        // 创建 HTTP 客户端
        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;
        
        // 连接到已有的浏览器实例（端口 2001）
        let ws_url = "ws://127.0.0.1:2001";
        
        tracing::info!("正在连接到浏览器 Debug 端口: {}", ws_url);
        
        let (browser, mut handler) = match Browser::connect(ws_url).await {
            Ok(b) => {
                tracing::info!("成功连接到已有浏览器实例");
                b
            }
            Err(e) => {
                tracing::error!("无法连接到浏览器: {}", e);
                tracing::info!("请确保浏览器已启动并运行在调试端口 2001");
                tracing::info!("启动命令示例: chrome.exe --remote-debugging-port=2001");
                return Err(anyhow::anyhow!("无法连接到浏览器端口 2001: {}", e));
            }
        };
        
        // 在后台处理浏览器事件
        tokio::spawn(async move {
            while let Some(event) = handler.next().await {
                if let Err(e) = event {
                    tracing::warn!("浏览器事件错误: {:?}", e);
                }
            }
        });
        
        // 目标 URL
        let target_url = "https://tk-lpzx.xdf.cn/#/paperEnterList";
        
        // 获取所有已打开的页面
        let pages = browser.pages().await?;
        tracing::info!("当前浏览器中有 {} 个页面", pages.len());
        
        // 查找是否已经有目标页面打开
        let mut found_page: Option<Page> = None;
        for page in pages {
            if let Ok(url) = page.url().await {
                if let Some(u) = url {
                    tracing::debug!("检查页面: {}", u);
                    if u.starts_with("https://tk-lpzx.xdf.cn/") {
                        tracing::info!("找到已存在的目标页面: {}", u);
                        found_page = Some(page);
                        break;
                    }
                }
            }
        }
        
        let page = if let Some(existing_page) = found_page {
            tracing::info!("使用已存在的页面");
            existing_page
        } else {
            tracing::info!("未找到目标页面，正在创建新页面...");
            let new_page = browser.new_page(target_url).await?;
            
            tracing::info!("页面已打开，等待 10 秒以便用户登录...");
            tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
            tracing::info!("等待完成");
            
            new_page
        };
        
        Ok(Self {
            http_client,
            browser: Arc::new(browser),
            page: Arc::new(RwLock::new(page)),
        })
    }
    
    /// 创建用于测试的状态（可选）
    #[cfg(test)]
    pub async fn new_for_test() -> Result<Self> {
        Self::new().await
    }
}
