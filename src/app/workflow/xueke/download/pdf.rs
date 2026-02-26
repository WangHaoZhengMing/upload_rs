use chromiumoxide::cdp::browser_protocol::page::PrintToPdfParams;
use std::path::Path;
use tokio_retry::Retry;
use tokio_retry::strategy::FixedInterval;
use tracing::warn;

/// 生成 PDF 文件
pub async fn generate_pdf(page: &chromiumoxide::Page, path: &Path) -> anyhow::Result<()> {
    let retry_strategy = FixedInterval::from_millis(1000).take(3);

    Retry::spawn(retry_strategy, || async {
        let params = PrintToPdfParams::default();
        page.save_pdf(params, path).await
    })
    .await
    .map_err(|e| {
        warn!("PDF generation failed after retries: {}", e);
        anyhow::anyhow!(e)
    })?;

    Ok(())
}
