# Mogan LaTeX 实现参考

调研日期：2026-09-16。依据上游 [MoganLab/mogan](https://github.com/MoganLab/mogan) 的固定提交 `fbf0c49f9690c38fb65890a446e7de9dd7b34dce`。本记录来自源码阅读，不是运行验证或任意 LaTeX 兼容性证明。

## 已观察到的机制

| 源码依据 | 观察 | Scholium 拟采用的设计 |
|---|---|---|
| [latex-format.scm](https://github.com/MoganLab/mogan/blob/fbf0c49f9690c38fb65890a446e7de9dd7b34dce/TeXmacs/plugins/latex/progs/latex/latex-format.scm) | 注册文档/片段转换，区分 texmacs-stree、latex-stree 和序列化，具有 conservative/source tracking 选项 | 分离语义投影、LaTeX 中间表示、序列化与 source provenance；原文未修改区保留 |
| [convert-latex-texout.scm](https://github.com/MoganLab/mogan/blob/fbf0c49f9690c38fb65890a446e7de9dd7b34dce/TeXmacs/plugins/latex/progs/latex/convert-latex-texout.scm) | 单独处理 documentclass、宏包、生成宏、用户导言区及正文输出 | package requirements 规划器与正文生成器分离，用户定义不可被生成内容覆盖 |
| [convert-latex-tmtex.scm](https://github.com/MoganLab/mogan/blob/fbf0c49f9690c38fb65890a446e7de9dd7b34dce/TeXmacs/plugins/latex/progs/latex/convert-latex-tmtex.scm) | 维护样式、文本/数学上下文、宏与编码选项，并加载样式适配 | Rust 显式 ConversionContext 与 capability 表；解析、受限纯宏投影及真实 TeX 构建分层 |
| [convert-latex-tmtex-ieee.scm](https://github.com/MoganLab/mogan/blob/fbf0c49f9690c38fb65890a446e7de9dd7b34dce/TeXmacs/plugins/latex/progs/latex/convert-latex-tmtex-ieee.scm) | IEEE 类别、作者/机构、摘要与关键词有专门变换 | 模板 profile 显式映射元数据和正文约束；不能只替换 documentclass 字符串 |
| [latex_tools.cpp](https://github.com/MoganLab/mogan/blob/fbf0c49f9690c38fb65890a446e7de9dd7b34dce/src/Plugins/Tex/latex_tools.cpp) | 提取/修改特定导言区与文档类信息 | 用独立 Rust CST patch 区分生成区与用户区，保留注释与原始拼写 |
| [pdflatex.scm](https://github.com/MoganLab/mogan/blob/fbf0c49f9690c38fb65890a446e7de9dd7b34dce/TeXmacs/plugins/binary/progs/binary/pdflatex.scm) | 探测不同平台 pdflatex 路径、可用性与版本 | 声明式工具链发现和能力报告；执行留在隔离 build 模块 |
| [latex_recover.cpp](https://github.com/MoganLab/mogan/blob/fbf0c49f9690c38fb65890a446e7de9dd7b34dce/src/Plugins/Tex/latex_recover.cpp) | 当前内容从成功输出日志提取页数 | 产物元数据可单独提取；文件名不能作为具有通用错误恢复能力的证据 |

## 采用边界

这些机制支持“转换分层、上下文与模板显式化”的设计判断；它们没有证明 LaTeX 与 Typst 可以任意互译，也没有证明 Tectonic、浏览器 WASM 或跨引擎宏共享可用。Scholium 仍需保留 ForeignSource、原始模板/宏及兼容报告，并实际构建最终目标。

遵循仓库 AGENT.md：只参考架构结论，不搬运 Mogan/TeXmacs 的 GPL 源码，不嵌入其 Scheme 实现。采用独立 Rust 模块及自编夹具验证，不把现有大型转换文件照搬成 Rust 大模块。

## 首批验证语料

- 普通 article：注释、未知宏、用户导言区、生成宏与宏包去重，验证未编辑原文保持。
- IEEE 风格自编样例：作者/机构、摘要、关键词、引用与双栏布局；分别验证模板字段映射和真实构建。
- 损坏输入、verbatim、catcode 变化及动态 include：保留原文并报告不确定，禁止危险重写。
- LaTeX/Typst 混用：正文、公式、图表、宏、模板及跨片段引用全部有夹具；导出两种目标包、保存重开和最终 PDF 分开验收。

最终模块边界见 [LaTeX 模块](../modules/latex.md)、[构建模块](../modules/build.md)与[全栈候选](../plan/TECH_STACK.md)。
