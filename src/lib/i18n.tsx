import {
  createContext,
  ReactNode,
  useContext,
  useEffect,
  useMemo,
  useState,
} from "react";

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
  "Daily Focus Goal": "每日专注目标",
  "Progress appears on Today.": "进度会显示在“今天”页面。",
  "Focus Block Gap": "专注块间隔",
  "Longer idle periods end a focus block.": "更长的空闲时间会结束当前专注块。",
  "Break Reminders": "休息提醒",
  "Show a local reminder after continuous focus.": "持续专注后显示本地提醒。",
  "Notification Permission": "通知权限",
  Privacy: "隐私",
  Data: "数据",
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
};

const replacements: Array<[RegExp, (match: RegExpMatchArray) => string]> = [
  [/^(\d+) sessions?$/, (m) => `${m[1]} 个会话`],
  [/^(\d+) selected$/, (m) => `已选择 ${m[1]} 项`],
  [/^Show more sessions \((\d+) of (\d+)\)$/, (m) => `显示更多会话（${m[1]}/${m[2]}）`],
  [/^Last (\d+) active days$/, (m) => `最近 ${m[1]} 个活跃日`],
  [/^(\d+)d$/, (m) => `${m[1]} 天`],
  [/^(\d+) min$/, (m) => `${m[1]} 分钟`],
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

interface LocaleContextValue {
  locale: Locale;
  setLocale: (locale: Locale) => void;
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
    setLocale(next: Locale) {
      window.localStorage.setItem(STORAGE_KEY, next);
      setLocaleState(next);
    },
  }), [locale]);

  useEffect(() => {
    document.documentElement.lang = locale;
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
