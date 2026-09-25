# Translating the app

FolderSkin speaks English, 简体中文 (`zh-CN`), 日本語 (`ja`), 한국어 (`ko`), Français (`fr`) and
Español (`es`). Every word the app shows is in this folder, one folder per language and one file
per namespace:

```
src/locales/
  en/        English, the source. Every other language is checked against it.
  zh-CN/  ja/  ko/  fr/  es/
    ai.json  common.json  community.json  composer.json  folder.json  library.json  menu.json
    native.json  onboarding.json  settings.json  share.json  sidebar.json  updates.json
```

A file that holds `{}` isn't translated yet: the app shows those words in English. A message a
language doesn't have is shown in English too, so the app never shows a blank or a key.

## How to translate a namespace

1. Open the English file, for example `en/sidebar.json`, and your language's file of the same
   name, for example `fr/sidebar.json`.
2. Write the same keys, nested the same way, with your language's words as the values. Keep the
   keys exactly as they are, in English. Translate a whole namespace in one go: once a file has any
   message, the check wants all of them.
3. Check it:

   ```sh
   pnpm check:locales --locale fr --strict
   ```

   It lists every problem with the file and key (`error fr sidebar.json items.skins: …`) and ends
   with "No problems." when the language is complete and correct. Without `--locale` it checks every
   language, and without `--strict` a namespace still holding `{}` is only a warning.
4. Run `pnpm test`, which runs the same check and the rest of the app's tests.
5. See it in place: `pnpm dev`, open http://localhost:14200 in a browser, and pick your language
   from the **Language** row in the sidebar, under Dark mode. The browser preview uses made-up
   folders and packs, so every screen can be opened without the desktop app. Look at the sidebar,
   the buttons and the segmented controls (the rows of two or three options) at the window's
   smallest size: a word that's too long is cut off with "…" or wraps, and a shorter word is better.

## The rules

These hold for every language, English included. `pnpm check:locales` checks the first six.

- **Keys stay the same.** Only the values are translated.
- **Placeholders** are the words in double braces, like `{{name}}` or `{{count}}`. The app fills
  them in, so copy each one exactly, untranslated, and move it to wherever your grammar puts it:
  `"{{count}} skins"` is `"{{count}} 个皮肤"`. Numbers are written the way your language writes them
  (1 234, 1.234, 1,234) by the app, so never write a separator yourself.
- **Tags** put a few words in bold or make them a link. Keep each tag, with its closing tag, around
  the words it belongs to in your sentence, and translate the words inside:
  - `<b>…</b>`: bold, often a name or the key words of a sentence
  - `<a>…</a>` and `<link>…</link>`: a link, whose words you translate
  - `<muted>…</muted>`: words shown paler, like "on this machine" after how long a picture took
- **Plurals** are one key for each form your language has. English has two, `_one` and `_other`:

  ```json
  "count_one": "{{count}} folder",
  "count_other": "{{count}} folders"
  ```

  | Language | Forms | Which numbers take which |
  |---|---|---|
  | zh-CN, ja, ko | `_other` | every number |
  | fr | `_one`, `_many`, `_other` | `_one`: 0, 1 and 1.5. `_many`: a million and other large round numbers ("1 000 000 de dossiers"). `_other`: the rest |
  | es | `_one`, `_many`, `_other` | `_one`: 1. `_many`: a million and other large round numbers ("1 000 000 de carpetas"). `_other`: the rest, 0 included |

  Write exactly your language's forms, no more and no fewer. A form other than `_other` may leave
  `{{count}}` out ("Un dossier"). `_other` keeps it.
- **No empty messages.**
- **No em dashes and no semicolons**, in any language: no `—`, `――`, `––`, `——`, `;` or `；`. Rewrite
  the sentence instead: two sentences, a colon, a comma, or brackets.
- **Never translate** FolderSkin, macOS, Windows, Linux, Finder, GitHub, API, GPL-3.0, FLUX.2, MIT,
  CC0, CC BY, Manrope, the AI providers' names (OpenAI, Google Gemini, xAI Grok, Recraft, Black
  Forest Labs, Stability AI, Ideogram), file names, commands, code and addresses.
- **File managers** go by the name each system gives them in your language. Finder stays "Finder"
  in every language, as Apple writes it. Windows' File Explorer and Linux's Files (GNOME's file
  manager) take the names Windows and GNOME give them:

  | English | zh-CN | ja | ko | fr | es |
  |---|---|---|---|---|---|
  | Finder (macOS) | Finder | Finder | Finder | Finder | Finder |
  | Explorer (Windows) | 文件资源管理器 | エクスプローラー | 파일 탐색기 | Explorateur de fichiers | Explorador de archivos |
  | Files (Linux) | 文件 | ファイル | 파일 | Fichiers | Archivos |

  They're in `common.showIn.*`, `composer.sizes.tip.*`, `folder.stage.status.catchesUp.*`,
  `folder.stage.status.catchUp.*` and `settings.about.points.fileBrowser.*`, each with a `macos`,
  a `windows` and a `linux` message, of which the app shows the one for the computer it runs on.
- **Tone:** natural and native, the way the app would read if it had been written in your language.
  Rewrite rather than translate word for word. Buttons stay short. Say it plainly and warmly, as the
  English does.

### Register and punctuation

| Language | Register | Punctuation |
|---|---|---|
| zh-CN | friendly and plain, 你 (not 您) | full-width `，。：！？` with `“”` for quotes |
| ja | です/ます in sentences, short noun phrases on buttons | `、。` with `「」` |
| ko | 해요체 in sentences, short noun forms on buttons | as Korean writes it, with spaces between words |
| fr | vous, friendly but correct | a non-breaking space before `: ! ?` and `« »` for quotes |
| es | tú, neutral Spanish for Spain and Latin America (no vosotros, no regional slang) | `¿…?` and `¡…!` |

### What each kind of key is

- A key ending in **`Label`** is what a screen reader says for a control that shows only an icon,
  like `sidebar.collapseLabel`, "collapse the sidebar". English starts these with a small letter,
  and so do French and Spanish.
- A key ending in **`Tip`** is a tooltip, shown when the pointer rests on a control.
- A key ending in **`Placeholder`** is the pale hint inside an empty text field.
- **Sentences that start with a small letter** and have no full stop are the end of a longer one,
  after a colon: "Couldn't apply it: its disk is full". Keep them that way where your language
  has capitals.
- **`common.twoSentences`** (`"{{first}} {{second}}"`) puts two sentences side by side. Chinese
  and Japanese write it with no space: `"{{first}}{{second}}"`.
- **`sidebar.withCount`** (`"{{label}} · {{count}}"`) is the tooltip of an entry on the folded
  sidebar that has a number beside it. Keep the middle dot.
- **`settings.find.*`** and the keys ending in **`Find`** are the words Settings' search answers to,
  besides the words on show: "dark light system mode" finds the Theme setting. Write the words
  someone might type in your language, separated by spaces. Keeping the English ones as well does
  no harm.

## The namespaces

| Namespace | What it holds | Where it shows |
|---|---|---|
| `sidebar` | the sidebar's entries, Dark mode, Language, Settings | the column on the left |
| `library` | the skin gallery: tabs, sorting, filters, colours, the empty states, a skin's menu | the middle of the window |
| `folder` | the folder panel: trying a skin on, applying it, subfolders, putting an icon back | the panel on the right |
| `community` | Community: packs, search, sorting, installing and removing | Community, in the sidebar |
| `share` | sharing a pack: the form, verifying a computer, submissions, the pack terms | Community's "Share yours" |
| `composer` | the composer: templates, layers, the inspector, fonts, colours, icons, saving | Design your own |
| `ai` | the AI chat, its styles, providers and keys, the Local Model | Generate with AI |
| `settings` | Settings, every page of it | Settings, in the sidebar |
| `onboarding` | the welcome on first launch and its starter packs | the first launch |
| `updates` | the updater | when an update is ready |
| `common` | words used all over: Cancel, sizes, file dialogs, About, licences | everywhere |
| `menu` | the macOS menu bar | the top of the screen on a Mac |
| `native` | sentences from the app's engine (errors and progress) | in messages, under a failed action |

### Where room is tight

| Keys | Where | Room |
|---|---|---|
| `sidebar.items.*`, `sidebar.groups.*`, `sidebar.darkMode`, `sidebar.settings`, `sidebar.language.menu` | the sidebar, 228 points wide | about 22 characters, or 11 in Chinese, Japanese and Korean |
| the options of a segmented control, such as `library.sort.*`, `community.views.*`, `community.sort.*`, `folder.look.name.*` and `settings.general.themes.*` | a row of two to five options | about 12 characters |
| buttons | a line of their own | close to the English, at most a third longer |
| `composer.templateWords.*` | words drawn big on a template's folder, like "IDEAS" or "Summer 2026" | 10 characters or fewer. Use capitals where English does, if your language has them |
| `composer.templates.*`, `composer.fonts.*`, `composer.weights.*`, `composer.layerNames.*` | tiles and menus in the composer | about 14 characters |
| `ai.styles.*` | the style chips in the AI chat | about 16 characters |
| `menu.*` | the macOS menu bar | the exact words macOS itself uses in your language, as in Finder's own menus |

### `native`: the engine's sentences

The app's engine writes its errors and progress in English, and the app shows them in your
language by finding each sentence in `native.json`. Translate the values as usual. The English
values must stay word for word what the engine writes, so a change to one belongs in the engine
too (a test checks). `native.os.*` are the operating system's own messages, such as "Permission
denied (os error 13)": translate them as macOS and Windows say them in your language.

## Glossary

Use these words everywhere, so the app, the website and the guides say the same thing.

| English | zh-CN | ja | ko | fr | es |
|---|---|---|---|---|---|
| skin (a folder's picture) | 皮肤 | スキン | 스킨 | habillage (m.) | aspecto (m.) |
| skins | 皮肤 | スキン | 스킨 | habillages | aspectos |
| pack (a set of skins) | 皮肤包 | パック | 팩 | pack (m.) | paquete (m.) |
| community pack | 社区皮肤包 | コミュニティパック | 커뮤니티 팩 | pack de la communauté | paquete de la comunidad |
| folder | 文件夹 | フォルダ | 폴더 | dossier | carpeta |
| subfolders | 子文件夹 | サブフォルダ | 하위 폴더 | sous-dossiers | subcarpetas |
| Library | 皮肤库 | ライブラリ | 라이브러리 | Bibliothèque | Biblioteca |
| All skins | 全部皮肤 | すべてのスキン | 모든 스킨 | Tous les habillages | Todos los aspectos |
| Yours (skins you made) | 我的皮肤 | マイスキン | 내 스킨 | Mes habillages | Mis aspectos |
| Favourites | 收藏 | お気に入り | 즐겨찾기 | Favoris | Favoritos |
| Add your photo | 添加你的照片 | 写真を追加 | 사진 추가 | Ajouter une photo | Añadir tu foto |
| Design your own | 自己设计 | 自分でデザイン | 직접 디자인 | Créer le vôtre | Diseña el tuyo |
| the composer | 设计器 | デザイナー | 디자이너 | l'éditeur | el editor |
| Generate with AI | AI 生成 | AIで生成 | AI로 생성 | Générer avec l'IA | Generar con IA |
| Community | 社区 | コミュニティ | 커뮤니티 | Communauté | Comunidad |
| Try on / Trying on X | 试穿 / 正在试穿 X | 試着 / X を試着中 | 미리 입혀 보기 / X 입혀 보는 중 | Essayer / Essai de X | Probar / Probando X |
| Apply skin | 应用皮肤 | スキンを適用 | 스킨 적용 | Appliquer l'habillage | Aplicar aspecto |
| Put the folder's own icon back | 恢复文件夹原来的图标 | フォルダの元のアイコンに戻す | 폴더 원래 아이콘으로 되돌리기 | Rétablir l'icône d'origine | Restaurar el icono original |
| Include subfolders | 包括子文件夹 | サブフォルダも含める | 하위 폴더 포함 | Inclure les sous-dossiers | Incluir subcarpetas |
| Install (a pack) | 安装 | インストール | 설치 | Installer | Instalar |
| Official | 官方 | 公式 | 공식 | Officiel | Oficial |
| Featured | 精选 | 注目 | 추천 | À la une | Destacado |
| Share a pack | 分享皮肤包 | パックを共有 | 팩 공유 | Partager un pack | Compartir un paquete |
| sent for review / reviewed by a person | 提交审核 / 由真人审核 | 審査に送信 / 人が審査 | 검토 요청 / 사람이 검토 | envoyé pour relecture / relu par une personne | enviado a revisión / revisado por una persona |
| Local Model | 本地模型 | ローカルモデル | 로컬 모델 | Modèle local | Modelo local |
| API key | API 密钥 | APIキー | API 키 | clé d'API | clave de API |
| provider | 服务商 | プロバイダ | 제공업체 | fournisseur | proveedor |
| Whole folder | 整个文件夹 | フォルダ全体 | 폴더 전체 | Dossier entier | Carpeta entera |
| Just the art | 只要图案 | 絵だけ | 그림만 | Juste l'image | Solo la imagen |
| style (art style preset) | 风格 | スタイル | 스타일 | style | estilo |
| Settings | 设置 | 設定 | 설정 | Réglages | Ajustes |
| Dark mode | 深色模式 | ダークモード | 다크 모드 | Mode sombre | Modo oscuro |
| Language | 语言 | 言語 | 언어 | Langue | Idioma |
| Mac look / Windows look (the folder style) | Mac 风格 / Windows 风格 | Mac 風 / Windows 風 | Mac 스타일 / Windows 스타일 | Style Mac / Style Windows | Estilo Mac / Estilo Windows |
| licence (of a pack) | 许可协议 | ライセンス | 라이선스 | licence | licencia |
| free and open source | 免费开源 | 無料のオープンソース | 무료 오픈 소스 | gratuit et open source | gratis y de código abierto |
| no account | 无需账号 | アカウント不要 | 계정 불필요 | sans compte | sin cuenta |
| Download | 下载 | ダウンロード | 다운로드 | Télécharger | Descargar |
| Give any folder a skin. (tagline) | 给任何文件夹换上皮肤。 | どんなフォルダにもスキンを。 | 어떤 폴더에든 스킨을 입혀 보세요. | Habillez n'importe quel dossier. | Dale un aspecto a cualquier carpeta. |

The language names are never translated, and each is written in its own language: English,
简体中文, 日本語, 한국어, Français, Español. The app lists them from `src/i18n/locales.ts`, not from
the catalogs.

### More words in Chinese

The Chinese guides use these too, and the app says the same:

| English | zh-CN |
|---|---|
| finished folder | 成品文件夹 |
| artwork | 图案 |
| Pattern (the composer's) | 花纹 |
| folder tab | 页签 |
| magenta | 品红 |
| Revert / Revert all | 还原 / 全部还原 |
| Remove custom icon | 移除自定义图标 |
| Show in Finder | 在 Finder 中显示 |
| Applied | 已应用 |
| Share with community | 分享到社区 |
| Your submissions | 我的投稿 |
| Add from a folder | 从文件夹添加 |
| List / Gallery | 列表 / 画廊 |
| Refresh | 刷新 |
| Update and restart | 更新并重启 |
| Save & apply | 保存并应用 |
| Save to Yours | 保存到我的皮肤 |
| Save changes | 保存修改 |
| Save as new | 另存为新皮肤 |
| Edit design | 编辑设计 |
| Remix in the composer | 在设计器中再创作 |
| Folder skeleton | 文件夹骨架 |
| Free icon | 自由图标 |
| On the folder | 贴在文件夹上 |
| AI Provider | AI 服务商 |
| Remove key | 移除密钥 |
| Generate | 生成 |
| No API key? | 没有 API 密钥？ |
| the AI styles: Travel poster, Woodblock, 70s airbrush, Collage, Art nouveau, Oil painting, Film still, Tiny diorama, Risograph, Clay | 旅行海报, 木版画, 70 年代喷绘, 拼贴, 新艺术, 油画, 电影剧照, 微缩场景, 孔版印刷, 黏土 |

## What stays in English

- The briefs the AI styles and suggestions send to the image model, which reads English best.
  They're in the code, not here, and a translated style still sends its English brief.
- The emoji picker's search words and the icon packs' icon names, which come from the packs.
- Messages from FolderSkin's community service (sharing and verifying), which it writes itself.
- A few rare technical errors from the engine that aren't in `native.json`: they show as written.
- The release notes shown by the updater, which are written once, in English, for each release.

## For developers

**Adding an English message:** add it to the right namespace in `en/`, then use it with
`t("namespace.key")` (or `useT()` in a component). The key is type-checked against English. In a
namespace a language has translated, add the translation in the same change, or the check fails.
Every file that uses a `composer.*` key imports `src/i18n/composer.ts`, which brings the composer's
English with the composer's own code.

**Adding a language:**

1. In `src/i18n/locales.ts`, add its code to `LOCALES`, its own name to `LOCALE_NAMES`, the locale
   `Intl` formats it with to `INTL_LOCALES`, and the website's path for it to `SITE_PATHS` (the
   guides have to be there). If a computer names it in several ways, as Chinese is, teach
   `matchLocale` about them.
2. Make `src/locales/<code>/` with a `{}` file for each of English's namespaces.
3. For macOS: add the language to `CFBundleLocalizations` in `src-tauri/Info.plist`, a
   `src-tauri/lproj/<apple code>.lproj/InfoPlist.strings` like the others, its line in
   `bundle.macOS.files` in `src-tauri/tauri.conf.json`, and its pair in `LANGUAGES` in
   `src-tauri/src/language.rs`.
4. If its script needs fonts of its own, add a `:lang()` rule for `--font` in
   `src/styles/tokens.css`, as Chinese, Japanese and Korean have.
5. Run `pnpm check:locales`, `pnpm test` and `cargo test --workspace`.
