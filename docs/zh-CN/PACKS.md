# 社区皮肤与皮肤包

任何人都可以把皮肤免费分享给所有 FolderSkin 用户。一组分享出来的皮肤叫作**皮肤包**，只分享一款皮肤时，它就是只装了一款的皮肤包。皮肤包存放在单独的仓库 [folderskin-community](https://github.com/prajwal-svm/folderskin-community) 的 `packs/` 下，添加皮肤包不需要账号。FolderSkin 本身不带任何皮肤：所有皮肤都来自皮肤包、你自己的图片和 AI 生成的结果。

## 添加皮肤包

FolderSkin 第一次打开时，会推荐一些皮肤包，帮你把皮肤库先充实起来。之后可以在应用里打开**社区**。顶部的筛选项就是各个皮肤包的标签。**添加**会把皮肤包里的皮肤放进你的皮肤库，并带上皮肤包的标签，每款皮肤的 ⋯ 菜单里都会注明它来自哪个皮肤包、由谁分享。**移除**会把整个皮肤包再拿掉。已经用上其中某款皮肤的文件夹会保留现有的图标，因为图标就保存在文件夹本身里。

**从文件夹添加**对你电脑上的皮肤包文件夹做同样的事，分享之前也可以用它先试试你的皮肤包。

[folderskin.app](https://folderskin.app/zh-cn/community/) 的皮肤包画廊里，每个皮肤包都有一个**安装**按钮。点它会打开 FolderSkin，在**社区**中定位到这个皮肤包并添加，效果和点它的**添加**按钮一样（见下文的[安装链接](#安装链接)）。标有**官方**的皮肤包由维护者亲自把关。

皮肤包要么完整添加，要么完全不加：先下载并检查每一张图片，再一次性全部保存，所以即使断了网或者磁盘满了，你的皮肤库里也不会只留下半个皮肤包。其中的皮肤按皮肤包自身的顺序排列。

## 分享你的皮肤

直接在应用里分享即可，FolderSkin 会把皮肤包发送到它的社区服务 `community.folderskin.app`，由维护者审核。你不需要 GitHub 账号。FolderSkin 0.1.6 及更早的版本还可以替你在 GitHub 上发起 pull request，0.1.7 去掉了这个选项。

1. 给想分享的皮肤打上标签（⋯ → 标签）。分享单款皮肤，用 ⋯ → **分享到社区**。一次分享多款，用**社区 → 分享你的皮肤**，然后选一个标签。
2. 填写皮肤包的名称、标签和许可协议，说明图片的来源，并勾选确认这些图片你有权分享。
3. 第一次分享时，FolderSkin 会在浏览器里验证这台电脑，并登记你的皮肤包署名所用的名字。每台电脑只需验证一次。
4. 提交审核。在任何数据离开你的电脑之前，FolderSkin 会先按下文的格式规范检查皮肤包。**我的投稿**里会列出你提交过的每个皮肤包，如果有被拒绝的，也会说明原因。

每张图片都以无损方式分享，所以皮肤包看起来和你做的一模一样，连成品文件夹的透明边缘也不例外。发送之前，FolderSkin 会把每张图片转成无损 WebP，每张需要几秒钟，界面会实时显示已经转好了几张。如果一张图片细节太多，在 1024 px 下超过 1.5 MB，就会先缩到 896 px，还不行再缩到 768 px，依然无损，FolderSkin 也会告诉你是哪几张。一个皮肤包的图片加起来最多 64 MB，超出的会被拒绝，并建议你拆成两个皮肤包。

每个皮肤包都会先由真人审核，之后其他人才能看到。审核通过后，它会在大约 15 分钟内自动发布（见下文的[审核通过后如何发布](#审核通过后如何发布)）。多个皮肤包可以同名：大家看到的是你起的名字，而皮肤包会有一个专属的 ID（见[皮肤包 ID](#皮肤包-id)）。

皮肤包也可以手动提交：向 [folderskin-community](https://github.com/prajwal-svm/folderskin-community) 发起一个 pull request，在 `packs/` 下添加一个文件夹。用 `packs make` 生成这个文件夹（见[用图片制作皮肤包](#用图片制作皮肤包)），它会自动分配一个 ID，pull request 也会运行和应用里相同的检查。应用里的**保存为文件夹**生成的皮肤包文件夹符合下文的所有规则。

## 皮肤包 ID

每个皮肤包都有一个 ID，它就是皮肤包文件夹的名字，也是所有指向它的链接的一部分。ID 在创建皮肤包时生成一次，由名称加上六个随机字符组成：名为 Classic Art 的皮肤包，ID 可能是 `classic-art-k7q2mx`。名称可以随意重复，就算有一百个皮肤包都叫 Classic Art 也没关系，只有 ID 必须唯一。手动制作的皮肤包由 `packs make` 生成 ID，从应用分享的皮肤包则由社区服务生成。ID 一经生成就不会再变，即使皮肤包改了名也一样。

随机部分是从 `a` 到 `z`、`2` 到 `7` 中取的六个字符，来自系统的安全随机源。新 ID 绝不会与 `packs/` 中已有的文件夹同名，也不会与 `moved.json` 中的旧 ID 重复。

### moved.json

在自动生成 ID 之前做的皮肤包，ID 只由名称构成，比如 `classic-art`。这些皮肤包已经用 `packs rename` 换上了自动生成的 ID，与 `packs/` 同级的 `moved.json` 记录了每个旧 ID 以及对应皮肤包现在的 ID：

```json
{ "version": 1, "moved": { "classic-art": "classic-art-k7q2mx" } }
```

- 每个旧 ID 都不对应 `packs/` 中的任何文件夹，也永远不会分配给新的皮肤包。
- 每个新 ID 都对应 `packs/` 中的一个皮肤包，而且不会再指向另一个旧 ID：皮肤包再次改名时，所有原本指向它的记录都会改为指向它最新的 ID。
- `packs index` 和 `packs catalog` 会把这份映射以 `"moved"` 字段写入 `index.json` 和 `head.json`，这样应用、网站和社区服务都能从旧 ID 找到对应的皮肤包。从 0.1.7 开始，应用会把你以旧 ID 添加的皮肤包迁移到新 ID 上，带旧 ID 的安装链接也仍然能找到对应的皮肤包。
- 改名后的皮肤包保留首次发布的日期：`packs index` 以添加过它任一 ID 的最早那次提交作为发布日期。
- `pack.json` 里不写任何相关内容：它不接受格式规范之外的字段，多写一个字段，0.1.4 到 0.1.6 版的应用就会拒绝这个皮肤包。

### packs rename

```sh
cargo run -p folderskin-tools -- packs rename --dir ../folderskin-community --all
cargo run -p folderskin-tools -- packs rename --dir ../folderskin-community classic-art
cargo run -p folderskin-tools -- packs rename --dir ../folderskin-community classic-art --to classic-art-k7q2mx
```

它在 folderskin-community 的 git 本地仓库中为皮肤包换上自动生成的 ID。每个文件夹都用 `git mv` 移动，历史记录也随之保留。`featured.json` 和 `official.json` 会按原有顺序改写为新 ID，`moved.json` 会记录每一次迁移（文件不存在时会自动创建）。所有改动都会暂存好，可以直接提交。

`--all` 会重命名所有 ID 不是自动生成的皮肤包，其余的保持不变。它只看 ID 的格式：如果旧 ID 的最后一个词恰好是六个字母，比如 `data-structures-in-bricks`，看起来就像是自动生成的，这种皮肤包需要按 ID 单独重命名。单独指定一个皮肤包时，无论它现在的 ID 是什么，都会换成一个新生成的 ID，或者换成 `--to` 指定的 ID（它必须是自动生成格式的 ID，而且现在和过去都没有皮肤包用过）。重复运行不会有任何改动：已经迁移过的皮肤包能在 `moved.json` 中查到，会提示它已经迁移过了。

## 审核通过后如何发布

皮肤包一经审核通过就会发布，不需要任何人手动复制文件。

1. 维护者审核通过一个皮肤包后，社区服务会为它分配 ID。如果服务配置了 GitHub 令牌（`GITHUB_DISPATCH_TOKEN`），还会立即通过 `pack-approved` repository dispatch 启动 folderskin-community 的 Packs 工作流。
2. 没有令牌时，工作流会自己找到这个皮肤包。它每 15 分钟向 `https://community.folderskin.app/v1/exports/pending` 查询有多少已通过审核的皮肤包在等待发布，这只需几秒钟，只有确实有待发布的皮肤包时才会执行后续步骤。此外，不管有没有皮肤包在等待，它每天和每周都会各运行一次，维护者也可以随时手动启动它。
3. `community pull --no-done` 把每个已通过审核的皮肤包写入 `packs/`，并将每个文件的大小和 SHA-256 与上传时服务记录的值逐一核对。服务分配的 ID 必须是自动生成的，已经存在的文件夹绝不会被覆盖，也不会被加上编号另存一份。检查之前，皮肤包里的成品文件夹会先统一成同一种形状（见[一个皮肤包，一种文件夹形状](#一个皮肤包一种文件夹形状)）。与这个形状相差太多的文件夹会保持原样，运行日志里会给出一条警告。
4. `packs check` 检查每个皮肤包，和检查 pull request 时一样。
5. 每个皮肤包都由 github-actions[bot] 提交，提交信息为 `Add the <name> pack`，并推送到 `main`。
6. 只有到这一步，`community done` 才会通知服务该皮肤包已发布。如果检查或推送失败，皮肤包会继续在服务端等待，下一次运行时再重试。
7. 同一次运行还会重建 `index.json`、预览图和 `v2/`，把 `v2/` 复制到镜像（见[下文](#镜像)），然后一并提交。用工作流自带令牌进行的推送不会触发其他工作流，所以这些步骤都在同一次运行里完成。

因此，皮肤包会在审核通过后大约 15 分钟内发布，如果由服务直接启动工作流，只需几分钟。运行失败时不会发布任何内容，GitHub 会发邮件告诉维护者工作流失败了。GitHub 繁忙时，定时运行可能会晚一些才启动，而且仓库连续 60 天没有活动时，GitHub 会关掉定时任务。dispatch 方式则不受这两点影响。

工作流需要维护者的签名密钥，也就是 `community keygen` 生成的完整文件，保存为仓库 secret `FOLDERSKIN_ADMIN_KEY`。没有它，已通过审核的皮肤包会一直在服务端等待，运行日志也会说明这一点。把仓库变量 `REQUIRE_GENERATED_IDS` 设为 `true` 后，每次检查都会拒绝 ID 不是自动生成的皮肤包。

手动拉取仍然可行。`community pull` 写入皮肤包后会立即通知服务，因为运行它的人会自己提交写入的内容。`--no-done --pulled pulled.json` 会推迟通知，直到运行 `community done --from pulled.json` 为止，工作流就是这样运行的。重复通知服务也不会有任何问题。

### 镜像

应用可以从 `https://packs.folderskin.app` 读取整个目录树，这是一个 Cloudflare R2 存储桶，里面的 `v2/` 与仓库中的完全相同。写入存储桶的是社区服务，所以工作流不需要自己的 Cloudflare 令牌：`community mirror` 会用维护者的密钥签名，通过服务上传每个文件。

```sh
cargo run -p folderskin-tools -- community mirror --tree ../folderskin-community \
  --public https://packs.folderskin.app --api https://community.folderskin.app --key ~/folderskin-admin.key
```

它用 HEAD 请求向镜像逐一查询每个文件，镜像上已有同样长度的文件就跳过：除了 `head.json`，每个文件的名字都是其内容的哈希。其余文件通过 `PUT /v1/admin/tree/<path>` 上传，每个请求都在 `X-Content-SHA256` 中附上文件的 SHA-256，供存储桶校验。`head.json` 最后上传，而且要等其他所有文件都到位之后才上传，所以镜像绝不会指向一个它自己并没有的目录。遇到可能自行恢复的失败（没有响应、5xx 或 429）时，请求总共会尝试五次，等待时间从一秒开始逐次翻倍，`Retry-After` 只要不超过 30 秒都会遵守。工作流会在提交之前运行它，所以只有当镜像已经拥有 `head.json` 所指向的全部内容后，GitHub 上的 `head.json` 才会更新。仓库变量 `COMMUNITY_MIRROR_URL` 用来开启镜像，只有在开启期间，`head.json` 才会列出它。

## 格式规范

一个皮肤包就是一个文件夹：

```
packs/night-prints-h4x2qe/
  pack.json
  koi.webp
  fox-in-the-rain.webp
```

`pack.json`：

```json
{
  "version": 1,
  "name": "Night prints",
  "author": "your-github-name",
  "license": "CC0-1.0",
  "tags": ["woodblock", "night"],
  "skins": [
    { "file": "koi.webp", "name": "Koi over the wave", "tags": ["animals"] },
    { "file": "fox-in-the-rain.webp", "name": "Fox in the rain" }
  ]
}
```

| 字段 | 规则 |
|---|---|
| `version` | `1` |
| `name` | 1 到 40 个字符 |
| `author` | 你的 GitHub 用户名 |
| `license` | `CC0-1.0`、`CC-BY-4.0` 或 `MIT` |
| `tags` | 1 到 5 个标签。皮肤包里的每款皮肤都会带上这些标签，第一个标签就是这个皮肤包在所有人筛选项里显示的名字 |
| `skins` | 1 到 50 项 |
| `skins[].file` | 文件夹中的一张图片 |
| `skins[].name` | 1 到 60 个字符 |
| `skins[].tags` | 可选，为这款皮肤再加最多 3 个标签 |

不允许出现其他字段，所以像 `"tag"` 这样的拼写错误会让检查失败，而不是被悄悄忽略。

### 限制

| | 限制 |
|---|---|
| 每个皮肤包的皮肤数量 | 1 到 50 |
| 每张图片 | 无损：PNG 或无损 WebP。最大 1.5 MB |
| 一个皮肤包的图片总和 | 最多 64 MB |
| 图片边长 | 256 到 1024 px |
| 文件名 | 字母、数字、`.`、`-` 和 `_`，以 `.png` 或 `.webp` 结尾 |
| ID，即文件夹名 | 名称加六个随机字符，比如 `night-prints-h4x2qe`（见[皮肤包 ID](#皮肤包-id)）：由小写字母和数字组成的单词，用单个连字符连接，最多 40 个字符 |
| 标签 | 小写字母、数字、空格和连字符，最多 24 个字符 |
| `pack.json` | 最大 64 KB |

为什么是 50 款和 64 MB：皮肤包是一组有主题的皮肤，每个添加它的人都要下载全部内容。五十款皮肤审核起来依然很快，64 MB 足够放下五十张每张 1.3 MB 的图片，或者四十二张最大尺寸的图片。

FolderSkin 0.1.7 之前发布的皮肤包，要求是每张图片不超过 2 MB，格式可以是 PNG、JPEG 或任意 WebP，应用仍然可以读取它们。用 `packs make` 重新制作后，它们就会符合上面的规则。加上 `--require-lossless` 后，`packs check` 会按这些规则检查皮肤包（见[自己检查皮肤包](#自己检查皮肤包)），社区服务也只接受符合这些规则的皮肤包。

### 图片

每张图片属于以下两类之一，判断方式和拖进窗口的图片相同：

- **成品文件夹**：画在透明背景上，或者纯品红 `#FF00FF` 背景上（FolderSkin 会把品红抠掉）。它会原样成为图标。
- **其他任何图片**都会被包裹到 FolderSkin 的文件夹上。[SKINS.md](SKINS.md) 说明了文件夹会裁掉图片的哪些部分，帮你保住画面主体。

三个系统绘制的图标最大也就 1024 px，所以再大的图片也不会带来任何提升。

每张图片都是无损的，所以皮肤包看起来和制作时一模一样：渐变里没有色块，文字周围没有振铃伪影，成品文件夹的边缘也和画出来时一样干净。无损 WebP 比同样的 PNG 小约三分之一，所以应用和 `packs make` 都输出 WebP。一张细节丰富的 1024 px 图片大小在 0.6 到 1.5 MB 之间，大多在 800 KB 左右。超过 1.5 MB 的会先缩到 896 px，还不行再缩到 768 px，依然无损，而不是为了塞进限制把图片弄糊。

### 一个皮肤包，一种文件夹形状

FolderSkin 会把成品文件夹的整张图片完整放进图标里。一张一张生成的文件夹，都是紧贴着各自的边缘裁出来的，没有哪两张渲染图的比例完全一样：某个皮肤包里的文件夹，宽度是高度的 1.03 到 1.30 倍不等。在 Finder 里并排摆放时，矮胖的看起来比瘦高的小。所以同一个皮肤包里的成品文件夹会统一成同一种形状：

- **皮肤包的形状**取所有文件夹形状的中位数。测量文件夹时，只算图片中不透明度超过一半的部分，按宽 ÷ 高计算，所以柔和的边缘或淡淡的阴影不算在内。
- **每个文件夹都按这个形状精确重绘**。先裁到文件夹本身，再缩放到 962 px 宽（这正是 FolderSkin 自己的文件夹在 1024 px 模板中的宽度，见 `folderskin-tools template`），高度则由形状决定。然后把它放进一张透明的 1024 × 1024 图片，摆在模板文件夹所在的位置：左边缘对齐，站在同一条基线上。最后保存为无损 WebP。PNG 会变成同名的 `.webp`，`pack.json` 也会随之更新。
- **每个文件夹最多只调整 8%**。这个幅度谁也看不出来。需要调整更多的文件夹属于“离群项”：拉伸到那个程度，文字和人脸都会显得被压扁，所以它的形状永远不会被改动。怎么处理取决于所用的命令，最终由人来决定。
- **图案保持不变**。FolderSkin 会把它包裹到自己的文件夹上，所以它本身没有需要统一的形状。只有一个成品文件夹的皮肤包也同样不做处理。

`packs make` 会对它制作的每个皮肤包做这一步，`community pull` 会对它拉取的每个皮肤包做，`packs normalize` 则处理 `packs/` 中已有的皮肤包。重复执行不会有任何变化：已经符合皮肤包形状、位置也正确的文件夹不会被重绘，内容已经一致的文件也不会被重写。

| 命令 | 对离群项的处理 |
|---|---|
| `packs make` | 从皮肤包中剔除，并列出来。加上 `--keep-outliers` 则原样保留 |
| `packs normalize` | 列出来，保持原样。加上 `--drop-outliers` 会把它从 `pack.json` 中移除，并删除对应的图片 |
| `community pull` | 原样保留，并在输出中给出警告，Packs 工作流的日志里可以看到。别人分享的皮肤，绝不会在没有人拍板的情况下被丢弃 |

```sh
cargo run -p folderskin-tools -- packs normalize --dir ../folderskin-community
cargo run -p folderskin-tools -- packs normalize --dir ../folderskin-community classic-art-5rxas2 --tolerance 0.3
cargo run -p folderskin-tools -- packs normalize --dir ../folderskin-community dreamscapes-ppfia6 --drop-outliers
```

`packs normalize` 会处理所有皮肤包，或者只处理指定的几个。对每个皮肤包，它会报告形状和重绘了哪些文件夹，并列出每个离群项及其偏差。没有通过 `packs check` 的皮肤包会先跳过，等它通过了再处理。`--tolerance` 用来修改 8% 这个上限，适用于离群项也应该统一形状的皮肤包，比如 Classic Art 里的《蒙娜丽莎》。`--drop-outliers` 仅供维护者使用：除此之外，没有任何操作会删除皮肤。`packs check --require-one-shape` 会拒绝文件夹之间形状相差超过 1% 的皮肤包（见[自己检查皮肤包](#自己检查皮肤包)），按统一形状重绘过的文件夹则永远不会超出这个范围。

### 许可协议

分享的皮肤使用 Creative Commons 或 MIT 许可协议：

- `CC0-1.0`：任何人都可以用于任何用途。这是默认选项，因为大多数皮肤由 AI 生成，而 CC0 对它们主张的权利最少。
- `CC-BY-4.0`：任何人都可以使用，但要注明你是作者。
- `MIT`：任何人都可以使用，并且要保留你的名字。

只分享你自己做的、或者你有权分享的图片。

## 用图片制作皮肤包

`packs make` 可以把一个装满图片的文件夹（比如从图像模型保存下来的渲染图）变成一个皮肤包，放在本地 folderskin-community 仓库的 `packs/` 下，而且直接就能通过检查。下面的命令在本仓库中运行，folderskin-community 克隆在它旁边：

```sh
cargo run -p folderskin-tools -- packs make ~/Downloads/3d-renders --dir ../folderskin-community \
  --name "3D" --tags 3d,glossy --author your-github-name --preview /tmp/3d.png
```

皮肤包会得到一个专属 ID，由名称加六个随机字符组成，比如 `3d-k7q2mx`，这也是它的文件夹名。运行报告里会给出这个 ID。它绝不会与 `packs/` 中已有的文件夹重名，也不会与 `moved.json` 里的旧 ID 重复。

每张图片的分类方式和你在应用里添加图片时一样。成品文件夹（按 [PROMPTS.md](PROMPTS.md) 里聊天提示词的要求画在品红背景上，或者带真正的透明背景）会被抠出来，直接成为图标。其他图片则作为图案，包裹到 FolderSkin 的文件夹上。每张图片都会缩小到 1024 px 并保存为无损 WebP，所用的编码器和应用分享皮肤包时用的是同一个，已经内置，不用另外安装。如果仍然超过 1.5 MB，就先缩到 896 px，还不行再缩到 768 px，运行报告里会注明。用 `--max-kb` 可以把图片限制得比 1.5 MB 更小。所有图片加起来超过 64 MB 时会被拒绝，并建议你拆成两个皮肤包。libwebp 最彻底的压缩设置每张图片要花好几秒，所以会同时用上所有 CPU 核心。关于格式的更多说明，见 [SKINS.md](SKINS.md#皮肤包里的图片)。运行报告会说明每张图片被归为哪一类。皮肤以文件名命名，所以请先给文件起好名字，或者之后在 `pack.json` 里修改。`--preview` 会把每款皮肤都画成文件夹的样子，汇总在一张 PNG 里，方便你检查。

有两个或更多成品文件夹时，会统一成同一种形状（见[一个皮肤包，一种文件夹形状](#一个皮肤包一种文件夹形状)），报告里会列出重绘了哪些。与其他文件夹形状相差超过 8% 的文件夹会被剔除，报告里会写明偏差多少。如果想原样保留它，就加上 `--keep-outliers`。没有保留任何离群项的皮肤包可以通过 `packs check --require-one-shape`。

`--id` 可以用新图片重新制作一个已有的皮肤包：`--id 3d-k7q2mx` 会替换 `packs/3d-k7q2mx/` 中的所有内容，皮肤包保留原来的 ID，所以已经添加过它的人都会收到新版本的更新。新文件夹会先在别处生成并检查，通过后才会替换旧文件夹。不加 `--id` 时，`packs make` 总是创建一个新的皮肤包。

图像模型被要求画 `#FF00FF` 时，常常会画成均匀的覆盆子红或亮粉色（Grok 在制作 Classic Art 皮肤包时就是这样）。`--flat-backdrop` 可以抠掉任意颜色的纯色背景：它会测量每张图片自己的背景色，只去除与图片边缘相连的部分（所以文件夹里的红斗篷会保留下来），顺带去掉柔和的投影，并让边缘呈现画作本身的颜色，而不是一圈粉边。在纯灰或纯黑背景上，它只在背景自身的噪点范围内抠除，也绝不会向上蔓延，所以贴着文件夹边缘的深色外套或墨线不会被误认为背景。处理完后看一下 `--preview` 生成的预览图（它画在浅灰背景上，要是抠出了洞，一眼就能看出来）。没有纯色背景的图片仍然会被当作图案处理。

想看某张图片在应用里的效果，可以用 `render` 把它画成文件夹：

```sh
cargo run -p folderskin-tools -- render ../folderskin-community/packs/3d-k7q2mx/glass.webp --out /tmp/glass.png --size 512
```

## 自己检查皮肤包

在本仓库中运行，folderskin-community 克隆在它旁边：

```
cargo run -p folderskin-tools -- packs check --dir ../folderskin-community
```

它用应用本身的规则检查 `packs/` 中的每个文件夹，并用一句话描述每个问题。如果在 folderskin-community 的本地仓库里运行，可以省略 `--dir`：工具默认在当前文件夹中查找。它按 0.1.7 之前发布的皮肤包所遵循的标准，限制每张图片不超过 2 MB，每个皮肤包的图片加起来不超过 64 MB。`--require-lossless` 会按新皮肤包的规则检查每张图片：PNG 或无损 WebP，最大 1.5 MB，这也正是社区服务接受、`packs make` 输出的格式。在旧皮肤包重新制作完成之前，这个选项默认关闭，需要时再开启。`--max-kb` 可以把图片限制得更小。

它还会检查 ID 之间有没有冲突：两个文件夹的名字如果只有大小写不同，在 macOS 和 Windows 上就是同一个文件夹。皮肤包也不能使用 `moved.json` 中的旧 ID，而 `moved.json` 本身也要符合它自己的规则（见 [moved.json](#movedjson)）。名称不参与比较，因为名称可以重复。`--require-generated-ids` 会拒绝 ID 不是自动生成的皮肤包。它默认关闭，当 folderskin-community 的仓库变量 `REQUIRE_GENERATED_IDS` 为 `true` 时，工作流会开启它。

`--require-one-shape` 会拒绝成品文件夹形状不统一的皮肤包，也就是其中有两个文件夹的宽 ÷ 高相差超过 1%。按统一形状重绘过的文件夹总在这个范围内，所以它能揪出从来没统一过形状的皮肤包，以及有人特意保留下来的离群项。报错信息会指出相差最远的两个文件夹，并告诉你该运行什么命令（见[一个皮肤包，一种文件夹形状](#一个皮肤包一种文件夹形状)）。它默认关闭，是为 folderskin-community 的工作流准备的。

## 应用如何读取皮肤包

**社区**有**列表**和**画廊**两种视图，点任意皮肤包上的**查看**就能打开它：在添加之前，每款皮肤都会以文件夹的样子连同名称展示出来。**刷新**会重新读取列表。如果你添加过的皮肤包后来有了变化，会显示**更新**，点它会把其中的皮肤换成新版本。文件夹会保留现有的图标，新旧两个版本共有的图片如果被你收藏过，也会继续留在收藏里。

- folderskin-community 中的 `index.json` 列出了每个皮肤包：ID、名称、作者、许可协议、标签、皮肤数量，以及其完整内容（`pack.json` 和每张图片）的哈希。应用会把哈希和添加的皮肤一起保存，以此判断皮肤包有没有更新。每一项还记录了皮肤包的首次发布时间（`"added"`，单位为 Unix 秒，也就是用最初的 ID 添加它的 `pack.json` 的那次提交的时间），对于 `official.json` 里列出的皮肤包，还有 `"official": true`。与皮肤包列表并列的 `"moved"` 就是 [moved.json](#movedjson)。`index.json` 由 `folderskin-tools packs index` 生成，同时生成的还有 `previews/<id>.png`，也就是把皮肤包的前四款皮肤画成文件夹排成一排的预览图。两者都在 folderskin-community 的 `main` 分支上自动生成，切勿手动编辑。
- `v2/` 由 `packs catalog` 生成，内容还是这些皮肤包，只是组织成了应用可以在你电脑上搜索的目录。其中的 `head.json` 指明当前的目录，列出 `featured` 和 `official` 皮肤包，带有 `moved`，还列出提供同一目录树的镜像，比如 `https://packs.folderskin.app`（见[镜像](#镜像)）。应用会先从镜像获取每个文件，镜像失败时再从 GitHub 获取，无论来源如何，都会用哈希校验每个文件。从 0.1.7 开始，`head.json` 本身也会优先从 `https://packs.folderskin.app` 读取。
- 只有在你添加皮肤包时，应用才会下载其中的图片，每次同时下载四张，并显示已经下载了多少张。它会按上述限制检查每张图片，只要有一张不通过就什么也不保存。全部通过后再一起保存，所以皮肤包永远不会只添加一半。
- `FOLDERSKIN_COMMUNITY_URL` 可以让应用改用另一份 folderskin-community。例如，在本地仓库的根目录用 `python3 -m http.server` 启动服务，再把它设为 `http://localhost:8000`，就能端到端地完整试用一个皮肤包。

`index.json` 和 `head.json` 会随着时间增加字段，每个版本的应用都只读取自己认识的字段，其余的直接跳过。`pack.json` 正好相反：它不接受格式规范之外的任何字段，所以永远不能往里加新东西。皮肤包的任何新信息都放在索引里。

## 精选与官方皮肤包

folderskin-community 的根目录下，与 `packs/` 并列放着两份列表，只有维护者可以编辑。每份都是一个由皮肤包 ID 组成的 JSON 列表，比如 `["classic-art-k7q2mx", "colours-a2b3c4"]`：

| 文件 | 作用 |
|---|---|
| `featured.json` | 首次启动时推荐、在**社区**中排在最前面的皮肤包，按此顺序排列 |
| `official.json` | 在**社区**、皮肤包查看页和网站上标为**官方**的皮肤包 |

两份列表都是可选的。每个 ID 都必须对应 `packs/` 中的一个皮肤包，重复列出的只算一次。只要任一列表指向了不存在的皮肤包，`packs index` 和 `packs catalog` 就会停下来，什么也不写，所以改名或删除皮肤包不会留下空缺。pull request 的检查会运行 `packs catalog`，在合并之前就能发现问题。`packs rename` 会自动改写这两份列表。folderskin-community 的 Packs 工作流会在其 `paths` 中的文件变化时重建索引，所以这两份列表和 `moved.json` 都应该和 `packs/**` 一起写在 `paths` 里。

## 安装链接

`folderskin://install?pack=<id>` 会打开 FolderSkin，在**社区**中定位到皮肤包 `<id>` 并添加它，和点它的**添加**按钮完全一样，进度显示和最后的提示也相同。窗口会先切换到最前面。如果重新向 GitHub 查询后，**社区**里仍然没有这个 ID 的皮肤包，FolderSkin 会告诉你，并建议你搜索一下。如果皮肤包已经在你的皮肤库里，就直接打开它，并提示已经添加过了。从 0.1.7 开始，带旧 ID 的链接会打开它迁移后的皮肤包（见 [moved.json](#movedjson)）。

只有完全符合下列格式的链接，FolderSkin 才会接受：scheme 是 `folderskin`，`install` 作为主机名（`folderskin://install?…`）或整个路径（`folderskin:install?…`），不带用户名、密码或端口，并且只有一个 `pack` 参数，它的值必须是皮肤包 ID（由小写字母和数字组成的单词，用单个连字符连接，最多 40 个字符）。其他参数会被跳过，其他任何形式的链接都会被忽略。

这个 scheme 由安装程序注册：macOS 应用的 `Info.plist`、Windows 安装程序，以及 Linux `.deb` 和 `.rpm` 的桌面条目。AppImage 没有安装步骤，所以会在启动时自行注册。在 Windows 和 Linux 上，点击链接会启动第二个 FolderSkin，它把链接交给已经在运行的那个后就退出，所以始终只有一个在运行。

试一试：

| | |
|---|---|
| macOS | 构建应用（`pnpm tauri build --bundles app`），然后打开一次 `target/release/bundle/macos/FolderSkin.app`，让 macOS 注册这个 scheme（最稳妥的做法是在 `/Applications` 里放一份），再运行 `open 'folderskin://install?pack=classic-art'`。macOS 只会把链接发给打包好的应用，所以 `pnpm tauri dev` 永远收不到链接 |
| Windows | 安装一个构建版本，或者运行 `pnpm tauri dev`（开发版本会为自己注册这个 scheme），然后在命令提示符中运行 `start "" "folderskin://install?pack=classic-art"`，或者在“运行”对话框（Windows+R）中输入同样的链接 |
| Linux | 安装 `.deb` 或 `.rpm`，或者启动一次 AppImage，或者运行 `pnpm tauri dev`，然后运行 `xdg-open 'folderskin://install?pack=classic-art'` |
| 浏览器预览 | 运行 `pnpm dev`，然后打开 `http://localhost:14200/?install=classic-art` |

在 Windows 或 Linux 上运行 `pnpm tauri dev` 之前，请先退出已安装的 FolderSkin：如果已经有一个在运行，新启动的会把链接交给它，然后自己退出。开发版本注册的 scheme 会一直保留，直到安装程序或另一个构建版本重新注册。

## 安装次数统计

从**社区**添加一个皮肤包后，FolderSkin 会把这个皮肤包的 ID 告诉社区服务：`POST https://community.folderskin.app/v1/packs/<id>/installs`，不带请求体。它发送的仅此而已：没有账号，没有设备 ID，也没有关于你的皮肤库或文件夹的任何信息（和 FolderSkin 发出的所有请求一样，User-Agent 里会写明应用的版本）。这个请求在皮肤包保存之后才发出，五秒没有响应就放弃，不会让任何操作等待它，失败了也不会报错。开发版本，以及从另一份副本读取皮肤包的版本（`FOLDERSKIN_COMMUNITY_URL`），都不会发送任何内容，除非 `FOLDERSKIN_COMMUNITY_API` 指定了要发送到的服务。

对每个网络和每个皮肤包，服务每天只计一次添加，而且只统计已发布的 `index.json` 中的皮肤包。以旧 ID 添加的会计入它迁移后的皮肤包，所以 0.1.7 之前的应用也照样计数。服务为每个皮肤包保存一个计数，并在当天（UTC）剩下的时间里保存请求来源网络的加盐哈希，这样同一天重复添加同一个皮肤包不会被计两次。每天的清理任务会删除这些哈希。不保存任何地址。

folderskin.app 从 `GET https://community.folderskin.app/v1/packs/installs` 读取计数：`{"version": 1, "installs": {"classic-art-k7q2mx": 42}}`，缓存五分钟。详见 [services/community/README.md](../../services/community/README.md)。
