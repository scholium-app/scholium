# 随应用分发的字体

字体资产依据 [ADR 0011](../../../../docs/adr/ADR-0011-font-asset-licenses.md)，全部保留原始字体字节、名称、版权与许可；未修改、未子集化。
仓库仅提交五个 `.br` 无损容器，不保存解压后的 `.otf` / `.ttf` 文件。
运行时不下载字体，首次离线启动即可使用。

## Noto Sans CJK SC Regular

- 字体：原始 OTF，Noto Sans CJK SC Regular。
- 来源：https://github.com/notofonts/noto-cjk/tree/main/Sans/OTF/SimplifiedChinese
- 下载：https://raw.githubusercontent.com/notofonts/noto-cjk/main/Sans/OTF/SimplifiedChinese/NotoSansCJKsc-Regular.otf
- SHA-256：`2c76254f6fc379fddfce0a7e84fb5385bb135d3e399294f6eeb6680d0365b74b`。
- 许可：SIL Open Font License 1.1，见 [OFL.txt](OFL.txt)，版权信息保留在字体文件中。

## Liberation Serif 2.1.5

- 官方发行：https://github.com/liberationfonts/liberation-fonts/releases/tag/2.1.5
- 官方发行说明中的 TTF 包：https://github.com/liberationfonts/liberation-fonts/files/7261482/liberation-fonts-ttf-2.1.5.tar.gz
- 包 SHA-256：`7191c669bf38899f73a2094ed00f7b800553364f90e2637010a69c0e268f25d0`。
- 以下四个 TTF 字节从上述发行包原样取得，再封装为对应的 `.ttf.br`。与 Times New Roman 度量兼容，见官方项目说明
  https://github.com/liberationfonts/liberation-fonts 。
- 许可：SIL Open Font License 1.1；完整原始许可与版权见 [OFL-Liberation.txt](OFL-Liberation.txt)。

| 文件 | 原始文件 SHA-256 |
| --- | --- |
| LiberationSerif-Regular.ttf | `058ea80864aef09a23f45cbec2bb5400bc3dfbdea01c3f10538a21fcb497fb74` |
| LiberationSerif-Italic.ttf | `0e3dea9f8d613e006ccfa62201f33e265d19167bd0907725c3e145368b04fc2e` |
| LiberationSerif-Bold.ttf | `d754ba427cfe0bca54ae052384baa8f842da5bd6550ad4da024ac441e7a7d5ce` |
| LiberationSerif-BoldItalic.ttf | `f17db8af71e24d2066b587546021d4f0b296be389512b658dec3c09affeb11a7` |

## 加载与无损压缩

Typst 正文依次请求 Times New Roman、Liberation Serif、SimSun、Noto Sans CJK SC；标题将 SimSun 换成 SimHei。
System 与 Bundled 都加载这些打包字体及 Typst embedded；System 额外扫描系统字体，Bundled 完全不扫描。
拉丁字体优先使用已安装的 Times New Roman，否则 Liberation Serif；中文使用已安装的宋体/黑体，否则 Noto。
界面字体选择独立，Noto 仍只作界面的最后回退。

五个原始字体共 17,953,440 bytes；Brotli-11 压缩为 11,833,855 bytes，节省 6,119,585 bytes。
`.br` 是原始字体字节的无损容器，不是字体修改版。构建仅嵌入 `.br`，纯 Rust `brotli-decompressor` 在首次加载时
解压一次并缓存，界面与 Typst 共用 Noto 字节。单元测试核验每个解压结果的上述 SHA-256。
再生成命令（先按上述来源在临时目录取得五个原始字体，再在该目录执行；
仅将生成的 `.br` 复制回本目录。需本地 Brotli CLI，常规构建与运行不依赖该工具）：

```sh
for font in NotoSansCJKsc-Regular.otf LiberationSerif-*.ttf; do
    brotli -q 11 -f -o "$font.br" "$font"
done
```

分发二进制时须同时保留本说明、OFL.txt、OFL-Liberation.txt 与字体中的版权信息。
