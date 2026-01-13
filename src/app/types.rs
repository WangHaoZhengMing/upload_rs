/// 处理结果枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessResult {
    /// 处理成功
    Success,
    /// 已存在，跳过
    AlreadyExists,
    /// 处理失败
    Failed,
}

/// 处理统计信息
#[derive(Debug, Default, Clone)]
pub struct ProcessStats {
    /// 成功数量
    pub success: usize,
    /// 已存在数量
    pub exists: usize,
    /// 失败数量
    pub failed: usize,
}

impl ProcessStats {
    /// 添加处理结果到统计
    pub fn add_result(&mut self, result: &ProcessResult) {
        match result {
            ProcessResult::Success => self.success += 1,
            ProcessResult::AlreadyExists => self.exists += 1,
            ProcessResult::Failed => self.failed += 1,
        }
    }
}
