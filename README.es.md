<div align="center">

<img src="public/app-icon.png" alt="Logotipo de FolderSkin" width="112" height="112" />

# FolderSkin

[![Descargar para macOS](https://img.shields.io/badge/Download_for_macOS-111827?style=for-the-badge&logo=apple&logoColor=white)](https://github.com/prajwal-svm/folderskin/releases/latest)
[![Descargar para Windows](https://img.shields.io/badge/Download_for_Windows-3A86FF?style=for-the-badge&logo=data:image/svg%2bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCA0NDggNTEyIj48cGF0aCBmaWxsPSIjZmZmIiBkPSJtMCA5My43bDE4My42LTI1LjN2MTc3LjRIMHptMCAzMjQuNmwxODMuNiAyNS4zVjI2OC40SDB6bTIwMy44IDI4TDQ0OCA0ODBWMjY4LjRIMjAzLjh6bTAtMzgwLjZ2MTgwLjFINDQ4VjMyeiIvPjwvc3ZnPg==)](https://github.com/prajwal-svm/folderskin/releases/latest)
[![Descargar para Linux](https://img.shields.io/badge/Download_for_Linux-FCC624?style=for-the-badge&logo=linux&logoColor=111827)](https://github.com/prajwal-svm/folderskin/releases/latest)

[![CI](https://github.com/prajwal-svm/folderskin/actions/workflows/ci.yml/badge.svg)](https://github.com/prajwal-svm/folderskin/actions/workflows/ci.yml) <!-- Insignias de SonarQube Cloud, ocultas hasta que el proyecto exista allí (docs/CI.md): [![Quality gate](https://sonarcloud.io/api/project_badges/measure?project=prajwal-svm_folderskin&metric=alert_status)](https://sonarcloud.io/summary/new_code?id=prajwal-svm_folderskin) [![Cobertura](https://sonarcloud.io/api/project_badges/measure?project=prajwal-svm_folderskin&metric=coverage)](https://sonarcloud.io/component_measures?id=prajwal-svm_folderskin&metric=coverage) --> [![Descargas](https://img.shields.io/endpoint?url=https%3A%2F%2Fraw.githubusercontent.com%2Fprajwal-svm%2Ffolderskin%2Fbadges%2Fdownloads.json)](https://github.com/prajwal-svm/folderskin/releases) [![Versión](https://img.shields.io/github/package-json/v/prajwal-svm/folderskin?label=version&color=3A86FF)](CHANGELOG.md) [![Último commit](https://img.shields.io/github/last-commit/prajwal-svm/folderskin?label=last%20commit&color=3A86FF)](https://github.com/prajwal-svm/folderskin/commits/main) [![Licencia: GPL-3.0](https://img.shields.io/badge/license-GPL--3.0-3A86FF)](LICENSE) [![Hecho con Tauri 2](https://img.shields.io/badge/built%20with-Tauri%202-24C8DB?logo=tauri&logoColor=white)](https://tauri.app) [![Paquetes de la comunidad bienvenidos](https://img.shields.io/badge/community%20packs-welcome-12b981)](https://github.com/prajwal-svm/folderskin-community) [![Estrellas](https://img.shields.io/github/stars/prajwal-svm/folderskin?style=social)](https://github.com/prajwal-svm/folderskin)

**Dale un aspecto a cualquier carpeta.**

Tus mejores recuerdos viven en la misma carpeta sin gracia que tus viejos papeles. FolderSkin le da a
cada carpeta un aspecto que va con lo que guarda: un fotograma de cine a la hora dorada para las
fotos del verano, un póster de viaje vintage para una escapada, pop art para un proyecto de video,
pasteles suaves para un cumpleaños. Suelta una carpeta en la ventana, pruébale aspectos y aplica el
que más te guste. Elige entre los paquetes gratuitos de la comunidad, usa una foto tuya, diseña el
tuyo a partir de un color, una palabra o un emoji, o describe cualquier estilo y deja que la IA lo
pinte.

<p>
  <a href="https://github.com/prajwal-svm/folderskin/releases/latest"><strong>Descargar gratis</strong></a>
  · <a href="https://folderskin.app/es/">folderskin.app</a>
  · <a href="https://folderskin.app/es/community/">Paquetes de la comunidad</a>
  · <a href="docs/es/PACKS.md">Compartir un paquete de aspectos</a>
  · <a href="#compilar-desde-el-código-fuente">Compilar desde el código fuente</a>
  · <a href="https://github.com/prajwal-svm/folderskin">⭐ Dar una estrella en GitHub</a>
</p>

Gratis · Código abierto · Sin cuenta · Sin rastreo

</div>

[English](README.md) · [简体中文](README.zh-CN.md) · [日本語](README.ja.md) · [한국어](README.ko.md) · [Français](README.fr.md) · Español

<div align="center">
  <img src="docs/images/app.webp" alt="FolderSkin 0.1.7 en macOS: el paquete Scientists Pop Art en la biblioteca, e Isaac Newton probado en la carpeta Descargas" width="100%" />
</div>

<details><summary>Modo oscuro</summary>

![FolderSkin en modo oscuro](docs/images/app-dark.webp)

</details>

## Por dónde empezar

| Si quieres… | Haz esto |
| --- | --- |
| Conseguir tus primeros aspectos | En el primer inicio, FolderSkin te ofrece los paquetes de la comunidad, con Classic Art ya seleccionado |
| Darle un aspecto nuevo a una carpeta | Suelta la carpeta en la ventana, haz clic en un aspecto y luego en **Aplicar aspecto** |
| Aplicarlo también a las carpetas que contiene | Activa **Incluir subcarpetas** debajo de la carpeta y luego **Aplicar a** todas |
| Usar una foto tuya | Suelta la imagen en la ventana o haz clic en **Añadir tu foto** |
| Diseñar el tuyo | **Diseña el tuyo**: empieza con un color, un rótulo, un emoji o una foto, cambia lo que quieras y luego **Guardar y aplicar** |
| Que una IA te pinte uno | **Generar con IA**, con tu propia clave de API, o el prompt para el chat de Grok o de ChatGPT en **¿Sin clave de API?** |
| Conseguir aspectos que hicieron otras personas | **Comunidad** y luego añade un paquete |
| Compartir los tuyos | Menú ⋯ de un aspecto → **Compartir con la comunidad** |
| Volver a encontrar un aspecto | Las etiquetas de arriba, ⌘F / Ctrl+F, el botón de filtro (color, paquete, cuándo lo añadiste y más) o la estrella de **Favoritos** |
| Deshacerlo | **Restaurar**, o **Quitar el icono personalizado** en una carpeta que ya tiene uno, vuelve a poner el icono del sistema operativo |

## Qué se queda en tu equipo

Todo, salvo las pocas cosas que necesitan internet. No hay cuenta, ni muro de pago, ni nada que te
rastree: lo único que FolderSkin comunica es que se añadió un paquete de la comunidad, solo con el
identificador del paquete, para que folderskin.app pueda mostrar cuántas veces se añade cada uno. En
Mac, la descarga ocupa menos de 20 MB.

| Se queda en tu equipo | Sale a internet |
| --- | --- |
| Tus carpetas y los iconos que FolderSkin escribe en ellas | Una solicitud de IA, cuando haces una: va al proveedor que elegiste, con tu clave |
| Cada imagen que añades y cada aspecto que creas | Comunidad y el primer inicio, que leen los paquetes compartidos desde GitHub |
| Tus claves de API, cifradas | La búsqueda de actualizaciones: al abrirse, FolderSkin lee en GitHub el archivo que indica la última versión |
| Favoritos, etiquetas y ajustes | Añadir un paquete desde Comunidad: su identificador, y nada más, va al servicio de la comunidad de FolderSkin, que cuenta los paquetes añadidos una sola vez al día por red y no guarda ninguna dirección ([detalles](docs/es/PACKS.md#recuento-de-instalaciones)) |

## Cómo se usa

La primera vez que se abre, FolderSkin muestra una breve bienvenida y luego te ofrece los paquetes
de aspectos de la comunidad, con Classic Art ya seleccionado. Añade los que quieras, o ninguno: cada
paquete se añade entero o no se añade, y siempre puedes llenar la biblioteca más adelante desde
**Comunidad**. La bienvenida no vuelve a aparecer.

<details><summary>El primer inicio</summary>

![El primer inicio ofrece los paquetes de la comunidad, con Classic Art seleccionado](docs/images/first-launch.png)

</details>

1. Arrastra una carpeta al panel de la carpeta, a la derecha, o haz clic en la carpeta vacía para
   elegir una.
2. Haz clic en un aspecto de la biblioteca para probarlo. El panel de la carpeta muestra el
   resultado al instante. En la barra lateral, elige **Todos los aspectos**, **Mis aspectos** o
   **Favoritos**. Las etiquetas de arriba acotan la lista y ⌘F / Ctrl+F busca.
3. Haz clic en **Aplicar aspecto**. La carpeta muestra **Aplicado** y **Mostrar en Finder** la
   abre.
4. Haz clic en **Restaurar** para volver a poner el icono predeterminado del sistema operativo. Si
   la carpeta ya tiene un icono personalizado, en cuanto la eliges aparece **Quitar el icono
   personalizado**.

Para darles el mismo aspecto a las carpetas que contiene, activa **Incluir subcarpetas** debajo del
nombre de la carpeta. Primero las cuenta (en todos los niveles, sin las carpetas ocultas ni las
apps), y el botón pasa a ser **Aplicar a 25 carpetas**, o las que haya. Si son más de diez, te
pregunta antes de empezar. El panel de la carpeta muestra cada carpeta a medida que termina,
**Detener** termina el proceso después de la carpeta en curso, y el resumen indica qué cambió, qué
carpetas no se pudieron cambiar y por qué, y te ofrece continuar o volver a intentarlo con esas.
**Restaurar todo** quita exactamente lo que puso el proceso.

Para usar tu propia imagen, suéltala en la ventana o haz clic en **Añadir tu foto**. Se guarda en
**Mis aspectos** y se queda ahí hasta que la borres (FolderSkin te pide confirmación antes). Una carpeta
terminada sobre un fondo magenta liso, como las que produce el prompt para chat de más abajo, se
recorta y se usa tal cual. Cualquier otra imagen se coloca sobre la carpeta de FolderSkin.

Cada aspecto que añades tiene un menú ⋯ para cambiarle el nombre (un doble clic en el nombre, o F2,
te lleva directo ahí), ponerle etiquetas (se convierten en filtros en la parte de arriba), ver cómo
se hizo (el modelo de IA y el prompt, o el paquete y quién lo compartió), compartirlo o borrarlo. Los
resultados de la IA llegan ya etiquetados con su estilo, como `airbrush`.

**Ajustes**, al final de la barra lateral, reúne el tema, tus claves de API, los datos que se
completan al compartir y dónde se guardan tus aspectos. Pasa el puntero sobre la insignia de
versión, junto al logotipo, para ver “Acerca de”.

## Aspectos de la comunidad

FolderSkin no trae aspectos propios. La gente comparte aspectos y paquetes de aspectos a través de
FolderSkin, gratis para todos. El primer inicio te los ofrece, y en **Comunidad** los tienes
siempre. Al añadir un paquete, sus aspectos pasan a tu biblioteca con sus etiquetas, y el botón
**Instalar** de un paquete en la galería de [folderskin.app](https://folderskin.app/es/community/)
abre FolderSkin y lo añade por ti. Los paquetes marcados como **Oficial** son los que avala el
mantenedor. **Classic Art** es un buen primer paquete: dieciséis pinturas de dominio público, de la
Mona Lisa a La noche estrellada, cada una pintada sobre una carpeta. Para compartir los tuyos, abre
el menú ⋯ de un aspecto y elige **Compartir con la comunidad**, o usa **Comunidad → Compartir tus
aspectos** para compartir varios. Verificas tu equipo una sola vez en el navegador, sin cuenta, y
una persona revisa el paquete antes de que se sume a Comunidad para todos. **Guardar en una
carpeta**, en el mismo cuadro de diálogo, escribe el paquete como archivos. Las imágenes se
comparten sin pérdida, así que un paquete se ve exactamente como lo hiciste.
[docs/es/PACKS.md](docs/es/PACKS.md) explica el contrato y sus límites: de 1 a 50 aspectos por
paquete, imágenes de hasta 1024 px y 1.5 MB, 64 MB por paquete, con licencia CC0, CC BY 4.0 o MIT.

## Diseña el tuyo

**Diseña el tuyo**, en la barra lateral, crea un aspecto desde cero, sin conexión. Empieza con un
color liso, un rótulo, un emoji, dos tonos, cristal, rayas o una foto con leyenda, y luego cámbialo
todo:

- cualquier color con cualquier nivel de transparencia, y degradados
- texto en diecisiete estilos de fuente, que se puede curvar en arco
- emojis, trece formas y diez patrones, hasta grano de película
- tus propias imágenes, con ajustes
- sombras, resplandores y bordes de sticker

El interruptor **Esqueleto de la carpeta** muestra el diseño sobre la carpeta o en plano, y el icono
de al lado, a los tamaños que usa el Finder, muestra si se lee bien. Haz una carpeta de cristal
transparente, o elige **Icono libre** para un sticker que no tenga forma de carpeta. **Guardar y
aplicar** lo pone en tu carpeta, y **Editar el diseño**, en su menú ⋯, lo vuelve a abrir.
[docs/es/COMPOSER.md](docs/es/COMPOSER.md) tiene los detalles.

<details><summary>El editor</summary>

![Diseñando un aspecto: un insecto de la biblioteca de iconos estampado en una carpeta azul, con la búsqueda de iconos al lado](docs/images/composer.webp)

</details>

## Generar un aspecto con IA

FolderSkin puede crear un aspecto a partir de una descripción. El **Modelo local** lo pinta en tu
propio equipo, gratis: lo configuras una vez, y no necesita clave ni envía nada a ninguna parte.
Funciona en Mac con chip de Apple y macOS 14 o posterior, y en PC con Windows y Linux. O usa **tu
propia clave de API** de un proveedor que ya uses. La clave se cifra y se guarda en tu equipo (sin
que el llavero te pida contraseñas), FolderSkin no tiene servidor propio y no se envía nada hasta
que presionas Enter. Abre **Generar con IA**, describe una escena, escoge un estilo y elige entre
**Carpeta entera** (el modelo pinta toda la carpeta a partir de la plantilla de FolderSkin, como un
póster) o **Solo la imagen** (arte plano que se coloca sobre la carpeta de FolderSkin). Cada
resultado se guarda en **Mis aspectos** y puedes probarlo al momento.

Elige dónde se crean las imágenes en **Ajustes → Proveedor de IA**: configura ahí el Modelo local o
pega una clave. El nombre de cada proveedor lleva a la página donde se crea una.

| | Proveedor | Modelos | Por imagen |
| :-: | --- | --- | --- |
| <picture><source media="(prefers-color-scheme: dark)" srcset="docs/images/providers/openai-dark.svg"><img src="docs/images/providers/openai.svg" width="20" height="20" alt=""></picture> | [OpenAI](https://platform.openai.com/api-keys) | GPT Image 2.5 Flare, GPT Image 2.5 Sunburst, GPT Image 1 | ~0.02–0.19 USD |
| <picture><source media="(prefers-color-scheme: dark)" srcset="docs/images/providers/xai-dark.svg"><img src="docs/images/providers/xai.svg" width="20" height="20" alt=""></picture> | [xAI Grok](https://console.x.ai) | Grok Imagine | ~0.02 USD |
| <picture><source media="(prefers-color-scheme: dark)" srcset="docs/images/providers/recraft-dark.svg"><img src="docs/images/providers/recraft.svg" width="20" height="20" alt=""></picture> | [Recraft](https://www.recraft.ai/profile/api) | Recraft V3 | ~0.04 USD |
| <img src="docs/images/providers/google.svg" width="20" height="20" alt=""> | [Google Gemini](https://aistudio.google.com/apikey) | Gemini 2.5 Flash Image | ~0.04 USD |
| <picture><source media="(prefers-color-scheme: dark)" srcset="docs/images/providers/bfl-dark.svg"><img src="docs/images/providers/bfl.svg" width="20" height="20" alt=""></picture> | [Black Forest Labs](https://dashboard.bfl.ai) | FLUX 1.1 Pro | ~0.04 USD |
| <img src="docs/images/providers/stability.svg" width="20" height="20" alt=""> | [Stability AI](https://platform.stability.ai/account/keys) | Stable Image Core | ~3 créditos |
| <picture><source media="(prefers-color-scheme: dark)" srcset="docs/images/providers/ideogram-dark.svg"><img src="docs/images/providers/ideogram.svg" width="20" height="20" alt=""></picture> | [Ideogram](https://ideogram.ai/manage-api) | Ideogram v3 | ~0.03–0.09 USD |

¿No tienes clave? [docs/es/PROMPTS.md](docs/es/PROMPTS.md) tiene una plantilla y un prompt para el
propio chat de Grok o de ChatGPT. La app muestra el mismo prompt, ya completado, en
**¿Sin clave de API?**

[docs/es/AI.md](docs/es/AI.md) explica los proveedores, dónde se guarda la clave, cómo se gestiona
la transparencia en los modelos que no pueden devolver un canal alfa y qué significa cada mensaje de
error.

## Crear un paquete

Una serie temática, como carpetas en 3D renderizadas con un modelo de imagen, se convierte en un
paquete de la comunidad con un solo comando. Los paquetes viven en su propio repositorio,
[folderskin-community](https://github.com/prajwal-svm/folderskin-community): clónalo junto a este.
`folderskin-tools packs make` recorta las carpetas terminadas de su fondo magenta (`--flat-backdrop`
para cualquier otro fondo liso, como un magenta que viró a rosa o un gris liso), reduce y comprime
cada imagen para que quepa en los límites, les da una sola forma a las carpetas terminadas y escribe
`pack.json`:

```sh
cargo run -p folderskin-tools -- packs make ~/Pictures/renders --dir ../folderskin-community \
  --name "3D Folders" --tags 3d,glossy --author your-github-name \
  --preview /tmp/3d-folders.png
cargo run -p folderskin-tools -- packs check --dir ../folderskin-community
```

`render` muestra cualquier imagen como la carpeta que produce, y `guide` dibuja las zonas seguras de
la plantilla para las ilustraciones que se colocan sobre la carpeta.
[docs/es/PACKS.md](docs/es/PACKS.md) explica el contrato y cómo se propone un paquete,
[docs/es/SKINS.md](docs/es/SKINS.md) cómo una imagen se convierte en icono, y
[.claude/skills/folderskin-skins/SKILL.md](.claude/skills/folderskin-skins/SKILL.md) le enseña a
Claude Code a hacer todo el ciclo, así que pedirle “haz un paquete con estos renders” es
perfectamente razonable.

## Cómo funciona

El núcleo en Rust (`crates/folderskin-core`) guarda la plantilla de la carpeta como trazados
vectoriales, ajusta tu imagen para cubrir el panel trasero y el delantero, renderiza todo una sola
vez a 2048 px con `tiny-skia` y lo reduce a cada tamaño de icono con Lanczos3. La webview nunca
dibuja la geometría de la carpeta. Muestra los PNG que renderizó el núcleo, así que la miniatura de
la galería, la vista previa y el icono en el disco son los mismos píxeles en todas las plataformas.
[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) tiene los detalles (en inglés).

## Notas por plataforma

| sistema | mecanismo | archivos que se escriben dentro de la carpeta | a tener en cuenta |
|---|---|---|---|
| macOS | `NSWorkspace.setIcon` | el archivo invisible `Icon\r` que mantiene macOS | nada: el Finder se actualiza al instante |
| Windows | `desktop.ini` + `folderskin-<hash>.ico`, ambos ocultos + de sistema, la carpeta marcada como de solo lectura, y luego `SHChangeNotify` sobre la carpeta y la que la contiene | `desktop.ini`, `folderskin-<hash>.ico` | nada: la carpeta se redibuja en cuanto termina de aplicarse |
| Linux | `.directory` para KDE, más `gio set metadata::custom-icon` para Nautilus, Nemo y Caja | `.directory`, `.folderskin.png` | algunos gestores de ventanas en mosaico y gestores de archivos minimalistas no leen ninguno de los dos |

Restaurar solo quita lo que escribió FolderSkin, y se puede ejecutar dos veces sin problema. Las
carpetas sincronizadas en la nube (iCloud, OneDrive, Dropbox) sincronizarán los archivos auxiliares
con tus otros equipos. La explicación completa, incluido cómo restaurar a mano, está en
[docs/PLATFORMS.md](docs/PLATFORMS.md) (en inglés).

## Descargar

Gratis · Código abierto · Sin cuenta · Sin rastreo

| Plataforma | Formato | Descarga |
| --- | --- | --- |
| macOS 12 o posterior · chip de Apple e Intel | DMG universal | [![Descargar para macOS](https://img.shields.io/badge/Download_for_macOS-111827?style=for-the-badge&logo=apple&logoColor=white)](https://github.com/prajwal-svm/folderskin/releases/latest) |
| Windows 10 y 11 · x86_64 | `-setup.exe` o MSI | [![Descargar para Windows](https://img.shields.io/badge/Download_for_Windows-3A86FF?style=for-the-badge&logo=data:image/svg%2bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCA0NDggNTEyIj48cGF0aCBmaWxsPSIjZmZmIiBkPSJtMCA5My43bDE4My42LTI1LjN2MTc3LjRIMHptMCAzMjQuNmwxODMuNiAyNS4zVjI2OC40SDB6bTIwMy44IDI4TDQ0OCA0ODBWMjY4LjRIMjAzLjh6bTAtMzgwLjZ2MTgwLjFINDQ4VjMyeiIvPjwvc3ZnPg==)](https://github.com/prajwal-svm/folderskin/releases/latest) |
| Linux · x86_64 | AppImage, DEB o RPM | [![Descargar para Linux](https://img.shields.io/badge/Download_for_Linux-FCC624?style=for-the-badge&logo=linux&logoColor=111827)](https://github.com/prajwal-svm/folderskin/releases/latest) |
| Linux · ARM64 | AppImage, DEB o RPM | [![Descargar para Linux](https://img.shields.io/badge/Download_for_Linux-FCC624?style=for-the-badge&logo=linux&logoColor=111827)](https://github.com/prajwal-svm/folderskin/releases/latest) |

La app de macOS está firmada y notarizada por Apple, por lo que se abre como cualquier otra. El
instalador de Windows todavía no está firmado, así que SmartScreen pregunta antes: elige “Más
información” y luego “Ejecutar de todas formas”. Las versiones para Linux necesitan glibc 2.35 o
posterior (Ubuntu 22.04, Debian 12, Fedora 36 y posteriores).

Una vez instalado, FolderSkin se mantiene al día solo: cuando sale una versión nueva, te muestra qué
cambió, y **Actualizar y reiniciar** la instala. Cada actualización está firmada, y la app comprueba
la firma antes de instalar nada.

## Compilar desde el código fuente

Requisitos en todas las plataformas: [Rust](https://rustup.rs) (rustup instala en el primer uso la
versión que indica `rust-toolchain.toml`), Node 22 o posterior, y pnpm 11 (`packageManager` en
`package.json` indica la versión exacta).

- **macOS:** las herramientas de línea de comandos de Xcode (`xcode-select --install`).
- **Windows:** Visual Studio Build Tools con la carga de trabajo de C++, y el runtime de WebView2
  (ya incluido en Windows 11).
- **Linux:**

  ```sh
  sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file \
    libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
  ```

Luego:

```sh
pnpm install
pnpm tauri dev      # run it
pnpm tauri build    # installers land in target/release/bundle/
```

Las comprobaciones, que la CI ejecuta todas ([docs/CI.md](docs/CI.md) enumera cada job, en inglés):

```sh
cargo test --workspace
pnpm test
pnpm typecheck
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
```

## Contribuir

Los informes de errores y los aspectos son bienvenidos. Los aspectos llegan a través de los
[paquetes de la comunidad](docs/es/PACKS.md). [CONTRIBUTING.md](CONTRIBUTING.md) explica el flujo de
trabajo y las pocas reglas de la casa, y [SECURITY.md](SECURITY.md), cómo comunicar una
vulnerabilidad en privado. Los cambios se registran en [CHANGELOG.md](CHANGELOG.md), y
[docs/RELEASING.md](docs/RELEASING.md) explica cómo se compila, se firma y se publica una versión.
Estos documentos están en inglés.

Si FolderSkin les dio mejor aspecto a tus carpetas, una [estrella en GitHub](https://github.com/prajwal-svm/folderskin)
ayuda a que otras personas lo encuentren.

## Licencia

FolderSkin es software libre bajo la [GNU General Public License v3.0](LICENSE)
(`GPL-3.0-only`): úsalo, estúdialo, modifícalo y compártelo. Si compartes una versión modificada,
comparte su código fuente bajo la misma licencia. Las versiones hasta la 0.1.6 salieron con licencia
MIT y la conservan.

El nombre y el logotipo de FolderSkin no forman parte de la licencia. [TRADEMARKS.md](TRADEMARKS.md)
explica cómo usarlos. Los aspectos de los paquetes de la comunidad tienen sus propias licencias,
indicadas en cada paquete.

Copyright 2026, los colaboradores de FolderSkin.

[Seguridad](SECURITY.md) · [Contribuir](CONTRIBUTING.md) · [GPL-3.0](LICENSE) · [Marcas](TRADEMARKS.md)

## Historial de estrellas

<a href="https://www.star-history.com/#prajwal-svm/folderskin&Date">
 <picture>
   <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/svg?repos=prajwal-svm/folderskin&type=Date&theme=dark" />
   <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/svg?repos=prajwal-svm/folderskin&type=Date" />
   <img alt="Gráfico del historial de estrellas" src="https://api.star-history.com/svg?repos=prajwal-svm/folderskin&type=Date" />
 </picture>
</a>
