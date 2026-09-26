<div align="center">

<img src="public/app-icon.png" alt="FolderSkin 标志" width="112" height="112" />

# FolderSkin

[English](README.md) · 简体中文 · [日本語](README.ja.md) · [한국어](README.ko.md) · [Français](README.fr.md) · [Español](README.es.md)

[![下载 macOS 版](https://img.shields.io/badge/Download_for_macOS-111827?style=for-the-badge&logo=apple&logoColor=white)](https://github.com/prajwal-svm/folderskin/releases/latest)
[![下载 Windows 版](https://img.shields.io/badge/Download_for_Windows-3A86FF?style=for-the-badge&logo=data:image/svg%2bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCA0NDggNTEyIj48cGF0aCBmaWxsPSIjZmZmIiBkPSJtMCA5My43bDE4My42LTI1LjN2MTc3LjRIMHptMCAzMjQuNmwxODMuNiAyNS4zVjI2OC40SDB6bTIwMy44IDI4TDQ0OCA0ODBWMjY4LjRIMjAzLjh6bTAtMzgwLjZ2MTgwLjFINDQ4VjMyeiIvPjwvc3ZnPg==)](https://github.com/prajwal-svm/folderskin/releases/latest)
[![下载 Linux 版](https://img.shields.io/badge/Download_for_Linux-FCC624?style=for-the-badge&logo=linux&logoColor=111827)](https://github.com/prajwal-svm/folderskin/releases/latest)

[![CI](https://github.com/prajwal-svm/folderskin/actions/workflows/ci.yml/badge.svg)](https://github.com/prajwal-svm/folderskin/actions/workflows/ci.yml) <!-- SonarQube Cloud badges, hidden until the project exists there (docs/CI.md): [![Quality gate](https://sonarcloud.io/api/project_badges/measure?project=prajwal-svm_folderskin&metric=alert_status)](https://sonarcloud.io/summary/new_code?id=prajwal-svm_folderskin) [![Coverage](https://sonarcloud.io/api/project_badges/measure?project=prajwal-svm_folderskin&metric=coverage)](https://sonarcloud.io/component_measures?id=prajwal-svm_folderskin&metric=coverage) --> [![下载量](https://img.shields.io/endpoint?url=https%3A%2F%2Fraw.githubusercontent.com%2Fprajwal-svm%2Ffolderskin%2Fbadges%2Fdownloads.json)](https://github.com/prajwal-svm/folderskin/releases) [![版本](https://img.shields.io/github/package-json/v/prajwal-svm/folderskin?label=version&color=3A86FF)](CHANGELOG.md) [![最近提交](https://img.shields.io/github/last-commit/prajwal-svm/folderskin?label=last%20commit&color=3A86FF)](https://github.com/prajwal-svm/folderskin/commits/main) [![许可协议：GPL-3.0](https://img.shields.io/badge/license-GPL--3.0-3A86FF)](LICENSE) [![基于 Tauri 2 构建](https://img.shields.io/badge/built%20with-Tauri%202-24C8DB?logo=tauri&logoColor=white)](https://tauri.app) [![欢迎贡献社区皮肤包](https://img.shields.io/badge/community%20packs-welcome-12b981)](https://github.com/prajwal-svm/folderskin-community) [![Star 数](https://img.shields.io/github/stars/prajwal-svm/folderskin?style=social)](https://github.com/prajwal-svm/folderskin)

**给任何文件夹换上皮肤。**

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/images/folder-cycle-dark.gif" />
  <img src="docs/images/folder-cycle.gif" alt="同一个文件夹依次试穿十二款皮肤：纸艺富士山、《星月夜》、波普艺术、水彩、旅行海报等" width="360" />
</picture>

你最珍贵的回忆，和那堆旧文件用着同一款平平无奇的文件夹图标。FolderSkin 给每个文件夹换上与内容相称的皮肤：夏天的照片配一帧黄金时刻的电影剧照，旅行配一张复古旅行海报，视频项目配波普艺术，生日配柔和的马卡龙色。把文件夹拖进窗口，试穿几款皮肤，再把最喜欢的那款应用上去。皮肤可以从免费的社区皮肤包里挑，也可以用你自己的照片，或者从一种颜色、一个词、一个 emoji 出发自己设计，还可以随便描述一种风格，交给 AI 来画。

<p>
  <a href="https://github.com/prajwal-svm/folderskin/releases/latest"><strong>免费下载</strong></a>
  · <a href="https://folderskin.app/zh-cn/">folderskin.app</a>
  · <a href="https://folderskin.app/zh-cn/community/">社区皮肤包</a>
  · <a href="docs/zh-CN/PACKS.md">分享皮肤包</a>
  · <a href="#从源码构建">从源码构建</a>
  · <a href="https://github.com/prajwal-svm/folderskin">⭐ 在 GitHub 上点个 Star</a>
</p>

免费 · 开源 · 无需账号 · 不追踪

</div>

<div align="center">
  <img src="docs/images/app.webp" alt="macOS 上的 FolderSkin 0.1.7：皮肤库里是 Scientists Pop Art 皮肤包，Downloads 文件夹正在试穿牛顿皮肤" width="100%" />
</div>

<details><summary>深色模式</summary>

![深色模式下的 FolderSkin](docs/images/app-dark.webp)

</details>

## 从这里开始

| 如果你想…… | 这样做 |
| --- | --- |
| 获得第一批皮肤 | 首次启动时会推荐社区的皮肤包，并已为你选好 Classic Art |
| 给文件夹换个新样子 | 把文件夹拖进窗口，点一款皮肤，再按**应用皮肤** |
| 连里面的文件夹一起换 | 打开文件夹下方的**包括子文件夹**，再按**应用到 N 个文件夹**，一次全部换上 |
| 用自己的照片 | 把图片拖进窗口，或者按**添加你的照片** |
| 自己设计 | 点**自己设计**：从一种颜色、一行文字、一个 emoji 或一张照片开始，想改什么就改什么，最后按**保存并应用** |
| 让 AI 画一款 | 点 **AI 生成**，用你自己的 API 密钥，或者点开**没有 API 密钥？**，把里面的提示词发到 Grok 或 ChatGPT 的聊天里 |
| 获取别人做的皮肤 | 打开**社区**，添加一个皮肤包 |
| 分享你的皮肤 | 皮肤的 ⋯ 菜单 → **分享到社区** |
| 再次找到某款皮肤 | 用顶部的标签、⌘F / Ctrl+F、筛选按钮（颜色、皮肤包、添加时间等），或者点星标查看**收藏** |
| 撤销 | 按**还原**，已有自定义图标的文件夹则按**移除自定义图标**，就能恢复操作系统自带的图标 |

## 哪些内容留在你的电脑上

除了少数必须联网的功能，其余一切都留在你的电脑上。没有账号，没有付费墙，也没有任何追踪：FolderSkin 唯一会上报的，是某个社区皮肤包被添加了，而且只附带这个皮肤包的 ID，好让 folderskin.app 显示每个皮肤包被添加了多少次。Mac 版下载不到 20 MB。

| 留在你的电脑上 | 需要联网 |
| --- | --- |
| 你的文件夹，以及 FolderSkin 写入的图标 | AI 请求（只在你发起时）：用你的密钥发给你选的服务商 |
| 你添加的每张图片、做的每款皮肤 | **社区**和首次启动：从 packs.folderskin.app 读取大家分享的皮肤包，连不上时改从 GitHub 读取 |
| 你的 AI 密钥（已加密） | 检查更新：每次启动时，FolderSkin 会从 GitHub 读取最新发布版本的版本文件 |
| 收藏、标签和设置 | 从**社区**添加皮肤包：只把它的 ID 发给 FolderSkin 的社区服务，别的什么都不发。该服务对同一网络每天只计一次，也不保存任何地址（[详情](docs/zh-CN/PACKS.md#安装次数统计)） |

FolderSkin 会发送哪些内容、发往哪里，以及社区服务会保存什么，[隐私政策](https://folderskin.app/zh-cn/privacy/)里都写得清清楚楚。

## 使用方法

第一次打开时，FolderSkin 会先播放一段简短的欢迎动画，然后推荐社区的皮肤包，并已为你选好 Classic Art。喜欢哪些就添加哪些，一个都不加也行：每个皮肤包要么完整添加，要么完全不加，以后随时可以在**社区**里继续充实皮肤库。欢迎动画只会出现这一次。

<details><summary>首次启动</summary>

![首次启动时推荐的社区皮肤包，已选中 Classic Art](docs/images/first-launch.png)

</details>

1. 把文件夹拖到右侧的文件夹面板上，或者点击空文件夹来选择一个。
2. 在皮肤库里点一款皮肤就能试穿，文件夹面板会立刻显示效果。侧边栏可以切换**全部皮肤**、**我的皮肤**和**收藏**，顶部的标签可以进一步筛选，⌘F / Ctrl+F 用来搜索。
3. 按下**应用皮肤**。文件夹会被标记为**已应用**，点**在 Finder 中显示**就能打开它。
4. 按**还原**可以恢复操作系统的默认图标。如果选中的文件夹原本就有自定义图标，会直接显示**移除自定义图标**。

窗口右上角的按钮可以关闭文件夹面板，把空间让给皮肤库，再按一次又会把它打开（在 Mac 上按 ⇧⌘\，在其他平台上按 Ctrl+Shift+\）。面板会保持你上次留下的开合状态，选择文件夹时它会自动打开。

想让里面的文件夹也换上同一款皮肤，就打开文件夹名称下方的**包括子文件夹**。不管有多少个，FolderSkin 都会在后台数一数（包括所有层级，但不算隐藏文件夹、应用程序包和其他磁盘），按钮随之变成**应用到 25 个文件夹**，数字以实际数量为准。点**选择**可以挑出要换的文件夹。超过十个时，开始前会先问你一声，数量很多时还会告诉你这些图标要占多少空间。换皮肤在后台进行，这期间你可以选别的文件夹，或者使用应用的其他部分。文件夹面板和侧边栏底部会显示进度，按**停止**会在处理完手头这个之后结束。最后的汇总会告诉你改了哪些、哪些没能改以及原因，并让你选择继续，或者重试那些没改成的文件夹。如果 FolderSkin 不在最前面，完成时会发通知告诉你。**全部还原**只会撤掉这次换上的图标，不多也不少。

想用自己的图片，就把它拖进窗口，或者按**添加你的照片**。图片会保存在**我的皮肤**里，直到你删除为止（删除前会先确认）。如果是画在纯品红背景上的成品文件夹（就像下文聊天提示词生成的那种），会被抠出来直接使用。其他图片则会包裹到 FolderSkin 的文件夹上。

你添加的每款皮肤都有一个 ⋯ 菜单：可以重命名（双击名称或按 F2 能直接改名），可以加标签（标签会成为顶部的筛选项），可以查看它是怎么来的（AI 模型和提示词，或者来自哪个皮肤包、由谁分享），也可以分享或删除。AI 生成的皮肤一到手就会自动打上风格标签，比如 `airbrush`。

侧边栏底部的**设置**里有主题、你的 AI 密钥、分享时自动填写的信息，以及皮肤的保存位置。把鼠标悬停在标志旁边的版本号上，可以查看“关于”信息。

## 社区皮肤

<table>
  <tr>
    <td align="center"><a href="https://folderskin.app/zh-cn/community/?pack=classic-art-5rxas2"><img src="docs/images/packs/classic-art.webp" alt="Classic Art 的四款皮肤：《蒙娜丽莎》《戴珍珠耳环的少女》《第九个浪头》和《雾海上的旅人》" width="340" /><br /><b>Classic Art</b></a></td>
    <td align="center"><a href="https://folderskin.app/zh-cn/community/?pack=scientists-pop-art-nb3dfx"><img src="docs/images/packs/scientists-pop-art.webp" alt="Scientists - Pop Art 的四款皮肤：玛丽·居里、艾达·洛夫莱斯、尼古拉·特斯拉和斯里尼瓦瑟·拉马努金的漫画风肖像" width="340" /><br /><b>Scientists - Pop Art</b></a></td>
  </tr>
  <tr>
    <td align="center"><a href="https://folderskin.app/zh-cn/community/?pack=watercolour-world-rt2klu"><img src="docs/images/packs/watercolour-world.webp" alt="Watercolour World 的四款皮肤：水彩画的京都、威尼斯、马丘比丘和马拉喀什" width="340" /><br /><b>Watercolour World</b></a></td>
    <td align="center"><a href="https://folderskin.app/zh-cn/community/?pack=countries-in-paper-lhadao"><img src="docs/images/packs/countries-in-paper.webp" alt="Countries in Paper 的四款皮肤：纸艺风格的印度、墨西哥、肯尼亚和冰岛" width="340" /><br /><b>Countries in Paper</b></a></td>
  </tr>
</table>

FolderSkin 本身不带任何皮肤。大家通过 FolderSkin 分享单款皮肤和整套皮肤包，所有人都能免费使用：首次启动时会推荐它们，平时也随时可以在**社区**里找到。添加皮肤包后，其中的皮肤会连同标签一起进入你的皮肤库。在 [folderskin.app](https://folderskin.app/zh-cn/community/) 的皮肤包画廊里，点任意皮肤包的**安装**按钮，就会打开 FolderSkin 并自动帮你添加。标有**官方**的皮肤包由维护者亲自把关。**Classic Art** 很适合作为第一个皮肤包：十六幅已进入公有领域的名画，从《蒙娜丽莎》到《星月夜》，每一幅都画在了文件夹上。想分享你的皮肤，就打开它的 ⋯ 菜单，选择**分享到社区**。想一次分享多款，就用**社区 → 分享你的皮肤**。你只需在浏览器里验证一次这台电脑，无需账号。皮肤包会先由真人审核，通过后才会出现在所有人的**社区**里。同一个对话框里的**保存为文件夹**则会把皮肤包存成文件。图片以无损方式分享，所以皮肤包看起来和你做的一模一样。[docs/zh-CN/PACKS.md](docs/zh-CN/PACKS.md) 里有格式规范和各项限制：每个皮肤包 1 到 50 款皮肤，每张图片最大 1024 px、1.5 MB，每个皮肤包最多 64 MB，许可协议可选 CC0、CC BY 4.0 或 MIT。

## 自己设计

侧边栏里的**自己设计**可以离线从零开始做一款皮肤。你可以从纯色、文字标签、emoji、双色、玻璃、条纹或带说明文字的照片开始，然后随意修改其中的一切：

- 任意颜色、任意透明度，还有渐变
- 十七种字体风格的文字，还能弯成拱形
- emoji、十三种形状，以及十种花纹，细到胶片颗粒都有
- 你自己的图片，并且可以调整效果
- 阴影、发光和贴纸描边

**文件夹骨架**开关可以切换设计是套在文件夹上显示，还是平铺显示。旁边按 Finder 实际尺寸显示的图标，能让你看清它在小尺寸下的效果。你可以做一个透明的玻璃文件夹，也可以选择**自由图标**，做一张完全不是文件夹形状的贴纸。**保存并应用**会把它用到你的文件夹上，之后在它的 ⋯ 菜单里点**编辑设计**就能重新打开。详见 [docs/zh-CN/COMPOSER.md](docs/zh-CN/COMPOSER.md)。

<details><summary>设计器</summary>

![设计一款皮肤：从图标库里选的一只虫子压印在蓝色文件夹上，旁边是图标搜索](docs/images/composer.webp)

</details>

## 用 AI 生成皮肤

FolderSkin 可以根据一段描述生成皮肤。**本地模型**直接在你自己的电脑上作画，完全免费：只需设置一次，之后不用密钥，也不会向任何地方发送数据。它支持搭载 Apple 芯片、运行 macOS 14 或更高版本的 Mac，以及 Windows 和 Linux 电脑。也可以换用**自己的 API 密钥**，用你本来就在用的服务商就行。密钥经过加密保存在你的电脑上（不会弹出钥匙串密码提示），FolderSkin 也没有自己的服务器，在你按下回车之前，什么都不会发出去。打开 **AI 生成**，描述一个场景。输入 / 可以调出三十种风格、可以作为起点的创意和你保存过的提示词，输入 @ 可以选择图片的用途：Mac 的文件夹、Windows 的文件夹，或者独立的**自由图标**。如果要做文件夹，再选择**整个文件夹**（模型以 FolderSkin 的模板为基础画出整个文件夹，像一张海报）或**只要图案**（画出平面图案，再包裹到 FolderSkin 的文件夹上）。引号里的文字会画在图上。每个结果都会保存到**我的皮肤**，马上就能试穿。

在**设置 → AI 服务商**里选择由谁来生成图片：可以在那里设置本地模型，也可以粘贴密钥。下表中每个服务商的名称都链接到创建密钥的页面。

| | 服务商 | 模型 | 每张图片 |
| :-: | --- | --- | --- |
| <picture><source media="(prefers-color-scheme: dark)" srcset="docs/images/providers/openai-dark.svg"><img src="docs/images/providers/openai.svg" width="20" height="20" alt=""></picture> | [OpenAI](https://platform.openai.com/api-keys) | GPT Image 2.5 Flare、GPT Image 2.5 Sunburst、GPT Image 2 | 约 $0.05–0.20 |
| <picture><source media="(prefers-color-scheme: dark)" srcset="docs/images/providers/xai-dark.svg"><img src="docs/images/providers/xai.svg" width="20" height="20" alt=""></picture> | [xAI Grok](https://console.x.ai) | Grok Imagine 2.0、Grok Imagine | 约 $0.02–0.04 |
| <picture><source media="(prefers-color-scheme: dark)" srcset="docs/images/providers/recraft-dark.svg"><img src="docs/images/providers/recraft.svg" width="20" height="20" alt=""></picture> | [Recraft](https://app.recraft.ai/profile/api) | Recraft V4.1、Recraft V4.1 Flash | 约 $0.007–0.035 |
| <img src="docs/images/providers/google.svg" width="20" height="20" alt=""> | [Google Gemini](https://aistudio.google.com/apikey) | Gemini 3.1 Flash Image、Gemini 3 Pro Image、Gemini 3.1 Flash Lite Image | 约 $0.034–0.14 |
| <picture><source media="(prefers-color-scheme: dark)" srcset="docs/images/providers/bfl-dark.svg"><img src="docs/images/providers/bfl.svg" width="20" height="20" alt=""></picture> | [Black Forest Labs](https://dashboard.bfl.ai) | FLUX.2 pro、FLUX.2 max、FLUX.2 flex、FLUX.2 klein 4B | 约 $0.015–0.10 |
| <img src="docs/images/providers/stability.svg" width="20" height="20" alt=""> | [Stability AI](https://platform.stability.ai/account/keys) | Stable Image Ultra、Stable Image Core | 约 $0.03–0.08 |
| <picture><source media="(prefers-color-scheme: dark)" srcset="docs/images/providers/ideogram-dark.svg"><img src="docs/images/providers/ideogram.svg" width="20" height="20" alt=""></picture> | [Ideogram](https://ideogram.ai/manage-api) | Ideogram 3.0 | 约 $0.06 |

没有密钥？[docs/zh-CN/PROMPTS.md](docs/zh-CN/PROMPTS.md) 里有一个模板和一段提示词，可以直接用在 Grok 或 ChatGPT 的聊天窗口里。在应用里点开**没有 API 密钥？**，也能看到同样的提示词，而且已经替你填好了。

[docs/zh-CN/AI.md](docs/zh-CN/AI.md) 介绍了各个服务商、密钥的存放位置、模型无法返回 Alpha 通道时如何处理透明背景，以及每条错误提示的含义。

## 制作皮肤包

一组主题图片，比如用图像模型渲染的 3D 文件夹，一条命令就能变成社区皮肤包。皮肤包放在单独的仓库 [folderskin-community](https://github.com/prajwal-svm/folderskin-community) 里，把它克隆到本仓库旁边即可。`folderskin-tools packs make` 会把成品文件夹从品红背景中抠出来（其他纯色背景，比如跑偏成粉色的品红或者纯灰色，就加上 `--flat-backdrop`），把每张图片缩小、压缩到符合要求，把成品文件夹统一成同一种形状，并写好 `pack.json`：

```sh
cargo run -p folderskin-tools -- packs make ~/Pictures/renders --dir ../folderskin-community \
  --name "3D Folders" --tags 3d,glossy --author your-github-name \
  --preview /tmp/3d-folders.png
cargo run -p folderskin-tools -- packs check --dir ../folderskin-community
```

`render` 可以预览任意图片做成文件夹后的样子，`guide` 会画出模板的安全区域，供要包裹到文件夹上的图案参考。[docs/zh-CN/PACKS.md](docs/zh-CN/PACKS.md) 介绍了格式规范和投稿流程，[docs/zh-CN/SKINS.md](docs/zh-CN/SKINS.md) 讲的是图片如何变成图标，[.claude/skills/folderskin-skins/SKILL.md](.claude/skills/folderskin-skins/SKILL.md) 则教 Claude Code 跑完整个流程，所以直接说一句“用这些渲染图做个皮肤包”也完全可行。

## 工作原理

Rust 核心（`crates/folderskin-core`）以矢量路径保存文件夹模板，把你的图片按铺满方式缩放进后面板和前面板，用 `tiny-skia` 以 2048 px 一次性渲染出整张图，再用 Lanczos3 缩小到各个图标尺寸。WebView 从不绘制文件夹的几何形状，只显示核心渲染好的 PNG，所以在每个平台上，皮肤库里的缩略图、预览和磁盘上的图标都是完全相同的像素。详见 [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)（英文）。

## 各平台说明

| 系统 | 机制 | 写入文件夹内的文件 | 注意事项 |
|---|---|---|---|
| macOS | `NSWorkspace.setIcon` | macOS 自己维护的隐藏文件 `Icon\r` | 无，Finder 会立即更新 |
| Windows | `desktop.ini` + `folderskin-<hash>.ico`，两者都设为“隐藏”和“系统”属性，文件夹标记为只读，然后对该文件夹及其父文件夹调用 `SHChangeNotify` | `desktop.ini`、`folderskin-<hash>.ico` | 无，图标一应用完，文件夹就会重绘 |
| Linux | KDE 用 `.directory`，Nautilus、Nemo 和 Caja 另外用 `gio set metadata::custom-icon` | `.directory`、`.folderskin.png` | 一些平铺式或极简的文件管理器两种都不读取 |

还原只会删除 FolderSkin 写入的内容，重复执行也不会出问题。云同步文件夹（iCloud、OneDrive、Dropbox）会把这些辅助文件同步到你的其他设备上。完整说明（包括如何手动还原）见 [docs/PLATFORMS.md](docs/PLATFORMS.md)（英文）。

## 下载

免费 · 开源 · 无需账号 · 不追踪

| 平台 | 安装包 | 下载 |
| --- | --- | --- |
| macOS 12 或更高版本 · Apple 芯片和 Intel | 通用 DMG | [![下载 macOS 版](https://img.shields.io/badge/Download_for_macOS-111827?style=for-the-badge&logo=apple&logoColor=white)](https://github.com/prajwal-svm/folderskin/releases/latest) |
| Windows 10 和 11 · x86_64 | `-setup.exe` 或 MSI | [![下载 Windows 版](https://img.shields.io/badge/Download_for_Windows-3A86FF?style=for-the-badge&logo=data:image/svg%2bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCA0NDggNTEyIj48cGF0aCBmaWxsPSIjZmZmIiBkPSJtMCA5My43bDE4My42LTI1LjN2MTc3LjRIMHptMCAzMjQuNmwxODMuNiAyNS4zVjI2OC40SDB6bTIwMy44IDI4TDQ0OCA0ODBWMjY4LjRIMjAzLjh6bTAtMzgwLjZ2MTgwLjFINDQ4VjMyeiIvPjwvc3ZnPg==)](https://github.com/prajwal-svm/folderskin/releases/latest) |
| Linux · x86_64 | AppImage、DEB 或 RPM | [![下载 Linux 版](https://img.shields.io/badge/Download_for_Linux-FCC624?style=for-the-badge&logo=linux&logoColor=111827)](https://github.com/prajwal-svm/folderskin/releases/latest) |
| Linux · ARM64 | AppImage、DEB 或 RPM | [![下载 Linux 版](https://img.shields.io/badge/Download_for_Linux-FCC624?style=for-the-badge&logo=linux&logoColor=111827)](https://github.com/prajwal-svm/folderskin/releases/latest) |

macOS 版经过了 Apple 的签名和公证，打开时和其他应用没有区别。Windows 安装程序暂未签名，所以 SmartScreen 会先弹出提示：点“更多信息”，再点“仍要运行”。Linux 安装包需要 glibc 2.35 或更高版本（Ubuntu 22.04、Debian 12、Fedora 36 及以上）。

安装之后，FolderSkin 会自动保持最新：有新版本时，它会告诉你有哪些变化，点**更新并重启**即可安装。每个更新都经过签名，应用在安装任何东西之前都会先校验签名。

### 命令行

`folderskin` 能在终端里做到应用能做的事，批量处理或处理整个磁盘时尤其方便：给文件夹换上图片再换回原样，用本地模型或你自己的密钥画文件夹图，还能制作皮肤包。在 Mac 或 Linux 上：

```sh
curl -fsSL https://folderskin.app/install-cli.sh | sh
```

在 Windows 上，打开 PowerShell 运行：

```powershell
irm https://folderskin.app/install-cli.ps1 | iex
```

两种方式都会用 SHA-256 校验下载的文件，而且不需要管理员权限。[crates/folderskin-cli/README.md](crates/folderskin-cli/README.md)（英文）里有上手示例和全部命令。

## 从源码构建

所有平台都需要：[Rust](https://rustup.rs)（首次使用时，rustup 会自动安装 `rust-toolchain.toml` 指定的版本）、Node 22 或更高版本，以及 pnpm 11（确切版本见 `package.json` 中的 `packageManager`）。

- **macOS**：Xcode 命令行工具（`xcode-select --install`）。
- **Windows**：带 C++ 工作负载的 Visual Studio Build Tools，以及 WebView2 运行时（Windows 11 已自带）。
- **Linux**：

  ```sh
  sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file \
    libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
  ```

然后：

```sh
pnpm install
pnpm tauri dev      # run it
pnpm tauri build    # installers land in target/release/bundle/
```

以下检查 CI 全都会运行，完整的任务列表见 [docs/CI.md](docs/CI.md)（英文）：

```sh
cargo test --workspace
pnpm test
pnpm typecheck
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
```

## 参与贡献

欢迎提交 bug 报告，也欢迎贡献皮肤，皮肤请通过[社区皮肤包](docs/zh-CN/PACKS.md)提交。[CONTRIBUTING.md](CONTRIBUTING.md) 介绍了协作流程和几条项目规矩，[SECURITY.md](SECURITY.md) 说明了如何私下报告安全漏洞。所有变更都记录在 [CHANGELOG.md](CHANGELOG.md) 里，[docs/RELEASING.md](docs/RELEASING.md)（英文）介绍了新版本如何构建、签名和发布。

如果 FolderSkin 让你的文件夹变好看了，欢迎在 GitHub 上[点个 Star](https://github.com/prajwal-svm/folderskin)，让更多人发现它。

## 许可协议

FolderSkin 是自由软件，采用 [GNU General Public License v3.0](LICENSE)（`GPL-3.0-only`）许可协议：你可以使用它、研究它、修改它、分享它。如果你分享修改后的版本，请以同样的许可协议公开它的源代码。0.1.6 及更早的版本以 MIT 许可协议发布，并继续沿用 MIT。

FolderSkin 的名称和标志不在许可协议的范围内，使用方式请参阅 [TRADEMARKS.md](TRADEMARKS.md)。社区皮肤包中的皮肤各有自己的许可协议，每个皮肤包里都有注明。

版权所有 © 2026 FolderSkin 贡献者。

[安全](SECURITY.md) · [参与贡献](CONTRIBUTING.md) · [GPL-3.0](LICENSE) · [商标](TRADEMARKS.md)

## Star 历史

<a href="https://www.star-history.com/#prajwal-svm/folderskin&Date">
 <picture>
   <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/svg?repos=prajwal-svm/folderskin&type=Date&theme=dark" />
   <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/svg?repos=prajwal-svm/folderskin&type=Date" />
   <img alt="Star 历史图表" src="https://api.star-history.com/svg?repos=prajwal-svm/folderskin&type=Date" />
 </picture>
</a>
