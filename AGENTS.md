# YunQi-Watchhouse Agent Instructions

## Git 工作流

当任务涉及新增、修改或删除项目文件时，在完成代码修改后必须执行以下流程。

1. 检查当前修改：

```bash
git status
git diff
```

2. 根据本次修改内容运行适当的检查、构建或测试。

如果测试、编译或检查失败：

- 优先修复由本次修改导致的问题。
- 未解决明显错误前不要提交和推送。
- 如果问题无法解决，在最终回复中明确说明原因。

3. 确认修改完成后，将本次任务相关文件加入暂存区：

```bash
git add -A
```

4. 根据实际修改内容自动生成简洁、准确的 Git Commit Message。

Commit Message 使用 Conventional Commits 风格，例如：

```text
feat: 添加活动时长统计
fix: 修复窗口状态保存异常
refactor: 重构活动记录模块
docs: 更新项目说明
style: 调整主界面布局
chore: 更新项目配置
```

不要使用没有信息量的提交说明，例如：

```text
update
change
修改代码
fix bug
```

5. 创建 Git Commit：

```bash
git commit -m "<根据实际修改生成的提交信息>"
```

6. 提交成功后，将当前提交推送到已经配置好的远程上游分支：

```bash
git push
```

禁止使用：

```bash
git push --force
git push -f
```

除非用户明确要求。

如果 `git push` 失败，不要通过 force push 绕过问题，应分析失败原因并在最终回复中说明。

## 提交范围

只提交与当前任务相关的修改。

不要擅自提交：

- `.env`
- API Key
- Token
- 密码
- 私钥
- 本地开发环境配置
- 其他敏感信息

如果工作区中存在用户之前留下的、与当前任务无关的修改，不要擅自覆盖、删除或回滚。

如果本次任务没有产生任何文件修改，则不要创建空 Commit，也不要执行没有意义的 Push。

## 完成任务后的汇报

每次完成涉及代码修改的任务后，最终回复必须包含：

### 完成内容

简要说明本次实现或修改了什么。

### 主要改动

列出重要修改，例如：

- 新增了哪些功能
- 修改了哪些核心逻辑
- 修复了哪些问题
- 涉及哪些主要文件或模块

### 验证情况

说明执行了哪些：

- 测试
- 编译
- lint
- type check
- cargo check
- cargo test
- npm/pnpm 检查

以及最终是否通过。

### Git 状态

说明：

- Commit Message
- Commit Hash
- 推送的分支
- 是否成功推送到远程仓库

例如：

```text
Git:
- Commit: feat: 添加应用活动时长统计
- Commit Hash: a13bc42
- Branch: main
- Push: 已成功推送到 origin/main
```

## Git 操作原则

默认允许：

```bash
git status
git diff
git add
git commit
git push
git log
```

未经用户明确要求，不执行具有破坏性的 Git 操作，包括但不限于：

```bash
git reset --hard
git clean -fd
git push --force
git rebase
git checkout -- .
git restore .
```
