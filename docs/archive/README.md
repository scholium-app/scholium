# 废弃原型档案

本目录保存正式重新立项前的背景、实验结果和阶段记录，仅供查证历史决策。

- 这些文档不定义当前产品或架构。
- 文档中的阶段编号、crate、技术栈和硬性约束均已失效。
- 可复用的是经过记录的实验现象，不是由此推导出的旧实现方案。
- 新决策若引用这里的结论，必须在当前环境重新验证并写入 ADR。

## 归档文档

- [BACKGROUND.md](BACKGROUND.md)：重新立项前的项目背景与旧方案概览。
- [P0_RESULTS.md](P0_RESULTS.md)：旧原型 P0 阶段实验记录（IME、字形 span、排版性能）。
- [P1_STATUS.md](P1_STATUS.md)：旧原型 P1 阶段状态与人工验收记录。
- [P2_STATUS.md](P2_STATUS.md)：旧原型 P2 阶段状态（数学 AST 与序列化）。

这些文件中的 crate 名（如 `scholium-shell`、`scholium-layout`、`scholium-doc`）、阶段编号和技术栈
都不属于当前模块清单，只作为历史证据阅读。
