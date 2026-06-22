# CI/CD Pipeline Enhancement Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a CI check workflow (`ci.yml`) that builds and lints on every push/PR, complementing the existing release workflow.

**Architecture:** The existing `release.yml` handles tag-based releases (5 platforms, AppImage, MSI, AUR). A new `ci.yml` will run on push to `main` and PRs — building on all 3 OS families + clippy + fmt check. This ensures broken code never reaches a release tag.

**Tech Stack:** GitHub Actions, Rust toolchain (dtolnay/rust-toolchain), Swatinem/rust-cache, same system deps as release.yml.

---

### Task 1: Create `.github/workflows/ci.yml`

**Files:**
- Create: `.github/workflows/ci.yml`

**Step 1: Write the CI workflow**

Create a workflow that:
- Triggers on `push` to `main` and `pull_request`
- Runs 3 jobs:
  1. `check` — `cargo fmt --check` + `cargo clippy -- -D warnings` (ubuntu-latest only, fast)
  2. `build` — matrix build on windows-latest, ubuntu-22.04, macos-14 (same targets as release.yml)
  3. Uses same Linux system deps as release.yml
  4. Uses cargo cache

**Step 2: Verify workflow syntax**

Run: `yamllint .github/workflows/ci.yml` or visual inspection
Expected: valid YAML, correct action references

**Step 3: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: add PR/push CI workflow with multi-platform build + clippy + fmt"
```
