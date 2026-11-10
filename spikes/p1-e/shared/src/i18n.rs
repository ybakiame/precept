//! Exhaustive UI catalog. Repository/user values and protocol evidence are never translated.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Locale {
    #[default]
    En,
    ZhCn,
}
impl Locale {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "en" => Some(Self::En),
            "zh-CN" => Some(Self::ZhCn),
            _ => None,
        }
    }
}
macro_rules! catalog {
    ($($key:ident => ($en:literal, $zh:literal)),* $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum Msg { $($key),* }
        impl Msg { pub const ALL: &'static [Self] = &[$(Self::$key),*]; }
        pub fn tr(locale: Locale, message: Msg) -> &'static str {
            match (locale,message) { $((Locale::En,Msg::$key)=>$en,(Locale::ZhCn,Msg::$key)=>$zh),* }
        }
    };
}
catalog! {
    Preview => ("Design preview", "设计预览"),
    PreviewNote => ("Synthetic data · read-only · no daemon connected", "合成数据 · 只读 · 未连接 Daemon"),
    Dashboard => ("Dashboard", "概览"),
    Projects => ("Workspaces", "工作区"),
    Preferences => ("Preferences", "偏好"),
    Handoff => ("Handoff", "交接"),
    Diagnostics => ("Diagnostics", "诊断"),
    Appearance => ("Appearance", "外观"),
    Settings => ("Settings", "设置"),
    CloseSettings => ("Back to context", "返回上下文"),
    ThemeCollection => ("Theme collection", "主题集"),
    ThemeHelp => ("One layout and type scale. Themes change colors only.", "共用布局与字体层级，主题只改变配色。"),
    Selected => ("Selected", "已选择"),
    Sample => ("Aa  Context / 上下文", "Aa  上下文 / Context"),
    ColorMode => ("Color mode", "明暗模式"),
    System => ("System", "跟随系统"),
    Light => ("Light", "浅色"),
    Dark => ("Dark", "深色"),
    HighContrast => ("High contrast", "高对比"),
    ContrastOn => ("High contrast: on", "高对比：开"),
    ContrastOff => ("High contrast: off", "高对比：关"),
    Language => ("Language", "语言"),
    SessionOnly => ("Appearance and language apply to this session only.", "外观与语言仅用于本次会话。"),
    SystemHelp => ("System follows the appearance reported by this runtime.", "跟随系统使用当前运行时报告的明暗设置。"),
    FontHelp => ("System UI font and CJK fallback; the TUI uses your terminal font.", "使用系统界面字体及中文回退字体；TUI 使用终端字体。"),
    ReadOnly => ("Read-only", "只读"),
    Scope => ("Scope", "范围"),
    Source => ("Source", "来源"),
    CurrentWorkspace => ("Current workspace", "当前工作区"),
    WorkspaceHelp => ("Only allowlisted workspaces. Switching clears the task lookup.", "仅显示允许访问的工作区；切换后清空任务查询。"),
    ClientAccess => ("Agent access", "Agent 接入"),
    ClientPresence => ("Client presence: not observed", "客户端在线状态：未观测"),
    ClientHelp => ("Project-scoped MCP access; this console does not launch agents.", "MCP 按项目范围只读访问；本控制台不启动 Agent。"),
    Transport => ("IPC: not connected in preview", "IPC：预览未连接"),
    Tools => ("MCP: two read-only tools", "MCP：两个只读工具"),
    EffectiveContext => ("Effective context", "有效上下文"),
    ContextHelp => ("Live repository facts outrank preferences and earlier handoffs.", "当前仓库事实优先于偏好与历史交接记录。"),
    ContractReferences => ("Project contract references", "项目规范引用"),
    MetadataOnly => ("Metadata only; document bodies are not loaded here.", "此处仅展示引用元数据，未加载文档正文。"),
    ConfirmedPreferences => ("Confirmed preferences", "已确认偏好"),
    ConfirmedOnly => ("Only confirmed records within this scope are included.", "仅包含当前范围内已确认的偏好。"),
    ReviewSuggestions => ("Preference review", "偏好审核"),
    ReviewDeferred => ("Suggestion review is not available in this read-only console.", "当前只读控制台尚不提供建议审核。"),
    ReviewHelp => ("Unconfirmed suggestions never become effective context automatically.", "未确认建议不会自动成为有效上下文。"),
    ResumeSummary => ("Resume context", "恢复任务上下文"),
    OpenHandoff => ("Open Handoff", "查看交接"),
    ExactTask => ("Exact task ID", "精确任务 ID"),
    ExactTaskHelp => ("Task lookup stays within the selected project; no task enumeration.", "仅在所选项目中按精确 ID 查询，不枚举任务。"),
    NoHandoff => ("No task selected. Open the live Handoff page for an exact lookup.", "尚未选择任务。请在交接页输入精确任务 ID。"),
    Objective => ("Recorded objective", "已记录的目标"),
    NextAction => ("Recorded next action", "已记录的下一步"),
    CurrentUncertainty => ("Current repository uncertainty", "当前仓库的不确定项"),
    EarlierNotCurrent => ("Earlier checks do not establish current verification.", "历史检查不能证明当前验证结果。"),
    RecordedEvidence => ("Recorded validation / historical", "已记录的验证 / 历史证据"),
    CurrentValidation => ("Current validation: NOT_RUN", "当前验证：NOT_RUN"),
    EvidenceHelp => ("Evidence retains its original revision, environment and status.", "证据保留原始修订、环境与状态。"),
    ExpandEvidence => ("Expand evidence", "展开证据"),
    CollapseEvidence => ("Collapse evidence", "收起证据"),
    Unknown => ("Unknown", "未知"),
    HasChanges => ("Changes observed", "已观测到修改"),
    NoChanges => ("No changes observed", "未观测到修改"),
    Branch => ("Branch", "分支"),
    Head => ("HEAD", "HEAD"),
    Worktree => ("Worktree", "工作树"),
    Fingerprint => ("Fingerprint", "指纹"),
    ObservedSnapshot => ("Observed repository snapshot", "已观测的仓库快照"),
    UnknownHelp => ("Unknown does not mean a clean or verified repository.", "未知不代表仓库干净或已通过验证。"),
    Minimize => ("Minimize window", "最小化窗口"),
    Maximize => ("Maximize window", "最大化窗口"),
    Restore => ("Restore window", "还原窗口"),
    Close => ("Close window", "关闭窗口"),
    Resize => ("Resize window from bottom right", "从右下角调整窗口大小"),
    Shortcuts => ("Alt 1 overview · Alt 4 handoff · Alt 0 appearance · Ctrl Q closes TUI", "Alt 1 概览 · Alt 4 交接 · Alt 0 外观 · Ctrl Q 退出 TUI"),
    ThemeShortcuts => ("Alt A/B/C theme · Ctrl L language · Esc returns to overview", "Alt A/B/C 切换主题 · Ctrl L 切换语言 · Esc 返回概览"),
    PlainData => ("Project values and evidence remain in their original language.", "项目数据与原始证据保持原文。"),
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn both_catalogs_are_complete_and_controls_are_not_in_translations() {
        for locale in [Locale::En, Locale::ZhCn] {
            for key in Msg::ALL {
                let text = tr(locale, *key);
                assert!(!text.is_empty());
                assert!(!text.chars().any(char::is_control));
            }
        }
        assert_eq!(Locale::parse("zh-CN"), Some(Locale::ZhCn));
        assert_eq!(Locale::parse("en"), Some(Locale::En));
        assert_eq!(Locale::parse("../../profile"), None);
    }
}
