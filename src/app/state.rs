use anyhow::{Context, Result};
use chromiumoxide::Page;
use chromiumoxide::browser::Browser;
use futures::StreamExt;
use std::{env, path::PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::sleep;
use tracing::{debug, info, warn};

/// 应用程序共享状态
#[derive(Clone)]
pub struct AppState {

    /// 浏览器实例
    pub browser: Arc<Browser>,

    /// 页面实例（使用 RwLock 支持并发访问）
    pub page: Arc<RwLock<Page>>,

    /// 应用配置
    pub _config: &'static crate::config::AppConfig,
}

impl AppState {
    /// 创建新的应用状态
    pub async fn new() -> Result<Self> {
        // 连接到浏览器
        let browser = connect_browser().await?;

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
            browser: Arc::new(browser),
            page: Arc::new(RwLock::new(page)),
            _config: crate::config::get(),
        })
    }
}

pub async fn connect_browser() -> Result<Browser> {
    let port = 2001;
    let browser_url = format!("http://localhost:{}", port);
    debug!("尝试连接到现有浏览器: {}", browser_url);

    let is_new_instance;
    let connect_result = Browser::connect(&browser_url).await;

    let (browser, mut handler) = match connect_result {
        Ok(res) => {
            info!("✓ 成功连接到端口 {} 的现有浏览器", port);
            is_new_instance = false;
            res
        }
        Err(_) => {
            warn!("无法连接到端口 {}，准备启动新的 Edge 实例...", port);
            is_new_instance = true;
            launch_edge_process(port)?;

            let mut retries = 20;
            let mut connected_browser = None;
            while retries > 0 {
                sleep(Duration::from_millis(500)).await;
                match Browser::connect(&browser_url).await {
                    Ok(res) => {
                        info!("✓ 新 Edge 启动成功并已连接");
                        connected_browser = Some(res);
                        break;
                    }
                    Err(_) => {
                        debug!("等待浏览器端口就绪... 剩余重试: {}", retries);
                        retries -= 1;
                    }
                }
            }
            connected_browser.ok_or_else(|| anyhow::anyhow!("启动 Edge 后连接超时"))?
        }
    };

    tokio::spawn(async move {
        while let Some(h) = handler.next().await {
            if h.is_err() {
                break;
            }
        }
    });

    if is_new_instance {
        info!("检测到新启动的浏览器实例，等待 10 秒供用户操作（如扫码登录）...");
        for i in (1..=10).rev() {
            if i % 2 == 0 {
                info!("等待中... 剩余 {} 秒", i);
            }
            sleep(Duration::from_secs(1)).await;
        }
        info!("等待结束，开始执行自动化任务");
    } else {
        debug!("复用现有实例，无需等待，立即执行");
    }

    Ok(browser)
}



fn launch_edge_process(port: u16) -> Result<()> {
    // ========== 1. 跨平台获取用户主目录 ==========
    let (base_user_data_dir, browser_exec_args) = if cfg!(target_os = "windows") {
        // Windows 逻辑
        let user_profile = env::var("USERPROFILE").context("找不到 USERPROFILE")?;
        let base_dir = PathBuf::from(user_profile)
            .join(r"AppData\Local\Microsoft\Edge\User Data");
        // 修改点1：统一用 String 类型，避免 &str 和 String 不兼容
        (base_dir, vec![])
    } else if cfg!(target_os = "macos") {
        // macOS 逻辑
        let home_dir = env::var("HOME").context("找不到 HOME 环境变量")?;
        let base_dir = PathBuf::from(home_dir)
            .join("Library/Application Support/Microsoft Edge");
        // 修改点1：将 &str 改为 String 类型（加 to_string()）
        (base_dir, vec!["--args".to_string()])
    } else {
        return Err(anyhow::anyhow!("暂不支持当前操作系统"));
    };

    // ========== 2. 构造用户数据目录 ==========
    let profile_name = format!("Profile_{}", port);
    let user_data_dir = base_user_data_dir.join(profile_name);

    // ========== 3. 跨平台查找浏览器可执行文件 ==========
    let browser_paths = if cfg!(target_os = "windows") {
        vec![
            r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
            r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
        ]
    } else if cfg!(target_os = "macos") {
        vec![
            "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        ]
    } else {
        vec![]
    };

    let browser_path = browser_paths
        .iter()
        .find(|path| std::path::Path::new(path).exists())
        .ok_or_else(|| anyhow::anyhow!("未找到 Edge/Chrome 浏览器"))?;

    info!("使用浏览器路径: {}", browser_path);

    // ========== 4. 构造启动参数（核心修复） ==========
    // 修改点2：所有参数统一为 String 类型，避免类型不匹配
    let mut args = browser_exec_args; // 现在是 Vec<String> 类型
    args.extend(vec![
        format!("--remote-debugging-port={}", port), // 返回 String
        format!("--user-data-dir={}", user_data_dir.to_string_lossy()), // 返回 String
        "--new-window".to_string(), // &str 转 String
        "--no-first-run".to_string(), // &str 转 String
        "--no-default-browser-check".to_string(), // &str 转 String
    ]);

    // ========== 5. 启动浏览器进程 ==========
    Command::new(browser_path)
        .args(&args)
        .spawn()
        .context("启动浏览器失败")?;

    info!("浏览器已启动，调试端口: {}", port);
    Ok(())
}


#[tokio::test]

async fn test_connect_browser() {
    use crate::app::logger;
    logger::init_test();
    match connect_browser().await {
        Ok(_) => tracing::info!("成功连接到浏览器"),
        Err(e) => tracing::error!("连接浏览器失败: {}", e),
    }
}
