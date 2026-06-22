# Distill Report — 2026-06-21

## Project: meatshell (E:\code\meatshell)
**Database**: `C:\Users\Capkin\.local\share\mimocode\mimocode.db`  
**Review window**: All available history (project is new — 3 sessions, all from 2026-06-21)

---

## Existing Assets Inventory

### Project-level (.mimocode/)
- **None** — no skills, commands, or agents exist yet.

### Global (C:\Users\Capkin\.claude\plugins\cache\superpowers-marketplace\)
14 skills, 3 commands, 1 agent from the superpowers plugin:
- Skills: brainstorming, writing-plans, executing-plans, TDD, systematic-debugging, dispatching-parallel-agents, verification-before-completion, finishing-a-development-branch, code-review (x2), git-worktrees, subagent-driven-development, writing-skills, using-superpowers
- Commands: write-plan, execute-plan, brainstorm
- Agents: code-reviewer

---

## Sessions Analyzed

### meatshell project (3 sessions)
| Session | Title | Date |
|---------|-------|------|
| ses_115c60793ffe | 增加显卡监控功能 | 2026-06-21 |
| ses_115c60770ffe | Auto Dream | 2026-06-21 |
| ses_115c6076affe | Auto Distill | 2026-06-21 |

### Cross-project sessions reviewed (27 sessions total across hlpt, jiazheng-new, exercise_hourse_manager, gp/my_gp)

---

## Shortlist of Candidates

### 1. "请帮我分析这个代码库并创建 CLAUDE.md 文件" (Codebase Analysis → CLAUDE.md)
- **Evidence**: 10+ sessions across 3 projects (hlpt/HrAppManenger ×3, hlpt/Hrapp ×3, jiazheng-new ×6). Most expensive pattern: ~470 tool calls total (Read: 226, Bash: 118, Edit: 106, Glob: 39).
- **Workflow**: Explore directory structure → read key config files → read source entry points → understand architecture → write CLAUDE.md with build commands, architecture, conventions.
- **Confidence**: HIGH — repeated 10+ times, stable inputs (any project dir), clear output (CLAUDE.md).
- **Recommended form**: **Skill** — `codebase-analysis`
- **Already covered?**: The `/init` command exists as a built-in, but users frequently invoke it with custom phrasing. The superpowers plugin does not have a dedicated codebase-analysis skill.
- **Decision**: **Create** — high repetition, high tool cost, clear procedure.

### 2. Docker MySQL 数据库备份
- **Evidence**: 3 sessions in hlpt project (2 backup + 1 compare). Clear step sequence: find container → get credentials → mysqldump → stage/commit.
- **Workflow**: `docker ps` → `docker inspect` for creds → `mysqldump` → `git add` → `git commit`
- **Confidence**: MEDIUM — repeated 3 times but only in one project (hlpt), not meatshell.
- **Recommended form**: Command — `docker-db-backup`
- **Already covered?**: No existing asset.
- **Decision**: **Skip for meatshell** — this is an hlpt-specific workflow. Not relevant to meatshell (Rust desktop app, no Docker/MySQL).

### 3. 前端 CSS/UI 微调循环
- **Evidence**: 5+ sessions in hlpt/web/Hrapp (44 Edit + 19 Read + 4 Grep tool calls in the largest session). Many user messages with precise pixel/color specifications.
- **Workflow**: User gives visual spec → read Vue component → find CSS → edit → user verifies → iterate.
- **Confidence**: LOW-MEDIUM — heavily interactive, each iteration is unique, user drives the direction.
- **Decision**: **Skip** — too interactive and context-dependent to package. The existing brainstorming skill partially covers the design discussion phase.

### 4. Feature Exploration / Design (meatshell pattern)
- **Evidence**: 1 session in meatshell ("增加显卡监控功能"). Pattern: read project structure → read existing similar feature code → understand architecture → present design with options → ask user to choose.
- **Confidence**: LOW — only 1 occurrence so far. But the pattern (explore existing code → design new feature) is likely to recur as meatshell grows.
- **Decision**: **Skip** — insufficient evidence. The brainstorming skill already covers the design discussion phase.

---

## What Was Created

**Nothing.** 

### Rationale
- The meatshell project has only 3 sessions (all from today). Two are automated (Dream/Distill), one is a feature exploration.
- No repeated manual workflows specific to meatshell exist yet.
- The strongest candidate (codebase-analysis / CLAUDE.md) is cross-project but not specific to meatshell. Since meatshell already has its codebase well-understood (single Rust + Slint app, clear structure), the need for a packaged analysis workflow is low.
- The Docker MySQL backup pattern is project-specific to hlpt and irrelevant to meatshell.
- Creating speculative assets for a project with <1 real work session would violate the "do not create speculative, overlapping, or overly broad assets" rule.

### When to re-run distill
After meatshell accumulates ~10+ work sessions (roughly 2-4 weeks of active development), re-run distill to capture project-specific patterns like:
- Adding new monitoring panels (CPU/GPU/memory pattern)
- Slint UI component workflow
- Rust build/test/fix cycles
- System sampler extension pattern

---

## Recommendations for Future Packaging

1. **If the user works on multiple projects with similar tech stacks** (Vue frontends, Spring Boot backends), consider a global `docker-db-backup` command in home config.
2. **Once meatshell has more sessions**, look for patterns around:
   - `src/system.rs` extension (adding new samplers)
   - `ui/sidebar.slint` extension (adding new StatRow widgets)
   - `src/app.rs` `refresh_sidebar()` extension
   - Cargo build + test workflows
3. **The codebase-analysis pattern** is best packaged as a global skill (in `~/.claude/`) since it applies across all projects, not just meatshell.
