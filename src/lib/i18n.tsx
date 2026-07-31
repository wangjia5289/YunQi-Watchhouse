import {
  createContext,
  ReactNode,
  useContext,
  useEffect,
  useMemo,
  useState,
} from "react";
import { setAppLocale } from "./ipc";

export type Locale = "en" | "zh-CN";

const STORAGE_KEY = "watchhouse.locale";
const textOriginals = new WeakMap<Node, string>();
const attributeOriginals = new WeakMap<Element, Map<string, string>>();

const zh: Record<string, string> = {
  Activity: "活动",
  Today: "今天",
  Timeline: "时间线",
  Applications: "应用",
  History: "历史",
  Reports: "报告",
  Settings: "设置",
  "Main navigation": "主导航",
  "Checking status": "正在检查状态",
  "Tracking paused": "追踪已暂停",
  "Tracking active": "追踪中",
  "Click to resume": "点击继续",
  "Click to pause": "点击暂停",
  "Connecting…": "正在连接…",
  Retry: "重试",
  Close: "关闭",
  Cancel: "取消",
  Save: "保存",
  Delete: "删除",
  Edit: "编辑",
  Split: "拆分",
  Clear: "清除",
  Apply: "应用",
  Import: "导入",
  Export: "导出",
  Loading: "加载中",
  "Loading…": "加载中…",
  "No change": "无变化",
  "New activity": "新增活动",
  "Computer activity": "电脑活动",
  Active: "活跃",
  Idle: "空闲",
  Sessions: "会话",
  "Day structure": "全天结构",
  Overview: "概览",
  Details: "详情",
  "Undo history": "撤销历史",
  "Recent timeline edits": "最近的时间线编辑",
  "Undo snapshots expire after 24 hours.": "撤销快照将在 24 小时后过期。",
  "No timeline edits can be undone.": "没有可撤销的时间线编辑。",
  Undo: "撤销",
  "Import Watchhouse data": "导入 Watchhouse 数据",
  "Choose a JSON or CSV export. Nothing is written until you confirm.":
    "选择 JSON 或 CSV 导出文件，确认前不会写入任何数据。",
  "Skip conflicts": "跳过冲突",
  "Merge compatible conflicts": "合并兼容的冲突",
  "Import activity": "导入活动",
  Search: "搜索",
  State: "状态",
  "All activity": "全部活动",
  Advanced: "高级筛选",
  "Hide advanced": "收起高级筛选",
  "Minimum minutes": "最短分钟数",
  "Maximum minutes": "最长分钟数",
  From: "开始",
  To: "结束",
  "No matching activity": "没有匹配的活动",
  "No activity recorded": "没有活动记录",
  "Adjust or clear the filters to see other sessions.": "调整或清除筛选条件以查看其他会话。",
  "Watchhouse did not record any computer activity on this day.":
    "Watchhouse 当天没有记录到电脑活动。",
  "No active application in this hour.": "这一小时没有活跃应用。",
  "Select sessions": "选择会话",
  Merge: "合并",
  Note: "备注",
  Category: "分类",
  Live: "实时",
  "End of recorded activity": "活动记录结束",
  Session: "会话",
  "Edit recorded time": "编辑记录时间",
  "Adjust the start and end of this closed session.": "调整该已结束会话的开始和结束时间。",
  "Save changes": "保存修改",
  "Timeline operation": "时间线操作",
  "Update session notes": "更新会话备注",
  "Change application category": "修改应用分类",
  "Delete recorded sessions": "删除活动会话",
  "Delete sessions": "删除会话",
  "Apply changes": "应用修改",
  "Split recorded session": "拆分活动会话",
  "Create two adjacent sessions at the selected time.": "在所选时间点创建两个相邻会话。",
  "Split session": "拆分会话",
  "Time by application": "应用使用时长",
  "Application usage range": "应用使用范围",
  Week: "周",
  Month: "月",
  Custom: "自定义",
  "Total active time": "总活跃时长",
  "Applications used": "使用过的应用",
  "Most used": "最常使用",
  "Usage by category": "按分类统计",
  "All categories": "全部分类",
  "Active time": "活跃时长",
  "Usage by application": "应用使用情况",
  "No application activity": "没有应用活动",
  "Active applications will appear here as Watchhouse records them.":
    "Watchhouse 记录到活跃应用后会显示在这里。",
  Application: "应用",
  "No bundle identifier": "没有 Bundle 标识符",
  "Ignore future activity": "忽略后续活动",
  "Existing history is preserved.": "已有历史记录会保留。",
  "Record window titles": "记录窗口标题",
  "Used only when the global privacy setting is enabled.": "仅在全局隐私设置启用时生效。",
  Work: "工作",
  Communication: "沟通",
  Learning: "学习",
  Creative: "创作",
  Entertainment: "娱乐",
  Uncategorized: "未分类",
  "7 Days": "7 天",
  "30 Days": "30 天",
  "Share of time": "时长占比",
  "Daily average": "日均时长",
  "Daily trend": "每日趋势",
  "Selected range": "所选范围",
  "No daily trend in this range.": "所选范围内没有每日趋势。",
  "Application details will appear here.": "应用详情会显示在这里。",
  "Long-term patterns": "长期趋势",
  Previous: "上一期",
  Next: "下一期",
  "Average daily active time": "日均活跃时长",
  "Most used app": "最常用应用",
  "Active and idle": "活跃与空闲",
  "Daily activity": "每日活动",
  "No history yet": "暂无历史记录",
  "Recorded activity will appear here.": "记录的活动会显示在这里。",
  "Patterns and progress": "趋势与进展",
  "Report period": "报告周期",
  "Export CSV": "导出 CSV",
  "Report totals": "报告汇总",
  "Idle time": "空闲时长",
  "Focus plans": "专注计划",
  "Plan history": "计划历史",
  "Completion rate": "完成率",
  Completed: "已完成",
  Cancelled: "已取消",
  "Focused time": "专注时长",
  "Longest streak": "最长连续天数",
  "Ended early": "提前结束",
  Repeat: "再次开始",
  "No focus plans ended in this period.": "这一周期内没有结束的专注计划。",
  Trend: "趋势",
  "Daily active time": "每日活跃时长",
  "No activity recorded in this period.": "这一周期内没有活动记录。",
  Rhythm: "节奏",
  "Active time by hour": "每小时活跃时长",
  Allocation: "分布",
  Categories: "分类",
  "No categorized activity in this period.": "这一周期内没有已分类活动。",
  "Today's activity": "今日活动",
  "Needs attention": "需要处理",
  Tracking: "正在追踪",
  "No new activity is being recorded": "当前不会记录新活动",
  "Away from computer": "离开电脑",
  "Starting activity monitor": "正在启动活动监视器",
  "Resume tracking": "继续追踪",
  "Pause tracking": "暂停追踪",
  "Time for a short break": "该休息一下了",
  Dismiss: "忽略",
  "Recorded locally on this Mac": "仅记录在这台 Mac 上",
  "Activity stays private and is stored only on this Mac.":
    "活动数据保持私密，仅存储在这台 Mac 上。",
  "Today summary": "今日汇总",
  "First activity": "首次活动",
  "Started today": "今天开始",
  "Last activity": "最近活动",
  "Latest checkpoint": "最近检查点",
  "Focus summary": "专注汇总",
  "Focused today": "今日专注",
  "Longest block": "最长专注块",
  "App switches": "应用切换",
  "Daily goal": "每日目标",
  Off: "关闭",
  "Focus paused": "专注已暂停",
  "Open-ended focus mode": "不限时专注模式",
  Resume: "继续",
  Pause: "暂停",
  End: "结束",
  Up: "上移",
  Down: "下移",
  Remove: "移除",
  "Start focus plan": "开始专注计划",
  "Focus plan templates": "专注计划模板",
  "Close template editor": "关闭模板编辑器",
  "New template": "新建模板",
  "Template name": "模板名称",
  "Save template": "保存模板",
  "Update template": "更新模板",
  "Loading activity": "正在加载活动",
  Preferences: "偏好设置",
  General: "通用",
  "Launch at Login": "登录时启动",
  "Start Watchhouse when you sign in.": "登录系统时启动 Watchhouse。",
  "Hide to Tray on Close": "关闭时隐藏到菜单栏",
  "Keep tracking when the window closes.": "窗口关闭后继续追踪。",
  "Start Tracking Automatically": "自动开始追踪",
  "Begin recording when Watchhouse starts.": "Watchhouse 启动后开始记录。",
  Language: "语言",
  "Interface language": "界面语言",
  "Choose the language used throughout Watchhouse.": "选择 Watchhouse 的界面语言。",
  English: "English",
  Chinese: "中文",
  "Activity detection": "活动检测",
  "Idle Threshold": "空闲阈值",
  "Changes apply immediately.": "修改会立即生效。",
  "Record Window Titles": "记录窗口标题",
  "Off by default for every application; sensitive text is redacted locally.":
    "每个应用默认关闭，敏感文本会在本机脱敏。",
  "Requires macOS Accessibility permission. Enable it in System Settings, then reopen Watchhouse.":
    "需要 macOS 辅助功能权限。请在系统设置中启用后重新打开 Watchhouse。",
  Appearance: "外观",
  Theme: "主题",
  System: "跟随系统",
  Light: "浅色",
  Dark: "深色",
  Focus: "专注",
  "Goals and breaks": "目标与休息",
  Keyboard: "键盘",
  "Global shortcuts": "全局快捷键",
  "Shortcuts work while Watchhouse is hidden. Disabled actions remain available from the app and tray.":
    "Watchhouse 隐藏时快捷键仍然有效；禁用后仍可从应用或菜单栏执行相应操作。",
  "Start or end focus": "开始或结束专注",
  "Pause or resume focus": "暂停或继续专注",
  "Start first template": "启动首个模板",
  Disabled: "已禁用",
  "Save shortcuts": "保存快捷键",
  "Global shortcuts saved.": "全局快捷键已保存。",
  "each enabled action must use a different shortcut": "每个已启用操作必须使用不同的快捷键。",
  "Daily Focus Goal": "每日专注目标",
  "Progress appears on Today.": "进度会显示在“今天”页面。",
  "Focus Block Gap": "专注块间隔",
  "Longer idle periods end a focus block.": "更长的空闲时间会结束当前专注块。",
  "Break Reminders": "休息提醒",
  "Show a local reminder after continuous focus.": "持续专注后显示本地提醒。",
  "Notification Permission": "通知权限",
  "Allowed. Use a test notification to verify macOS delivery settings.":
    "已允许。可发送测试通知以确认 macOS 投递设置。",
  "Denied. Allow Watchhouse notifications in macOS System Settings.":
    "已拒绝。请在 macOS 系统设置中允许 Watchhouse 通知。",
  "Permission has not been requested.": "尚未请求权限。",
  "Checking notification permission...": "正在检查通知权限…",
  Allowed: "已允许",
  Denied: "已拒绝",
  "Not requested": "未请求",
  Checking: "检查中",
  Allow: "允许",
  "Check Again": "再次检查",
  "Send Test": "发送测试",
  "Notification permission granted.": "通知权限已授予。",
  "Notification permission was not granted.": "通知权限未授予。",
  "Test notification sent.": "测试通知已发送。",
  "Reminder Interval": "提醒间隔",
  "Reminders reset when a new focus block starts.": "新的专注块开始时会重置提醒。",
  "Quiet Hours": "免打扰时段",
  "Break reminders stay silent during this period.": "此时间段内不会发送休息提醒。",
  to: "至",
  Privacy: "隐私",
  Data: "数据",
  "Local storage": "本地存储",
  "Loading database location…": "正在加载数据库位置…",
  "Show in Finder": "在 Finder 中显示",
  "Back Up Database": "备份数据库",
  "Optimize Database": "优化数据库",
  "Export JSON": "导出 JSON",
  "Restore Database Backup": "恢复数据库备份",
  "Restore a Watchhouse database backup? Current activity data will be replaced and tracking will pause.":
    "要恢复 Watchhouse 数据库备份吗？当前活动数据将被替换，追踪也会暂停。",
  "Database restored. Review the data, then resume tracking.":
    "数据库已恢复。请检查数据，然后继续追踪。",
  "Database optimized.": "数据库已优化。",
  "Application icons will be reloaded automatically.": "应用图标将自动重新加载。",
  "Refresh Application Icons": "刷新应用图标",
  Maintenance: "维护",
  "Retention and backups": "保留与备份",
  "Keep Activity": "活动保留期限",
  Forever: "永久",
  "1 year": "1 年",
  "Automatic Backups": "自动备份",
  "Create local SQLite backups on schedule.": "按计划创建本地 SQLite 备份。",
  "Backup Schedule": "备份计划",
  "Old automatic backups are rotated.": "旧的自动备份会轮换删除。",
  Daily: "每天",
  Weekly: "每周",
  "Automatic backups to keep": "自动备份保留数量",
  "Default application data / backups": "默认应用数据/备份目录",
  "Choose Backup Folder": "选择备份文件夹",
  "Show Backup Folder": "显示备份文件夹",
  "Back Up Now": "立即备份",
  "Clean Up Now": "立即清理",
  "Clean unused application data and optimize retention metadata?":
    "要清理未使用的应用数据并优化保留元数据吗？",
  "Data Health": "数据健康",
  "No sessions currently need cleanup.": "当前没有需要清理的会话。",
  "No repairable session problems found.": "未发现可修复的会话问题。",
  "Repair Problems": "修复问题",
  "Repair overlapping and zero-duration closed sessions? Back up first if you may need the original timestamps.":
    "要修复重叠和零时长的已结束会话吗？如果可能需要原始时间戳，请先备份。",
  "Undo Last Repair": "撤销上次修复",
  "Automatic maintenance is running.": "自动维护正在运行。",
  "Delete All Activity Data": "删除全部活动数据",
  "Delete all recorded activity? This cannot be undone.": "要删除所有活动记录吗？此操作无法撤销。",
  "All activity data was deleted.": "所有活动数据已删除。",
  "Local-first protection": "本地优先保护",
  "Review what Watchhouse records and what it deliberately avoids collecting.":
    "查看 Watchhouse 会记录什么，以及明确不会收集什么。",
  "View Privacy Notice": "查看隐私说明",
  "Show Diagnostic Logs": "显示诊断日志",
  About: "关于",
  Version: "版本",
  "Version…": "版本…",
  Database: "数据库",
  WAL: "预写日志",
  Icons: "图标",
  Logs: "日志",
  Backups: "备份",
  "Private, local-first computer activity timeline.": "私密、本地优先的电脑活动时间线。",
  "Settings saved.": "设置已保存。",
  "Loading settings…": "正在加载设置…",
  "Private by design": "隐私优先设计",
  "Welcome to Watchhouse": "欢迎使用 Watchhouse",
  "Privacy in Watchhouse": "Watchhouse 隐私说明",
  "What is recorded": "会记录什么",
  "Application name and identifier": "应用名称和标识符",
  "Active and idle timestamps": "活跃和空闲时间",
  "Session duration": "会话时长",
  "Window titles only after global and per-application opt-in":
    "仅在全局和单个应用均主动启用后记录窗口标题",
  "What is never recorded": "绝不会记录什么",
  "Keystrokes or typed text": "按键或输入内容",
  "Screenshots or screen recordings": "屏幕截图或录屏",
  "Clipboard contents or passwords": "剪贴板内容或密码",
  "Stored locally": "仅存储在本机",
  "Activity remains in the Watchhouse SQLite database. Diagnostics contain only technical lifecycle and error information.":
    "活动数据保存在本机 Watchhouse SQLite 数据库中，诊断信息仅包含技术生命周期和错误信息。",
  "Accept and Continue": "接受并继续",
  "Selected day summary": "所选日期汇总",
  "Timeline view": "时间线视图",
  "Close undo history": "关闭撤销历史",
  "Import conflict policy": "导入冲突处理方式",
  "Dismiss message": "关闭消息",
  "Edit session time": "编辑会话时间",
  "Delete session": "删除会话",
  "Choose a valid split time.": "请选择有效的拆分时间。",
  "Session end must be after its start.": "会话结束时间必须晚于开始时间。",
  "Enter an application category.": "请输入应用分类。",
  "Leave empty to clear existing notes": "留空可清除已有备注",
  "Work, Communication, Learning…": "工作、沟通、学习…",
  "Split session into two parts.": "已将会话拆分为两部分。",
  "These sessions will be removed from the timeline. You can undo this operation afterward.":
    "这些会话将从时间线中删除，之后可以撤销此操作。",
  "Split at": "拆分时间",
  "Application usage summary": "应用使用汇总",
  "Applications ranked by usage": "应用使用时长排名",
  "Focus plan duration": "专注计划时长",
  "Template duration in minutes": "模板时长（分钟）",
  "This week": "本周",
  "This month": "本月",
  Day: "日",
  Until: "截至",
  "← Previous": "← 上一期",
  "Next →": "下一期 →",
  "Report saved to": "报告已保存到",
  "Try again": "重试",
  Start: "开始",
};

const replacements: Array<[RegExp, (match: RegExpMatchArray) => string]> = [
  [/^(\d+) sessions?$/, (m) => `${m[1]} 个会话`],
  [/^(\d+) selected$/, (m) => `已选择 ${m[1]} 项`],
  [/^Show more sessions \((\d+) of (\d+)\)$/, (m) => `显示更多会话（${m[1]}/${m[2]}）`],
  [/^Last (\d+) active days$/, (m) => `最近 ${m[1]} 个活跃日`],
  [/^(\d+)d$/, (m) => `${m[1]} 天`],
  [/^(\d+) min$/, (m) => `${m[1]} 分钟`],
  [/^(\d+) minutes?$/, (m) => `${m[1]} 分钟`],
  [/^(\d+)m$/, (m) => `${m[1]} 分钟`],
  [/^(\d+) hours?$/, (m) => `${m[1]} 小时`],
  [/^(\d+) days?$/, (m) => `${m[1]} 天`],
  [/^Keep (\d+)$/, (m) => `保留 ${m[1]} 个`],
  [/^Version (.+)$/, (m) => `版本 ${m[1]}`],
  [/^Backup saved to (.+)$/, (m) => `备份已保存到 ${m[1]}`],
  [/^Exported to (.+)$/, (m) => `已导出到 ${m[1]}`],
  [/^(\d+) old sessions are eligible for cleanup\.$/, (m) => `${m[1]} 个旧会话可以清理。`],
  [/^Safety backup: (.+)$/, (m) => `安全备份：${m[1]}`],
  [/^Last cleanup: (.+) · Last backup: (.+)$/, (m) => `上次清理：${m[1]} · 上次备份：${m[2]}`],
  [/^Automatic maintenance failed: (.+)$/, (m) => `自动维护失败：${m[1]}`],
  [/^(\d+) overlapping and (\d+) zero-duration sessions found\.$/, (m) =>
    `发现 ${m[1]} 个重叠会话和 ${m[2]} 个零时长会话。`],
  [/^Data repaired: (\d+) trimmed and (\d+) removed\.$/, (m) =>
    `数据修复完成：调整 ${m[1]} 个，删除 ${m[2]} 个。`],
  [/^Restored (\d+) sessions from the last health repair\.$/, (m) =>
    `已从上次健康修复中恢复 ${m[1]} 个会话。`],
  [/^Maintenance complete: (\d+) sessions and (\d+) unused applications removed\.$/, (m) =>
    `维护完成：删除 ${m[1]} 个会话和 ${m[2]} 个未使用应用。`],
  [/^Delete (\d+) expired sessions and unused application data\?$/, (m) =>
    `要删除 ${m[1]} 个过期会话和未使用的应用数据吗？`],
  [/^You have focused for (.+)\.$/, (m) => `你已专注 ${m[1]}。`],
  [/^(.+) remaining$/, (m) => `剩余 ${m[1]}`],
  [/^Focused (.+)$/, (m) => `已专注 ${m[1]}`],
  [/^Planned until (.+)$/, (m) => `计划至 ${m[1]}`],
  [/^Edit (.+) template$/, (m) => `编辑“${m[1]}”模板`],
  [/^Move (.+) template up$/, (m) => `上移“${m[1]}”模板`],
  [/^Move (.+) template down$/, (m) => `下移“${m[1]}”模板`],
  [/^Remove (.+) template$/, (m) => `移除“${m[1]}”模板`],
  [/^(.+): (\d+) starts · (\d+)% complete$/, (m) =>
    `${m[1]}：启动 ${m[2]} 次 · 完成率 ${m[3]}%`],
  [/^Updated notes on (\d+) sessions\.$/, (m) => `已更新 ${m[1]} 个会话的备注。`],
  [/^Updated (\d+) application categories\.$/, (m) => `已更新 ${m[1]} 个应用分类。`],
  [/^Deleted (\d+) sessions\.$/, (m) => `已删除 ${m[1]} 个会话。`],
  [/^Restored (\d+) sessions\.(.*)$/, (m) => `已恢复 ${m[1]} 个会话。${m[2]}`],
  [/^(\d+) undo steps remain\.$/, (m) => `还剩 ${m[1]} 个撤销步骤。`],
  [/^(\d+) recorded days$/, (m) => `记录了 ${m[1]} 天`],
  [/^(\d+) valid sessions$/, (m) => `${m[1]} 个有效会话`],
  [/^(\d+) conflicts$/, (m) => `${m[1]} 个冲突`],
  [/^(\d+) invalid$/, (m) => `${m[1]} 个无效项`],
  [/^Imported (\d+); merged (\d+); skipped (\d+)\.$/, (m) =>
    `已导入 ${m[1]} 个，合并 ${m[2]} 个，跳过 ${m[3]} 个。`],
  [/^Merged (\d+) sessions\.$/, (m) => `已合并 ${m[1]} 个会话。`],
  [/^([+-]?\d+)% vs previous$/, (m) => `较上一周期 ${m[1]}%`],
  [/^Report saved to (.+)$/, (m) => `报告已保存到 ${m[1]}`],
  [/^(\d+)m planned · (.+) focused$/, (m) => `计划 ${m[1]} 分钟 · 专注 ${m[2]}`],
  [/^(\d+)-minute focus plan started\.$/, (m) => `已开始 ${m[1]} 分钟专注计划。`],
  [/^shortcut is unavailable: (.+)$/, (m) => `快捷键不可用：${m[1]}`],
];

export function translateText(value: string): string {
  const trimmed = value.trim();
  let translated = zh[trimmed];
  if (!translated) {
    for (const [pattern, replace] of replacements) {
      const match = trimmed.match(pattern);
      if (match) {
        translated = replace(match);
        break;
      }
    }
  }
  return translated
    ? value.replace(trimmed, translated)
    : value;
}

export function localize(locale: Locale, value: string): string {
  return locale === "zh-CN" ? translateText(value) : value;
}

interface LocaleContextValue {
  locale: Locale;
  setLocale: (locale: Locale) => void;
  t: (value: string) => string;
}

const LocaleContext = createContext<LocaleContextValue | null>(null);

function initialLocale(): Locale {
  const stored = window.localStorage.getItem(STORAGE_KEY);
  if (stored === "en" || stored === "zh-CN") return stored;
  return navigator.language.toLowerCase().startsWith("zh") ? "zh-CN" : "en";
}

export function LocaleProvider({ children }: { children: ReactNode }) {
  const [locale, setLocaleState] = useState<Locale>(initialLocale);
  const value = useMemo(() => ({
    locale,
    t(value: string) {
      return localize(locale, value);
    },
    setLocale(next: Locale) {
      window.localStorage.setItem(STORAGE_KEY, next);
      setLocaleState(next);
    },
  }), [locale]);

  useEffect(() => {
    document.documentElement.lang = locale;
    void setAppLocale(locale).catch(() => {
      // Browser preview does not expose Tauri IPC.
    });
    const visit = (root: Node) => {
      const nodes = root.nodeType === Node.TEXT_NODE
        ? [root]
        : [...(root as Element).querySelectorAll?.("*") ?? [], root];
      for (const node of nodes) {
        if (node.nodeType === Node.TEXT_NODE) {
          const parent = node.parentElement;
          if (!parent || ["SCRIPT", "STYLE"].includes(parent.tagName)) continue;
          const current = node.nodeValue ?? "";
          const cached = textOriginals.get(node);
          const original = cached
            && current !== cached
            && current !== translateText(cached)
            ? current
            : cached ?? current;
          textOriginals.set(node, original);
          const next = locale === "zh-CN" ? translateText(original) : original;
          if (node.nodeValue !== next) node.nodeValue = next;
          continue;
        }
        const element = node as Element;
        for (const name of ["aria-label", "placeholder", "title"]) {
          if (!element.hasAttribute?.(name)) continue;
          let attributes = attributeOriginals.get(element);
          if (!attributes) {
            attributes = new Map();
            attributeOriginals.set(element, attributes);
          }
          const current = element.getAttribute(name) ?? "";
          const cached = attributes.get(name);
          const original = cached
            && current !== cached
            && current !== translateText(cached)
            ? current
            : cached ?? current;
          attributes.set(name, original);
          const next = locale === "zh-CN" ? translateText(original) : original;
          if (element.getAttribute(name) !== next) element.setAttribute(name, next);
        }
        for (const child of element.childNodes) {
          if (child.nodeType === Node.TEXT_NODE) visit(child);
        }
      }
    };
    visit(document.body);
    const observer = new MutationObserver((mutations) => {
      for (const mutation of mutations) {
        if (mutation.type === "characterData" || mutation.type === "attributes") {
          visit(mutation.target);
        }
        for (const node of mutation.addedNodes) visit(node);
      }
    });
    observer.observe(document.body, {
      attributes: true,
      attributeFilter: ["aria-label", "placeholder", "title"],
      childList: true,
      characterData: true,
      subtree: true,
    });
    return () => observer.disconnect();
  }, [locale]);

  return <LocaleContext.Provider value={value}>{children}</LocaleContext.Provider>;
}

export function useLocale(): LocaleContextValue {
  const context = useContext(LocaleContext);
  if (!context) throw new Error("useLocale must be used inside LocaleProvider");
  return context;
}
