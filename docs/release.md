# Chronicle 发行手册

从「本地出一个能双击的 exe」到「推 tag 自动发布 GitHub Release」的完整流程。

## 1. 环境前置

本机工具链位置（一次性配置，已就绪）：

| 工具 | 位置 | 说明 |
| --- | --- | --- |
| Rust 1.97.0 | `P:\Environment\Rust`（`cargo`、`rustup` 各自一个子目录） | `CARGO_HOME=P:\Environment\Rust\cargo`、`RUSTUP_HOME=P:\Environment\Rust\rustup`，已写入用户环境变量 |
| Node 24 | `D:\Program Files\nodejs` | 已随系统安装 |
| MSVC C++ 生成工具 | `D:\Program Files\Microsoft Visual Studio\2022\Community` | 提供 `link.exe`；Rust 编译 Windows 目标的必需项 |
| Windows SDK | `D:\Windows Kits\10`（10.0.26100.0） | 随 Visual Studio 安装 |

> **重要**：`CARGO_HOME` / `RUSTUP_HOME` 是用户级环境变量，**已经打开的终端不会自动生效**。若在新终端里发现 `cargo` 不存在，或 rustup 反复重新下载工具链，请先执行：
>
> ```powershell
> $env:CARGO_HOME='P:\Environment\Rust\cargo'
> $env:RUSTUP_HOME='P:\Environment\Rust\rustup'
> $env:PATH="P:\Environment\Rust\cargo\bin;"+$env:PATH
> ```

验证：

```powershell
cargo --version    # cargo 1.97.0
rustc --version    # rustc 1.97.0
node --version     # v24.x
npm --version
```

首次克隆仓库后安装依赖：

```powershell
npm --prefix apps/desktop ci
```

## 2. 本地出一个可测试的 exe

最快路径，只编译不打包安装器：

```powershell
npm --prefix apps/desktop run desktop:build -- --no-bundle
```

产物：`apps/desktop/src-tauri/target/release/Chronicle.exe`，直接双击即可测试。

> 测试「更改存储位置」这类功能时，注意它会在默认位置写入数据；需要在干净环境验证时，把 exe 复制到单独文件夹并放置空的 `portable.marker`，即以便携模式运行、数据落在同目录 `Chronicle-data`。

## 3. 本地完整打包

```powershell
# 1) 安装器 + 原始 exe
npm --prefix apps/desktop run desktop:build

# 2) 便携版 zip
\scripts\package-portable.ps1 `
  -ExecutablePath apps/desktop/src-tauri/target/release/Chronicle.exe `
  -OutputDirectory apps/desktop/src-tauri/target/release/bundle/portable
```

> 如提示“禁止运行脚本”，在命令前加 `powershell -NoProfile -ExecutionPolicy Bypass -File`，例如
> `powershell -NoProfile -ExecutionPolicy Bypass -File scripts\package-portable.ps1 -ExecutablePath ... -OutputDirectory ...`。

产物：

- 安装版：`apps/desktop/src-tauri/target/release/bundle/nsis/Chronicle_<版本>_x64-setup.exe`
- 便携版：`apps/desktop/src-tauri/target/release/bundle/portable/Chronicle-<版本>-windows-x64-portable.zip`

生成校验文件（格式必须是 `<小写hex>  <文件名>`，应用内更新器按此解析）：

```powershell
$release = "apps/desktop/src-tauri/target/release/bundle/release"
New-Item -ItemType Directory -Force -Path $release | Out-Null
$version = (Get-Content apps/desktop/package.json -Raw | ConvertFrom-Json).version
$assets = @(
  "apps/desktop/src-tauri/target/release/bundle/nsis/Chronicle_${version}_x64-setup.exe",
  "apps/desktop/src-tauri/target/release/bundle/portable/Chronicle-${version}-windows-x64-portable.zip"
)
Copy-Item $assets -Destination $release
Get-ChildItem $release -File |
  Where-Object { $_.Name -ne "SHA256SUMS.txt" } |
  ForEach-Object { "{0}  {1}" -f (Get-FileHash -Algorithm SHA256 $_.FullName).Hash.ToLowerInvariant(), $_.Name } |
  Set-Content -Encoding ascii -Path "$release\SHA256SUMS.txt"
Get-Content "$release\SHA256SUMS.txt"
```

## 4. 版本升级

用同步脚本一次改全 5 处版本引用，避免遗漏：

```powershell
\scripts\set-version.ps1 -Version 1.3.4          # 正式版
\scripts\set-version.ps1 -Version 1.3.4-beta     # 预发布
\scripts\set-version.ps1 -Version 1.3.4 -DryRun  # 只预览不写入
```

脚本会更新：

1. `Cargo.toml` → `[workspace.package] version`
2. `apps/desktop/package.json` → `version`（决定便携包文件名）
3. `apps/desktop/package-lock.json` → 2 处
4. `apps/desktop/src-tauri/Cargo.toml` → `version`
5. `apps/desktop/src-tauri/tauri.conf.json` → `version`（决定安装器文件名与应用版本）

**脚本不会改**（需手工更新）：

- `README.md` 的「当前正式版为 …」
- `docs/changelog.md`：在顶部新增 `## v<版本>` 小节，并写明两个下载文件名
- `Cargo.lock` / `apps/desktop/src-tauri/Cargo.lock`：由 cargo 在下次构建时自动更新，不要手工编辑

## 5. 发布前的完整验证

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml

npm --prefix apps/desktop test
npm --prefix apps/desktop run typecheck
npm --prefix apps/desktop run build
```

全部退出码为 0 才继续。

> 不对 `apps/desktop/src-tauri` 跑 `clippy -D warnings`：该 crate 存在一批**既有的** clippy 告警（如 `too_many_lines`、`filter_map_next`），项目 CI（`.github/workflows/ci.yml`）也只对它跑 `fmt` 与 `test`。若后续要收紧，应先单独清理这些历史告警。

## 6. 自动发布（推荐）

推送形如 `v<版本>` 的 tag 即触发 `.github/workflows/release.yml`：

```powershell
git add -A
git commit -m "Release v1.3.4"
git push origin main
git tag -a v1.3.4 -m "Chronicle v1.3.4"
git push origin v1.3.4
```

工作流会在 `windows-latest` 上依次执行：校验 tag 与 `package.json` 版本一致 → 前端测试与类型检查 → Rust 格式化 / clippy / 测试 → 构建安装器 → 打包便携版 → 生成 `SHA256SUMS.txt` → 创建 Release。

规则：

- tag 必须是 `v` 加 `package.json` 里的版本，否则工作流直接失败（防止资产文件名与更新器期望不一致）。
- tag 中含 `-`（如 `v1.3.4-beta`）会自动标记为 **prerelease**，不会成为 “Latest”。
- Release 说明优先取 `docs/release-notes/<tag>.md`；没有该文件时，自动从 `docs/changelog.md` 抽取对应 `## v<版本>` 小节；再没有则给出通用说明。
- 测试或 clippy 失败即中止，不会构建、不会发布。

发布后核对：

```powershell
gh release view v1.3.4 --repo ThermalEX/Chronicle --json tagName,isPrerelease,assets,url
```

期望资产恰好 3 个：安装器、便携版 zip、`SHA256SUMS.txt`。

## 7. 手动发布（备用）

工作流不可用时（例如离线或需要临时改产物）：

```powershell
gh release create v1.3.4 `
  "apps/desktop/src-tauri/target/release/bundle/nsis/Chronicle_1.3.4_x64-setup.exe" `
  "apps/desktop/src-tauri/target/release/bundle/portable/Chronicle-1.3.4-windows-x64-portable.zip" `
  "apps/desktop/src-tauri/target/release/bundle/release/SHA256SUMS.txt" `
  --title "Chronicle v1.3.4" --notes-file .\release-notes.md --verify-tag
```

## 8. 常见故障

| 现象 | 原因与处理 |
| --- | --- |
| `cargo` 不是可识别的命令 | 新终端未继承环境变量。按第 1 节的 PowerShell 片段显式导出 `CARGO_HOME` / `RUSTUP_HOME` / `PATH` |
| rustup 每次都重新下载工具链 | 同上；`RUSTUP_HOME` 未生效导致 rustup 回落到 `%USERPROFILE%\.rustup`。确认该目录是否被重新创建，并删除之 |
| `link.exe not found` | MSVC C++ 生成工具缺失或未安装 VCTools 工作负载。用 `vswhere` 确认 `Microsoft.VisualStudio.Component.VC.Tools.x86.x64` |
| NSIS 打包失败、下载超时 | 打包阶段需要联网下载 NSIS 与 WebView2 bootstrapper。检查代理后重试 |
| 应用内更新提示「更新安装包来源无效」 | Release 资产命名不符合更新器规则。必须是 `Chronicle_<版本>_x64-setup.exe`，且挂在 `github.com/ThermalEX/Chronicle/releases/download/<tag>/` 下 |
| 应用内更新校验失败 | `SHA256SUMS.txt` 格式不对。必须是 `<小写hex><两空格><文件名>`，且只有两列 |
| `target` 目录撑满 P 盘 | `cargo clean` 清理；或把 `CARGO_TARGET_DIR` 指向空间更大的盘（例如 E:） |

## 9. 关于代码签名

当前 Windows 构建**未签名**，因此首次运行可能出现 SmartScreen「Windows 已保护你的电脑」提示，需要用户点「更多信息 → 仍要运行」。这是未签名软件的正常现象，不是缺陷。

- Release 说明中必须保留「未签名」提示。
- 同时提供 `SHA256SUMS.txt`，让谨慎的用户自行核对下载完整性。
- 未来若购买代码签名证书（约 $200–400/年，且 2023 年后 CA 要求用硬件令牌或云 HSM 保管私钥），只需在工作流中增加一步签名，不影响现有结构。
