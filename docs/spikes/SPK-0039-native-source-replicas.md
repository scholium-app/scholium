# Spike 0039：源码双副本原生协作闭环

- 日期：2026-09-20；基线 `4d573c0` 加本轮实现。
- 决策：[ADR 0020](../adr/ADR-0020-native-source-replicas.md)。
- 结论：限定纯文本源码副本通过；阶段 0 不升级，见 [0012](SPK-0012-exit-criteria.md)。

## 实现与验证

两个独立 LoroDoc/peer/UndoManager 进入原生窗口；已有协调器门禁先于写入和撤销。
队列真正传递导出的二进制增量，非直接复制文本；显式延迟、逆序、重复交付。
正文只读显示副本内容，源码框独立保留未提交草稿。模式与 SDG 编辑器、会话文件隔离。

- 6 项模型回归：三更新乱序重复收敛、两次本地撤销保留远端、脏草稿不覆盖远端、
  排空/IME/epoch 屏障、非 BMP Unicode 下标和语法拒绝、同 epoch 再生成保留 undo、离线拒绝。
- 候选默认 **53 passed / 11 ignored**：[tests.log](evidence/SPK-0039/tests.log)。
- all-target Clippy、修改文件 rustfmt、licenses/bans/sources 通过；未跑 advisories。
  [Clippy](evidence/SPK-0039/clippy.log)、[许可](evidence/SPK-0039/license.log)。
- 真实 Linux/niri 窗口：Alice 连续两次、Bob 一次输入，延迟状态不同；逆序重复交付后相同；
  Alice 两次撤销后两侧只留 REMOTE。旧草稿跨 epoch 拒绝，新 epoch 输入再次收敛。
- 真实 fcitx5/rime：Bob 组合期间请求切换，完成被拒；选词结束后完成切换，中文草稿仍在旧 epoch，
  应用被拒且两侧已接受正文不变。人工核对最终截图。
  [结果](evidence/SPK-0039/desktop/results.json)、[截图](evidence/SPK-0039/desktop/replica-collaboration.png)、
  [动作日志](evidence/SPK-0039/desktop/replica-collaboration.log)。

首次编译缺少 LoroEncodeError 转换，补类型化错误后通过；模型审查另补同 epoch 再生成不重置撤销。
新增 lock 88 个 package/version，既有版本未移除；generator 仅条件依赖，当前 Linux cargo tree 无路径，
未新增当前构建的 C/C++/Zig 代码。Loro 内部类型只在实验适配模块内使用。

## 复现与限制

```sh
CARGO_HOME="$PWD/spikes/native-ui/.cargo-home" cargo build --offline --manifest-path spikes/native-ui/candidate-egui/Cargo.toml
SCHOLIUM_SPIKE_REPLICAS=1 spikes/native-ui/candidate-egui/target/debug/scholium-spike-egui
/usr/bin/python spikes/native-ui/candidate-egui/scripts/native-edit-acceptance.py /tmp/scholium-replicas replica_collaboration
```

单窗口两副本、受信内存队列、纯文本片段，不是两进程网络服务或共享 SDG 结构编辑。
未接账号/服务端 opaque update 验证；队列限额不等于会话历史总内存上限。正式历史身份与恢复未改变。
完整读屏、大源码窗口验收和安全总资源配额仍需完成；不以此子集替代完整原生共同验收。
