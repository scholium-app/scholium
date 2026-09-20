# 原生依赖登记

`AGENT.md` 硬性约束与阶段 0 出口条件要求：**C/C++/Zig 依赖必须显式声明 ABI、内存所有权、
线程约束和销毁顺序，并有构建记录；许可证必须落入允许清单**（另见 `docs/SECURITY.md` §6 供应链）。
本页是这些依赖的唯一登记处，新增即须补录。

阶段 0 的结论：**没有自研 C/C++/Zig 源码**；需要编译本地代码的依赖只有一个（`psm`），
另有若干**以独立子进程调用、不构成链接**的外部工具链。

## 1. 链接进产物的本地代码

| 依赖 | 版本 | 许可 | 用途 | 引入路径 |
|---|---|---|---|---|
| [psm](https://github.com/rust-lang/stacker/) | 0.1.32 | MIT OR Apache-2.0 | 可移植栈操作：为深递归提供独立栈（避免解析深文档时栈溢出） | `typst` → `typst-eval` → `stacker` 0.1.25 → `psm` |

**ABI**：`psm` 对外的 ABI 只有 Rust 侧的 `psm::on_stack` / `Stack`；其内部按平台编译一小段
架构相关汇编（构建期经 `cc` 编译，例如产出 `…-x86_64.o`）。**没有任何本项目类型跨过该边界**，
也没有 C 头文件暴露给本项目。

**内存所有权**：栈内存由 `psm` 自己分配与释放；不接收调用方的缓冲区，也不返回需调用方释放的指针。
跨栈传递的是 Rust 闭包的返回值，遵循正常的 Rust 所有权规则。

**线程约束**：提供的是线程局部的栈切换能力，不引入跨线程共享状态；本项目不跨线程复用其栈。

**销毁顺序**：没有全局析构或需要显式关闭的资源；栈不可用时返回 `None`，不做部分初始化。

**构建记录**：构建脚本在编译期调用 C 编译器；实测产物
`spikes/typst-mapping/target/release/build/psm-*/out/*-x86_64.o` 存在。
版本由各 spike 的 `Cargo.lock` 锁定（`psm 0.1.32`）。

**其它平台绑定**：`windows-sys`、`js-sys` 会出现在 `Cargo.lock` 里，但它们只用于非 Linux 目标，
在本项目当前平台上不编译、不链接。

## 2. 以独立子进程调用的外部工具链（不构成链接）

### Linux 编译会话的系统 ABI

原生 UI spike 直接使用已锁定的 `libc 0.2.189`（MIT OR Apache-2.0）调用 Linux
`prctl(PR_SET_PDEATHSIG)` 与 `getppid`，保证 UI 被终止时编译会话监督进程也被终止。
它是 Rust 系统绑定，无新增自研 C 源码；调用系统动态 libc，不静态打包 libc。
ABI 只传递整数，无指针或跨边界内存所有权。调用限定在 `Command::pre_exec` 的子进程，
不申请内存、不访问共享 Rust 状态；正常关闭先终止/回收监督进程，再删除会话临时目录。
该路径仅适用于当前 Linux spike。

按 `AGENT.md` 的许可证政策，"以独立子进程调用的外部工具链不构成链接，不改变本项目许可；
但若在安装包中分发其二进制，必须单独履行对应 GPL 义务并登记"。

| 工具 | 版本 | 许可 | 调用方式 | 分发义务 |
|---|---|---|---|---|
| TeX Live（`xelatex` / `pdflatex` / `latexmk`） | 2026（`texlive-bin 2026.0-2`、`texlive-core 2026.1-1`） | GPL-2.0-or-later / GPL（latexmk 亦为 GPL 系） | **独立子进程**，本阶段用于 LaTeX 构建与验证 | 若随安装包分发二进制，须单独履行 GPL 义务并在此登记；当前仅依赖用户系统安装 |
| [bubblewrap](https://github.com/containers/bubblewrap)（`bwrap`） | 0.12.0-1 | LGPL-2.1-or-later | **独立子进程**，用于把不可信构建放进 OS 级沙箱 | 同上；动态链接由发行版提供 |

**安全边界（阶段 0 实测，见[报告 0011](spikes/SPK-0011-untrusted-input.md)）**：
TeX 自身的配置**挡不住**读取项目外文件（`\openin` 在默认配置下读到 `/etc/hostname`，
`openin_any=p` 无效），因此**不得**把 TeX 直接跑在用户会话里；必须放进 OS 级沙箱。
沙箱的可用配方尚未完成（完整只读根可编译但会暴露项目外文件；受限挂载集编译失败、缺失依赖未定位）。

## 3. 当前不存在的类别

- **自研 C/C++/Zig**：无。仓库内 `spikes/native-ui/.vendor/eui-neo` 曾有一份上游 C++ 克隆，
  已删除（该候选在[报告 0003](spikes/SPK-0003-native-ui-prescreen.md) 中预筛不投入）。
- **静态链接的 GPL/LGPL**：无（Slint 已因许可证被排除，见 [ADR 0006](adr/ADR-0006-native-ui-framework.md)）。
- **FFI 到项目自有类型**：无。

## 维护

- 新增任何需要编译本地代码的依赖时，必须在本页补一行，并填写四项（ABI / 所有权 / 线程 / 销毁）
  与构建记录；CI 的 `cargo deny` 只覆盖许可证，不覆盖这四项。
- 若将来分发 TeX 或 `bwrap` 二进制，必须在上表补"分发义务"栏的具体履行方式。
