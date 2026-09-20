# Spike 0029：持久编译与局部纹理更新

## 问题与实现

结论：**Pass（限定单页连续输入、局部上传、映射一致性与会话生命周期）**，完整编辑器验收不在此结论内。
平台：Linux/Wayland/niri；Typst 0.15.1、egui/eframe 0.36.2、comemo 0.5.1、libc 0.2.189，版本由 Cargo.lock 锁定。

用户体验确认：约半秒的逐请求编译和整页图片替换不适合即时编辑。
此次复用沙箱进程、Typst World、Source 与字体，改为完整场景清单加可复用图像块，
UI 保留页面纹理，只上传变化的块。状态栏固定单行，避免编译提示换行挤动画布。
边界和超时政策见 [ADR 0014](../adr/ADR-0014-resident-typst-editor.md)。

每次依然调用完整 Typst 编译 API，利用其内部缓存；每页仍栅格化后进行精确像素比较。
本轮没有实现真正的局部栅格化，不把小文档测试解释为任意文档都即时。

## 验证

- 原实现的“编辑后纹理身份不变”回归先失败（Managed(1) 变为 Managed(2)），改后通过。
- 连续编辑复用部分块；跳过中间结果后最终完整场景与新会话相同，像素和交互框均比较。
- 多页末段编辑保留前页块；缩回单页不残留多余页；空槽位透明、可命中且有光标框。
- 请求超时终止会话，后续会话可编译；未声明宿主文件拒绝；已退出进程不复用；
  强制杀死父进程后监督进程、命名空间及 helper 均停止运行。
- 独立脚本把块恢复为完整 RGBA，与原 one-shot PNG 导出逐字节比较，源码几何也完全一致。
  每轮输出目录仅保留当前场景的块，无逐键文件积累。

真实桌面[首轮](evidence/SPK-0029/desktop/results.json)八项通过，IME 因焦点离开测试窗口而停止注入，
保留失败；[定向复测](evidence/SPK-0029/recheck/results.json) IME 和连续输入通过。
通过项涵盖数学编辑、AT-SPI、拖选/剪切、Unicode、整节点选区、左右对比和末尾空叶子输入。
连续输入场景不逐字等待编译，检查过程中持续采纳中间结果、全文正确、编辑区原点不移动，
并检查单次未上传整个页面。[输入中](evidence/SPK-0029/desktop/continuous-typing-during.png)与
[输入后](evidence/SPK-0029/desktop/continuous-typing.png)截图已人工核对，无背景块、整页位移或旧字残留。
窄列仍可水平裁剪页面内容，这是已有固定页面宽度边界。

## 耗时口径

真实窗口标准单页夹具连续输入 19 字，后台处理 19 次，中位数 **17.924 ms**，范围
11.786–30.538 ms；首次请求 357.572 ms。每次上传 **4–5 / 70** 个块。
这些数值包含后台投影、文件交换、编译/栅格化、变化块读取及映射，不含主线程采纳和 GPU 呈现，
不能说成键入到像素出现的端到端延迟。原始日志见 `desktop/continuous-typing.log`。

[独立对比数据](evidence/SPK-0029/stream.json)：生成源码尾部另起段连续插入，产生两页，
20 次暖请求的中位往返时间 10.088 ms；首次 344.505 ms，暖请求每次改变 2–4 / 140 个块。
该脚本不含 UI 解码/上传；两页空白较多，不能与前述窗口场景直接比较。
测试过程可能与其它检查并行，全部原始样本保留，不设置固定 15/16 ms 通过线。

## 检查与复现

egui 格式、Clippy 全 targets 通过。真实沙箱运行 9 项（含一个供父进程退出测试调用的子进程夹具）；
常规回归 36 项通过、11 ignored（9 项上述沙箱测试另行运行，另两项为既有手工测试），
日志见同目录 `regression.log` 与 `sandbox-tests.log`。Typst helper release 构建通过；helper 全 targets 的严格 Clippy 仍被旧探针的
长函数、仅递归使用参数等 8 个诊断阻断（`checks.rs`、`generator.rs`、`latency.rs`、`main.rs`），
没有把这个检查声称为通过；本轮新 helper 模块没有相关诊断。

```sh
bash spikes/native-ui/candidate-egui/scripts/run-typst-editor.sh
CARGO_HOME="$PWD/spikes/native-ui/.cargo-home" cargo test --offline --manifest-path spikes/native-ui/candidate-egui/Cargo.toml typst_editor -- --ignored --nocapture
/usr/bin/python spikes/native-ui/candidate-egui/scripts/editor-stream-check.py /tmp/scholium-stream.json
SCHOLIUM_TYPST_BIN="$PWD/spikes/mixed-build/out/packages/toolchain/typst" /usr/bin/python spikes/native-ui/candidate-egui/scripts/native-edit-acceptance.py /tmp/scholium-editor-continuous continuous_typing ime
```

## 剩余范围

大文档各类局部/跨页重排、长时间编辑及 CPU 配额轮换体验、输入到 GPU 呈现分位数仍未完成。
异步等待时当前 revision 的几何尚未到达，仍不允许旧坐标点击；没有用预测字形伪装新排版。
右侧快速预览仍用原独立 CLI 流程，其计时不代表左侧编辑器。完整共同验收与阶段 0 均不关闭。
