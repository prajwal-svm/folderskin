<div align="center">

<img src="public/app-icon.png" alt="FolderSkinのロゴ" width="112" height="112" />

# FolderSkin

[![macOS版をダウンロード](https://img.shields.io/badge/Download_for_macOS-111827?style=for-the-badge&logo=apple&logoColor=white)](https://github.com/prajwal-svm/folderskin/releases/latest)
[![Windows版をダウンロード](https://img.shields.io/badge/Download_for_Windows-3A86FF?style=for-the-badge&logo=data:image/svg%2bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCA0NDggNTEyIj48cGF0aCBmaWxsPSIjZmZmIiBkPSJtMCA5My43bDE4My42LTI1LjN2MTc3LjRIMHptMCAzMjQuNmwxODMuNiAyNS4zVjI2OC40SDB6bTIwMy44IDI4TDQ0OCA0ODBWMjY4LjRIMjAzLjh6bTAtMzgwLjZ2MTgwLjFINDQ4VjMyeiIvPjwvc3ZnPg==)](https://github.com/prajwal-svm/folderskin/releases/latest)
[![Linux版をダウンロード](https://img.shields.io/badge/Download_for_Linux-FCC624?style=for-the-badge&logo=linux&logoColor=111827)](https://github.com/prajwal-svm/folderskin/releases/latest)

[![CI](https://github.com/prajwal-svm/folderskin/actions/workflows/ci.yml/badge.svg)](https://github.com/prajwal-svm/folderskin/actions/workflows/ci.yml) <!-- SonarQube Cloud badges, hidden until the project exists there (docs/CI.md): [![Quality gate](https://sonarcloud.io/api/project_badges/measure?project=prajwal-svm_folderskin&metric=alert_status)](https://sonarcloud.io/summary/new_code?id=prajwal-svm_folderskin) [![Coverage](https://sonarcloud.io/api/project_badges/measure?project=prajwal-svm_folderskin&metric=coverage)](https://sonarcloud.io/component_measures?id=prajwal-svm_folderskin&metric=coverage) --> [![ダウンロード数](https://img.shields.io/endpoint?url=https%3A%2F%2Fraw.githubusercontent.com%2Fprajwal-svm%2Ffolderskin%2Fbadges%2Fdownloads.json)](https://github.com/prajwal-svm/folderskin/releases) [![バージョン](https://img.shields.io/github/package-json/v/prajwal-svm/folderskin?label=version&color=3A86FF)](CHANGELOG.md) [![最終コミット](https://img.shields.io/github/last-commit/prajwal-svm/folderskin?label=last%20commit&color=3A86FF)](https://github.com/prajwal-svm/folderskin/commits/main) [![ライセンス：GPL-3.0](https://img.shields.io/badge/license-GPL--3.0-3A86FF)](LICENSE) [![Tauri 2で開発](https://img.shields.io/badge/built%20with-Tauri%202-24C8DB?logo=tauri&logoColor=white)](https://tauri.app) [![コミュニティパック歓迎](https://img.shields.io/badge/community%20packs-welcome-12b981)](https://github.com/prajwal-svm/folderskin-community) [![スター](https://img.shields.io/github/stars/prajwal-svm/folderskin?style=social)](https://github.com/prajwal-svm/folderskin)

**どんなフォルダにもスキンを。**

最高の思い出も、昔の書類と同じ味気ないフォルダの中。FolderSkinなら、どのフォルダにも中身にぴったりのスキンを着せられます。夏の写真にはゴールデンアワーの映画のワンシーン、旅行にはヴィンテージの旅行ポスター、動画プロジェクトにはポップアート、誕生日にはやさしいパステルカラー。フォルダをウィンドウにドロップしてスキンを試着し、気に入ったものを適用するだけです。無料のコミュニティパックから選ぶのも、自分の写真を使うのも、色や言葉、絵文字から自分でデザインするのも自由です。好きなスタイルを言葉で伝えて、AIに描いてもらうこともできます。

<p>
  <a href="https://github.com/prajwal-svm/folderskin/releases/latest"><strong>無料でダウンロード</strong></a>
  · <a href="https://folderskin.app/ja/">folderskin.app</a>
  · <a href="https://folderskin.app/ja/community/">コミュニティパック</a>
  · <a href="docs/ja/PACKS.md">スキンパックを共有</a>
  · <a href="#ソースからビルド">ソースからビルド</a>
  · <a href="https://github.com/prajwal-svm/folderskin">⭐ GitHubでスターを付ける</a>
</p>

無料 · オープンソース · アカウント不要 · トラッキングなし

</div>

[English](README.md) · [简体中文](README.zh-CN.md) · 日本語 · [한국어](README.ko.md) · [Français](README.fr.md) · [Español](README.es.md)

<div align="center">
  <img src="docs/images/app.webp" alt="macOSで動くFolderSkin 0.1.7。ライブラリにScientists Pop Artパックが並び、「ダウンロード」フォルダでアイザック・ニュートンのスキンを試着しているところ" width="100%" />
</div>

<details><summary>ダークモード</summary>

![ダークモードのFolderSkin](docs/images/app-dark.webp)

</details>

## まずはここから

| やりたいこと | やり方 |
| --- | --- |
| 最初のスキンを手に入れる | 初回起動時にコミュニティのパックが表示され、Classic Artが最初から選ばれています |
| フォルダの見た目を変える | フォルダをウィンドウにドロップし、スキンをクリックして、**スキンを適用**を押します |
| 中のフォルダにもまとめて適用する | フォルダの下にある**サブフォルダも含める**をオンにしてから、**N個のフォルダに適用**を押します |
| 自分の写真を使う | 画像をウィンドウにドロップするか、**写真を追加**を押します |
| 自分でデザインする | **自分でデザイン**を開き、色、ラベル、絵文字、写真のどれかから始めて好きなように手を加え、最後に**保存して適用**を押します |
| AIに描いてもらう | **AIで生成**を開き、自分のAPIキーを使うか、**APIキーがない場合**にあるGrokやChatGPTのチャット用プロンプトを使います |
| ほかの人が作ったスキンを入手する | **コミュニティ**を開いて、パックを追加します |
| 自分のスキンを共有する | スキンの⋯メニュー → **コミュニティで共有** |
| スキンをもう一度探す | 上部のタグ、⌘F / Ctrl+F、フィルタボタン（色、パック、追加した時期など）、または**お気に入り**の星 |
| 元のアイコンに戻す | **元に戻す**を押すか、すでにカスタムアイコンが付いているフォルダなら**カスタムアイコンを削除**を押すと、OS標準のアイコンに戻ります |

## 手元のコンピュータに残るもの

インターネットが必要な一部の機能を除けば、すべてです。アカウントも有料プランもなく、あなたを追跡するものもありません。FolderSkinが外部に伝えるのは、コミュニティパックが追加されたことだけで、送るのもパックのIDだけです。folderskin.appで、各パックが追加された回数を表示するためです。Mac版のダウンロードは20MB未満です。

| コンピュータに残るもの | オンラインに送られるもの |
| --- | --- |
| フォルダと、FolderSkinが書き込むアイコン | AIへのリクエスト（実行したときだけ）。選んだプロバイダに、あなたのキーで送られます |
| 追加したすべての画像と、作ったすべてのスキン | コミュニティと初回起動。共有されたパックをGitHubから読み込みます |
| AIのキー（暗号化して保存） | アップデートの確認。起動時に、最新リリースのバージョンファイルをGitHubから読み込みます |
| お気に入り、タグ、設定 | コミュニティからのパックの追加。そのIDだけがFolderSkinのコミュニティサービスに送られます。サービスは追加をネットワークごとに1日1回だけ数え、アドレスは保存しません（[詳細](docs/ja/PACKS.md#インストール数)） |

## 使い方

初めて開くと、FolderSkinは短いウェルカム画面を表示したあと、コミュニティのスキンパックを紹介します。Classic Artは最初から選ばれています。好きなものをいくつ追加しても、ひとつも追加しなくてもかまいません。パックは丸ごと追加されるか、まったく追加されないかのどちらかで、ライブラリはあとからいつでも**コミュニティ**で増やせます。ウェルカム画面が再び表示されることはありません。

<details><summary>初回起動</summary>

![初回起動の画面。コミュニティのパックが並び、Classic Artが選ばれている](docs/images/first-launch.png)

</details>

1. フォルダを右側のフォルダパネルにドラッグするか、空のフォルダをクリックして選びます。
2. ライブラリのスキンをクリックすると試着でき、フォルダパネルにすぐ結果が表示されます。サイドバーで**すべてのスキン**、**マイスキン**、**お気に入り**を切り替え、上部のタグで絞り込み、⌘F / Ctrl+Fで検索できます。
3. **スキンを適用**を押します。フォルダに**適用済み**の印が付き、**Finderで表示**でフォルダを開けます。
4. **元に戻す**を押すと、OS標準のアイコンに戻ります。すでにカスタムアイコンが付いているフォルダを選ぶと、すぐに**カスタムアイコンを削除**が表示されます。

中のフォルダにも同じスキンを付けるには、フォルダ名の下にある**サブフォルダも含める**をオンにします。まずフォルダの数を数え（下の階層すべてが対象で、隠しフォルダとアプリのバンドルは除きます）、ボタンが**25個のフォルダに適用**のように、実際の数に合わせて変わります。10個を超えるときは、始める前に確認します。フォルダパネルには処理が済んだフォルダが順に表示され、**停止**を押すと、処理中のフォルダが終わったところで止まります。最後のまとめでは、何が変わったか、どのフォルダを変更できなかったかとその理由が表示され、続きを実行するか、失敗したフォルダをもう一度試すかを選べます。**すべて元に戻す**を押すと、その実行で付けたものだけを正確に外します。

自分の画像を使うには、ウィンドウにドロップするか、**写真を追加**を押します。画像は**マイスキン**に保存され、削除するまで残ります（削除の前には確認があります）。下で紹介するチャット用プロンプトで作るような、単色のマゼンタ背景に描かれた完成したフォルダは、切り抜かれてそのまま使われます。それ以外の画像は、FolderSkinのフォルダにはめ込まれます。

追加したスキンにはどれも⋯メニューがあります。名前の変更（名前をダブルクリックするかF2を押すと、すぐに変更できます）、タグ付け（タグは上部のフィルタになります）、作り方の確認（AIのモデルとプロンプト、またはパックと共有した人）、共有、削除ができます。AIで作ったスキンには、届いた時点で`airbrush`のようなスタイルのタグが付きます。

サイドバーの下にある**設定**では、テーマ、AIのキー、共有時に自動で入力される内容、スキンの保存場所を管理できます。ロゴの横のバージョンバッジにポインタを合わせると、アプリの情報が表示されます。

## コミュニティのスキン

FolderSkinには、最初から入っているスキンはありません。スキンやスキンのパックは、みんながFolderSkinを通じて共有していて、誰でも無料で使えます。初回起動時に紹介されるほか、**コミュニティ**からいつでも入手できます。パックを追加すると、そのスキンがタグ付きでライブラリに入ります。[folderskin.app](https://folderskin.app/ja/community/)のギャラリーでパックの**インストール**ボタンを押すと、FolderSkinが開いてパックを追加してくれます。**公式**の印が付いたパックは、メンテナのお墨付きです。最初のパックには**Classic Art**がおすすめです。モナ・リザから星月夜まで、パブリックドメインの名画16点を、それぞれフォルダに描き込んだパックです。自分のスキンを共有するには、スキンの⋯メニューから**コミュニティで共有**を選ぶか、複数ならまとめて**コミュニティ → スキンを共有**を使います。ブラウザでコンピュータの確認を一度行うだけで、アカウントは要りません。パックは人が審査したうえで、コミュニティで全員に公開されます。同じダイアログの**フォルダとして保存**を使うと、送信する代わりにパックをファイルとして書き出せます。画像はロスレスで共有されるので、パックは作ったとおりの見た目で届きます。仕様と制限は[docs/ja/PACKS.md](docs/ja/PACKS.md)にあります。1パックあたりスキン1〜50個、画像は1024px・1.5MBまで、パック全体で64MBまで、ライセンスはCC0、CC BY 4.0、MITのいずれかです。

## 自分でデザイン

サイドバーの**自分でデザイン**では、オフラインでスキンを一から作れます。単色、ラベル、絵文字、ツートーン、ガラス、ストライプ、キャプション付きの写真のどれかから始めて、あとはすべてを自由に変えられます。

- 好きな色と透明度、そしてグラデーション
- 17種類のフォントスタイルの文字（アーチ状に曲げることもできます）
- 絵文字、13種類の図形、フィルムグレインまでそろった10種類のパターン
- 自分の画像（補正もできます）
- 影、グロー、ステッカー風のふち

**フォルダの骨組み**スイッチで、デザインをフォルダに載せた状態と平面の状態を切り替えられます。横にはFinderが表示するサイズでアイコンが並ぶので、どう見えるかを確かめられます。透けるガラスのフォルダを作ることも、**自由形アイコン**を選んで、フォルダとはまったく違う形のステッカーを作ることもできます。**保存して適用**でフォルダに付き、⋯メニューの**デザインを編集**でもう一度開けます。詳しくは[docs/ja/COMPOSER.md](docs/ja/COMPOSER.md)をご覧ください。

<details><summary>デザイナー</summary>

![スキンをデザインしているところ。アイコンライブラリの虫を青いフォルダに型押しし、横にアイコン検索がある](docs/images/composer.webp)

</details>

## AIでスキンを生成

FolderSkinは、言葉の説明からスキンを作れます。**ローカルモデル**なら、あなたのコンピュータ上で無料で描けます。一度セットアップすれば、キーは不要で、どこにも何も送りません。macOS 14以降のAppleシリコン搭載Mac、WindowsとLinuxのPCで動きます。すでに使っているプロバイダの**自分のAPIキー**を使うこともできます。キーは暗号化してコンピュータに保存され（キーチェーンのパスワードを求められることもありません）、FolderSkin自体はサーバを持たず、Enterキーを押すまでは何も送信されません。**AIで生成**を開いて場面を説明し、スタイルを選んで、**フォルダ全体**（モデルがFolderSkinのテンプレートをもとに、ポスターのようにフォルダ全体を描きます）か**絵だけ**（平面の絵をFolderSkinのフォルダにはめ込みます）を選びます。結果はすべて**マイスキン**に保存され、すぐに試着できます。

画像をどこで作るかは、**設定 → AIプロバイダ**で選びます。そこでローカルモデルをセットアップするか、キーを貼り付けてください。各プロバイダの名前は、キーを作成するページにリンクしています。

| | プロバイダ | モデル | 1枚あたり |
| :-: | --- | --- | --- |
| <picture><source media="(prefers-color-scheme: dark)" srcset="docs/images/providers/openai-dark.svg"><img src="docs/images/providers/openai.svg" width="20" height="20" alt=""></picture> | [OpenAI](https://platform.openai.com/api-keys) | GPT Image 2.5 Flare, GPT Image 2.5 Sunburst, GPT Image 1 | 約$0.02〜0.19 |
| <picture><source media="(prefers-color-scheme: dark)" srcset="docs/images/providers/xai-dark.svg"><img src="docs/images/providers/xai.svg" width="20" height="20" alt=""></picture> | [xAI Grok](https://console.x.ai) | Grok Imagine | 約$0.02 |
| <picture><source media="(prefers-color-scheme: dark)" srcset="docs/images/providers/recraft-dark.svg"><img src="docs/images/providers/recraft.svg" width="20" height="20" alt=""></picture> | [Recraft](https://www.recraft.ai/profile/api) | Recraft V3 | 約$0.04 |
| <img src="docs/images/providers/google.svg" width="20" height="20" alt=""> | [Google Gemini](https://aistudio.google.com/apikey) | Gemini 2.5 Flash Image | 約$0.04 |
| <picture><source media="(prefers-color-scheme: dark)" srcset="docs/images/providers/bfl-dark.svg"><img src="docs/images/providers/bfl.svg" width="20" height="20" alt=""></picture> | [Black Forest Labs](https://dashboard.bfl.ai) | FLUX 1.1 Pro | 約$0.04 |
| <img src="docs/images/providers/stability.svg" width="20" height="20" alt=""> | [Stability AI](https://platform.stability.ai/account/keys) | Stable Image Core | 約3クレジット |
| <picture><source media="(prefers-color-scheme: dark)" srcset="docs/images/providers/ideogram-dark.svg"><img src="docs/images/providers/ideogram.svg" width="20" height="20" alt=""></picture> | [Ideogram](https://ideogram.ai/manage-api) | Ideogram v3 | 約$0.03〜0.09 |

キーがなくても大丈夫です。[docs/ja/PROMPTS.md](docs/ja/PROMPTS.md)に、GrokやChatGPTのチャットで使えるテンプレートとプロンプトがあります。アプリでも、**APIキーがない場合**を開くと、同じプロンプトが入力済みの状態で表示されます。

[docs/ja/AI.md](docs/ja/AI.md)では、プロバイダ、キーの保存場所、アルファチャンネルを返せないモデルでの透明部分の扱い、各エラーメッセージの意味を説明しています。

## パックを作る

画像生成モデルでレンダリングした3Dフォルダのような、テーマのあるセットは、コマンド1つでコミュニティパックになります。パックは専用のリポジトリ[folderskin-community](https://github.com/prajwal-svm/folderskin-community)にあるので、このリポジトリの隣にチェックアウトしてください。`folderskin-tools packs make`は、完成したフォルダをマゼンタの背景から切り抜き（ピンクにずれた背景や無地のグレーなど、ほかの単色の背景には`--flat-backdrop`を使います）、すべての画像を制限に収まるよう縮小・圧縮し、完成したフォルダの形をそろえてから、`pack.json`を書き出します。

```sh
cargo run -p folderskin-tools -- packs make ~/Pictures/renders --dir ../folderskin-community \
  --name "3D Folders" --tags 3d,glossy --author your-github-name \
  --preview /tmp/3d-folders.png
cargo run -p folderskin-tools -- packs check --dir ../folderskin-community
```

`render`は、どんな画像でも、それがどんなフォルダになるかをプレビューします。`guide`は、フォルダにはめ込むアートワーク用に、テンプレートのセーフエリアを描き出します。パックの仕様と提案の方法は[docs/ja/PACKS.md](docs/ja/PACKS.md)に、画像がアイコンになるしくみは[docs/ja/SKINS.md](docs/ja/SKINS.md)にあります。[.claude/skills/folderskin-skins/SKILL.md](.claude/skills/folderskin-skins/SKILL.md)はClaude Codeに一連の作業を教えるので、「このレンダリング画像からパックを作って」と頼むこともできます。

## しくみ

Rustのコア（`crates/folderskin-core`）は、フォルダのテンプレートをベクターパスとして持ち、画像を背面パネルと前面パネルにカバーフィットで収め、`tiny-skia`で全体を2048pxで一度だけレンダリングしてから、Lanczos3で各アイコンサイズに縮小します。WebViewがフォルダの形を描くことはなく、表示するのはコアがレンダリングしたPNGだけです。そのため、ギャラリーのサムネイル、プレビュー、ディスク上のアイコンは、どのプラットフォームでもまったく同じピクセルになります。詳しくは[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)をご覧ください。

## プラットフォームごとの注意

| OS | 仕組み | フォルダ内に書き込まれるファイル | 注意点 |
|---|---|---|---|
| macOS | `NSWorkspace.setIcon` | macOSが管理する不可視の`Icon\r`ファイル | なし。Finderにすぐ反映されます |
| Windows | `desktop.ini` + `folderskin-<hash>.ico`（どちらも隠し属性とシステム属性付き）。フォルダを読み取り専用にし、フォルダとその親に`SHChangeNotify`を送ります | `desktop.ini`、`folderskin-<hash>.ico` | なし。適用が終わると、フォルダが再描画されます |
| Linux | KDE用の`.directory`と、Nautilus、Nemo、Caja用の`gio set metadata::custom-icon` | `.directory`、`.folderskin.png` | タイル型や最小構成のファイルマネージャには、どちらも読まないものがあります |

元に戻す操作では、FolderSkinが書き込んだものだけを削除し、2回実行しても問題ありません。クラウドで同期しているフォルダ（iCloud、OneDrive、Dropbox）では、補助ファイルもほかのコンピュータに同期されます。手作業で元に戻す方法も含め、詳しくは[docs/PLATFORMS.md](docs/PLATFORMS.md)をご覧ください。

## ダウンロード

無料 · オープンソース · アカウント不要 · トラッキングなし

| プラットフォーム | パッケージ | ダウンロード |
| --- | --- | --- |
| macOS 12以降 · AppleシリコンとIntel | ユニバーサルDMG | [![macOS版をダウンロード](https://img.shields.io/badge/Download_for_macOS-111827?style=for-the-badge&logo=apple&logoColor=white)](https://github.com/prajwal-svm/folderskin/releases/latest) |
| Windows 10、11 · x86_64 | `-setup.exe`またはMSI | [![Windows版をダウンロード](https://img.shields.io/badge/Download_for_Windows-3A86FF?style=for-the-badge&logo=data:image/svg%2bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCA0NDggNTEyIj48cGF0aCBmaWxsPSIjZmZmIiBkPSJtMCA5My43bDE4My42LTI1LjN2MTc3LjRIMHptMCAzMjQuNmwxODMuNiAyNS4zVjI2OC40SDB6bTIwMy44IDI4TDQ0OCA0ODBWMjY4LjRIMjAzLjh6bTAtMzgwLjZ2MTgwLjFINDQ4VjMyeiIvPjwvc3ZnPg==)](https://github.com/prajwal-svm/folderskin/releases/latest) |
| Linux · x86_64 | AppImage、DEB、RPM | [![Linux版をダウンロード](https://img.shields.io/badge/Download_for_Linux-FCC624?style=for-the-badge&logo=linux&logoColor=111827)](https://github.com/prajwal-svm/folderskin/releases/latest) |
| Linux · ARM64 | AppImage、DEB、RPM | [![Linux版をダウンロード](https://img.shields.io/badge/Download_for_Linux-FCC624?style=for-the-badge&logo=linux&logoColor=111827)](https://github.com/prajwal-svm/folderskin/releases/latest) |

macOS版はAppleによる署名と公証を受けているので、ほかのアプリと同じように開けます。Windows版のインストーラはまだ署名されていないため、最初にSmartScreenの確認が表示されます。「詳細情報」を選んでから「実行」を押してください。Linux版のパッケージにはglibc 2.35以降が必要です（Ubuntu 22.04、Debian 12、Fedora 36以降）。

インストールしたあとは、FolderSkinが自分で最新の状態を保ちます。新しいバージョンが出ると変更点が表示され、**アップデートして再起動**でインストールできます。アップデートにはすべて署名があり、アプリは何かをインストールする前に必ず署名を確認します。

## ソースからビルド

どのプラットフォームでも必要なもの：[Rust](https://rustup.rs)（初回使用時に、`rust-toolchain.toml`で指定されたバージョンをrustupがインストールします）、Node 22以降、pnpm 11（正確なバージョンは`package.json`の`packageManager`で指定されています）。

- **macOS**：Xcodeコマンドラインツール（`xcode-select --install`）。
- **Windows**：C++ワークロードを含むVisual Studio Build Tools、およびWebView2ランタイム（Windows 11には最初から入っています）。
- **Linux**：

  ```sh
  sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file \
    libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
  ```

準備ができたら、次を実行します。

```sh
pnpm install
pnpm tauri dev      # run it
pnpm tauri build    # installers land in target/release/bundle/
```

チェックは次のとおりで、どれもCIで実行されます（全ジョブの一覧は[docs/CI.md](docs/CI.md)にあります）。

```sh
cargo test --workspace
pnpm test
pnpm typecheck
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
```

## コントリビューション

バグ報告もスキンも大歓迎です。スキンは[コミュニティパック](docs/ja/PACKS.md)として受け付けています。作業の流れといくつかのルールは[CONTRIBUTING.md](CONTRIBUTING.md)に、脆弱性を非公開で報告する方法は[SECURITY.md](SECURITY.md)にあります。変更点は[CHANGELOG.md](CHANGELOG.md)に記録していて、リリースのビルド、署名、公開の手順は[docs/RELEASING.md](docs/RELEASING.md)にまとめています。

FolderSkinでフォルダが素敵になったら、[GitHubでスター](https://github.com/prajwal-svm/folderskin)を付けてもらえると、ほかの人にも見つけてもらいやすくなります。

## ライセンス

FolderSkinは、[GNU General Public License v3.0](LICENSE)（`GPL-3.0-only`）のもとで配布されるフリーソフトウェアです。自由に使い、調べ、変更し、共有できます。変更したものを共有する場合は、そのソースコードも同じライセンスで共有してください。0.1.6までのリリースはMITライセンスで公開されており、今後もMITのままです。

FolderSkinの名前とロゴは、このライセンスの対象外です。使い方は[TRADEMARKS.md](TRADEMARKS.md)をご覧ください。コミュニティパックのスキンには、パックごとに指定された独自のライセンスがあります。

Copyright 2026 FolderSkin contributors.

[セキュリティ](SECURITY.md) · [コントリビューション](CONTRIBUTING.md) · [GPL-3.0](LICENSE) · [商標](TRADEMARKS.md)

## スター履歴

<a href="https://www.star-history.com/#prajwal-svm/folderskin&Date">
 <picture>
   <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/svg?repos=prajwal-svm/folderskin&type=Date&theme=dark" />
   <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/svg?repos=prajwal-svm/folderskin&type=Date" />
   <img alt="スター履歴のグラフ" src="https://api.star-history.com/svg?repos=prajwal-svm/folderskin&type=Date" />
 </picture>
</a>
