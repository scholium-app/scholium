# Spike 0011：不受信源码的隔离（出口条件）

- 结论：**部分 Pass** —— Typst 侧满足；**LaTeX 侧不满足**，且常用配置手段无效，必须靠 OS 级沙箱
- 对应验证项：阶段 0 出口条件「不可信 LaTeX/Typst 无法执行外部命令或读取项目根外文件」
- 日期：2026-09-16
- 执行者：ation_ciger
- 代码：[`spikes/untrusted-input/`](../../spikes/untrusted-input/)

## 问题与判据

要关闭的"待验证"是出口条件里那条安全要求。判据在动手前写定：

| # | 判据 |
|---|---|
| 1 | 关闭 shell escape 时，`\write18{...}` 不能产生任何副作用 |
| 2 | 开启 shell escape 时副作用**确实产生**（证明判据 1 的测试有效，不是"命令本来就没跑"） |
| 3 | `\input` / `\openin` 不能把项目根之外的文件读进产物 |
| 4 | 若默认配置拦不住，要**如实记录**，并说明可行的隔离手段 |
| 5 | Typst 侧任何读取项目外文件的尝试都失败，且不泄漏内容 |
| 6 | Typst 侧加载本地插件（可执行本地代码）必须被拒绝 |

## 环境

- TeX Live 2026（`xelatex` / `pdflatex`），`openin_any` 默认值 `a`（`kpsewhich --var-value=openin_any`）
- `typst = 0.15.1`，自写最小 `World`（除主文件外**不做任何磁盘访问**）
- `bwrap`（bubblewrap）可用于 OS 级沙箱
- 项目外样本文件：`/etc/hostname`

## 方法与夹具

- **shell escape**：`\immediate\write18{touch /tmp/scholium-spike-pwned}`，分别以
  `-no-shell-escape` 与 `-shell-escape` 编译，检查副作用文件是否出现。后者是**基准夹具**，
  用来证明测试本身有效。
- **读项目外文件**：用 `\openin` + `\read` 把 `/etc/hostname` 读进 `\line`，再 `\message` 到日志。
  之所以不用 `\input`：`\input` 会给文件名补 `.tex`，不能直接读任意文件。
- **OS 沙箱**：`bwrap` 只读挂载运行时与 TeX 资源、项目目录可写、其余不可见，再跑同一份恶意文档。

## 结果

```text
=== LaTeX 侧 ===
  [PASS] 关闭 shell escape：副作用未产生（编译成功）
  [基准] 开启 shell escape：副作用确实产生（编译成功）
  [FAIL(默认配置)] 默认读取策略：读到项目外内容=true（"myarch"）
        SCHOLIUM-LEAK:[myarch ] [1] (./doc.aux) )
  [FAIL(该配置无效)] 限制读取（openin_any=p）：读到项目外内容=true

=== Typst 侧 ===
  [PASS] 读取项目外文件：被拒绝（file not found (searched at etc/hostname)）
  [PASS] 加载本地插件：被拒绝（file not found (searched at bin/sh)）

  [PASS] 沙箱内能否读到 /etc/hostname：不可见
  [PASS] 沙箱内编译恶意文档：泄漏=false
  [记录] 沙箱内良性文档编译：失败（挂载配方尚未让它产出 PDF）
```

### 关键发现

1. **shell escape 默认关闭且有效。** 关掉时 `\write18` 无副作用；显式打开时副作用确实产生
   （基准夹具），所以这条不是"命令本来就没跑"。
2. **LaTeX 能读项目根之外的文件 —— 默认配置下就成立。** `\openin` + `\read` 直接读到了
   `/etc/hostname` 的内容。
3. **`openin_any=p` 挡不住它。** 用 `-cnf-line=openin_any=p` 时 `kpsewhich --var-value=openin_any`
   确实变成 `p`，但**运行中的引擎仍然读到了文件**。也就是说该设置约束不了引擎级的 `\openin`。
   环境变量 `openin_any=p` 同样无效（另测 `pdflatex` 亦同）。
4. **Typst 侧按构造安全。** 拒绝读取项目外文件、拒绝加载本地插件；只要 `World::file`
   保持"除主文件外一律不碰磁盘"这个不变量，攻击面就是这个不变量本身。
5. **OS 级沙箱确实能阻断。** `bwrap` 里 `/etc/hostname` 不可见，同一份恶意文档不再泄漏。

## 失败与不确定性

- **LaTeX 侧出口条件未满足。** 仅靠 TeX 配置无法满足，必须把编译进程放进 OS 级沙箱。
- **本 spike 的 bwrap 配方还不完整**：恶意文档被成功阻断，但**良性文档在沙箱里编译失败**
  （`xdvipdfmx` 驱动返回 256；尝试过挂 `/var/lib/texmf`、把 `TEXMFVAR`/`HOME` 指向可写 tmpfs、挂 fontconfig 缓存）。
  也就是说"沙箱能阻断"已验证，"沙箱能正常干活"**尚未验证**——这是本项最大的缺口。
- **未验证**：以其它用户身份运行、Landlock/seccomp、容器方案；TeX 的写路径限制
  （`openout_any`）；`\input` 与 `\openin` 在 kpathsea 层的差异（本次 `\input` 绝对路径测试未读到文件，
  但没有查明原因，因此没有据此下结论）；Typst 的 `sys.inputs`、包下载等其它入口。
- **未覆盖**：字体解析、图片解码等**解析器层**的攻击面（本项只测命令执行与文件读取）。

## 对设计的影响

1. **LaTeX 编译必须跑在 OS 级沙箱里**，不能依赖 TeX 自身配置。建议：
   只读挂载运行时与 TeX 资源、项目目录单独可写、无网络、独立 PID/挂载命名空间、以非特权用户运行。
   这条应写进 `SECURITY.md` 与构建模块文档，并在 CI 里用"恶意夹具必须被阻断"来守。
2. **Typst 的 `World` 实现要写成不变量**：除主文件与显式纳入的资源外，`file()` 一律返回
   `NotFound`，且绝不访问磁盘。`spikes/untrusted-input` 与 `spikes/typst-mapping` 的实现可作为参考。
3. **产物必须可审计**：编译入口永远显式带 `-no-shell-escape`（本 spike 的调用即如此），
   并且沙箱不因"已经关了 shell escape"而省略。
4. **出口条件需要改写或补充**：当前表述是"不可信 LaTeX/Typst 无法执行外部命令或读取项目根外文件"，
   实测表明对 LaTeX 而言这**不是配置问题而是隔离问题**，应明确写成"编译进程在 OS 级沙箱内运行"，
   否则这条会被误以为已经满足。

## 复现步骤

```bash
cd /home/ation_ciger/Projects/Mogan/scholium
export CARGO_HOME="$PWD/spikes/native-ui/.cargo-home"
cargo run --release --manifest-path spikes/untrusted-input/Cargo.toml
```

需要 `xelatex` 与 `bwrap` 在 PATH 中。
