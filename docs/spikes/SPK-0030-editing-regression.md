# Spike 0030：编辑回归与性能验证收口

- 日期：2026-09-18
- 基线：`77795fd`，加本轮撤销快捷键和验证工具路径修复。
- 结论：**Pass（已声明编辑/预览夹具）**；完整阶段判定见 [0012](SPK-0012-exit-criteria.md)。
- 平台：Linux / Wayland / niri，真实 fcitx5/Rime、AT-SPI；egui/eframe 0.36.2，Typst 0.15.1。

## 范围与用户裁决

用户确认延迟已无明显体感，要求 spike 停止精细优化、尽快推进功能。本轮只做必要回归并修复
实际失败，不继续局部栅格化、GPU 呈现分位数或固定帧预算研究。
条件 3 按原“后台运行、编辑/导航可用、旧结果不闪回、声明规模实测”验收；这些优化不再是推进门槛。

## 桌面结果

[首轮结果](evidence/SPK-0030/desktop/results.json)全部通过：中文输入法、LaTeX/Typst 源码编辑与草稿、
数学编辑、无障碍接口、拖选/剪切/撤销、Unicode、整节点选区、多页预览、左右对比、空叶子、连续输入。
另记录窗口 CPU 样本，不作固定毫秒通过/失败判定。多页预览检查页码、缩放、段落定位及当前 revision。

新增大文档实际编辑场景：256 个扩展段落（另有标准首段），9378 个可访问字符，**26 页**。
首段输入 `LARGE`、选中删除、撤销，再在末页输入 `END`。
[首次失败](evidence/SPK-0030/large/results.json)保留，[修复后复测](evidence/SPK-0030/recheck/results.json)通过。
人工核对[末页截图](evidence/SPK-0030/recheck/large-document-edit.png)、多页预览截图与连续输入中间截图；
窄列水平裁剪仍是已知页面宽度边界，未声称完整屏幕阅读器验收。

大文档后台处理首次约 2141 ms，后续样本约 603–692 ms；编译期间语义输入继续接受，
只采纳最新图像和几何。这些数据不是单页约 18 ms 的外推，也不是 GPU 呈现计时。
按用户本轮要求记录边界，不继续据此做精细优化。

## 发现并修复的两个问题

1. **短按 Ctrl+Z 漏识别。** 大文档帧中按下与松开可能同时到达；原实现读取帧末修饰键状态，
   Ctrl 已松开时忽略 Z 的快捷键含义。新增输入路径测试先稳定复现失败，再改为读取 Z 按下事件的
   modifiers。13 项输入回归、桌面数学撤销和 26 页删除/撤销复测通过。没有通过放慢测试按键掩盖问题。
2. **验收工具被自己删除。** 将 Typst CLI 指向旧导出包时，`packages` 重建先清空输出树，导致 CLI 消失，
   连带后续右侧预览失败。验收脚本在开始时保存受信 CLI 副本，避免引用即将被重建的产物目录。
   缺失工具使用上游 Typst 0.15.1 release 恢复到临时工具目录后重测；没有添加应用内下载行为。

## 自动检查

阶段脚本首轮 **33 项通过、2 项失败**，见[原始汇总](evidence/SPK-0030/automated/first-summary.log)。
两项均涉及上述 CLI 丢失：导出包重建和真实后台预览。定向复测两项均通过，其中包重建 **28/28**；
[包重建汇总](evidence/SPK-0030/automated/packages-recheck-summary.log)与
[预览汇总](evidence/SPK-0030/automated/preview-recheck-summary.log)保留在 `automated/`，
不覆盖首轮失败，不用自动检查数量代替阶段出口判定。
核心、UI 候选构建、Typst 映射、reconcile、CRDT、恢复、语言协调、混合构建、安全和十二个 workspace
许可/来源检查均在首轮通过。离线许可检查未跑 advisories，不代表安全漏洞扫描通过。

本轮 egui 全 targets Clippy、格式和脚本语法检查通过，验证脚本自测通过。
10 万行源码 headless 测试已执行，日志在 `automated/first/18.log`；这不代表十万行真实窗口验收。
旧 helper Clippy 长函数等问题仍按报告 0029 保留，本轮未扩大到无关重构。

```sh
SCHOLIUM_TYPST_BIN=/tmp/scholium-typst-toolchain/bin/typst bash spikes/verify-stage0.sh all
SCHOLIUM_TYPST_BIN=/tmp/scholium-typst-toolchain/bin/typst /usr/bin/python spikes/native-ui/candidate-egui/scripts/native-edit-acceptance.py /tmp/scholium-edit-regression
```

## 下一步

编辑/预览性能验证按已测夹具收口，后续只有用户实际反馈或新规模需求才重新优化。
优先补阶段 0 窗口的打开、保存、重开、未提交源码草稿恢复闭环；团队窗口集成、完整无障碍、
运行时入口审查和混用保真继续按出口清单推进，不提前搭建生产脚手架。
