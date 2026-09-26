# El asistente de IA

FolderSkin puede generar un aspecto a partir de una descripción, de una de estas dos formas:

- **El Modelo local** pinta en tu propio equipo, gratis. Lo configuras una vez (FLUX.2 [klein] 4B,
  una descarga de 4.6 GB en Mac y de 5.2 GB en los demás sistemas), y funciona sin conexión, sin
  clave y sin enviar nada a ninguna parte. Funciona en Mac con chip de Apple y macOS 14 o posterior,
  y en PC con Windows y Linux.
- **Tu propia clave**: pegas una clave de API de un proveedor en el que ya tienes cuenta, la clave se
  guarda en un archivo privado en tu equipo y FolderSkin se comunica con ese proveedor directamente
  desde tu máquina.

Todo esto se elige en **Ajustes → Proveedor de IA**. Una marca indica que el Modelo local está
configurado, o que un proveedor tiene clave.

![Ajustes, Proveedor de IA: el Modelo local y siete proveedores, cada uno con una marca o con la etiqueta Sin clave, y debajo el modelo instalado en este equipo](../images/ai-providers.webp)

No hay servidor de FolderSkin, ni proxy, ni clave incluida, ni un plan gratuito que subvencionar. Con
una clave, no se envía nada hasta que haces clic en **Generar**, y lo que se envía es el prompt que
FolderSkin escribe a partir de tus palabras, el tamaño que necesita la forma y las imágenes que lo
acompañan: si es una carpeta entera, la propia plantilla en blanco de FolderSkin para esa carpeta, y
después de ella las imágenes de referencia que hayas añadido.

## Dónde se guarda la clave

Cifrada, en la carpeta propia de FolderSkin, y solo la puede leer tu cuenta de usuario:

| | |
|---|---|
| macOS | `~/Library/Application Support/app.folderskin.desktop/keys.json` |
| Windows | `%APPDATA%\app.folderskin.desktop\keys.json` |
| Linux | `~/.config/app.folderskin.desktop/keys.json` |

Las claves se sellan con AES-256-GCM antes de escribirse. La clave de cifrado se deriva
(HKDF-SHA256) de un secreto aleatorio guardado en `keys.secret`, junto a `keys.json`, y del
identificador de hardware de este equipo, así que `keys.json` por sí solo no revela nada, y los dos
archivos copiados a otro equipo no se abren allí: vuelve a introducir las claves en el nuevo. Los dos
archivos se crean con permisos solo para el propietario (0600) y se escriben de forma atómica. Lo que
ningún archivo puede hacer es mantener fuera a un programa que ya se ejecuta con tu usuario, que
podría leer los dos. Solo el llavero del sistema podría, con las peticiones de contraseña que se
explican abajo.

Por qué no el llavero del sistema: macOS vincula un elemento guardado en el llavero a la firma exacta
de la app que lo guardó. Las compilaciones de código abierto suelen estar sin firmar o con una firma
ad hoc, así que cada recompilación o actualización parecería una app distinta y volvería a pedirte la
contraseña de inicio de sesión. Una petición de contraseña de una app que acabas de descargar se
parece justo a lo que no es, así que FolderSkin no usa el llavero en absoluto.

La clave se lee en el momento de la solicitud, nunca aparece en un mensaje de error y nunca se
devuelve a la ventana de la app. **Eliminar la clave**, en el cuadro del proveedor, la borra del
archivo. Borrar `keys.json` las elimina todas.

## Para qué es la imagen

Cada imagen se crea para una forma. El botón que hay junto al nombre del modelo, debajo del cuadro
del prompt, muestra cuál es con un pequeño dibujo de ella. Haz clic en él, o escribe @ en el cuadro,
para elegir otra:

- **Carpeta Mac**: la carpeta de FolderSkin, tal como la muestra el Finder.
- **Carpeta Windows**: la carpeta que dibuja Windows.
- **Icono libre**: una sola cosa, como una mascota, un objeto o un personaje, por sí sola y sin
  ninguna carpeta alrededor. Se pone tal cual en cualquier carpeta.

Después de @, escribe parte de un nombre (`@win`) y presiona Retorno o Tab, o haz clic en la que
quieras. El botón cambia y la palabra con @ desaparece del cuadro. Un chat nuevo empieza con la
carpeta sobre la que el panel de la carpeta muestra los aspectos.

La forma pertenece al chat. Cada imagen se crea para la forma que estaba elegida cuando se envió, un
chat anterior se abre con la forma de su última imagen, y el aspecto se guarda como hecho para esa
forma. La forma decide el prompt, la plantilla, el tamaño y cómo se recorta el resultado, con todos
los proveedores y con el Modelo local.

## Solo la imagen o la carpeta entera

Para una carpeta, esta es la elección que más importa, y no tiene que ver con la calidad.

**Solo la imagen** pide al modelo una imagen plana con las proporciones de la propia carpeta
(1024 × 960 para la carpeta de Mac, 1024 × 800 para la de Windows), y FolderSkin la coloca sobre su
propia carpeta, exactamente igual que una foto que añades. La geometría es nuestra, así que cada
aspecto se alinea con todos los demás, en todos los tamaños de icono. Cualquier proveedor puede
hacerlo, incluidos los que no admiten transparencia. Es la opción predeterminada y la correcta la
mayoría de las veces.

**Carpeta entera** pide al modelo que pinte la propia carpeta, y esa imagen se convierte
directamente en el icono, sin pasar por el compositor. Renuncias a una geometría exacta al píxel y
ganas una ilustración que puede tener relieve de verdad y sobresalir por el borde superior de la
carpeta.

Cuando el modelo puede partir de una imagen (OpenAI, Grok, Gemini y FLUX.2, y el Modelo local),
FolderSkin envía como primera imagen su propia plantilla en blanco de la carpeta: la carpeta pintada
de un gris claro liso, centrada sobre un color de recorte liso, con las proporciones de la propia
carpeta y como mucho 1024 píxeles en su lado más largo (`Base::blank`). El prompt le pide al modelo
que vuelva a pintar exactamente esa carpeta, conservando su contorno, su pestaña, las partes que la
identifican como esa carpeta, su tamaño y su posición, y que deje el fondo como está. El color de
recorte nunca se nombra, porque un modelo al que se le habla del magenta lo usa al pintar. Después,
FolderSkin recorta la pintura siguiendo el propio contorno de la carpeta, así que el resultado
conserva la silueta de FolderSkin y los colores de la pintura llegan hasta el mismo borde. Con una
pintura que movió o deformó la carpeta, se usa en cambio el color de recorte. Las imágenes de
referencia que añades van después de la plantilla, cada una con la función que le diste, tantas como
admita el modelo.

Un **Icono libre** siempre se pinta entero: un solo motivo, completo, en el centro de un cuadrado,
sobre un fondo transparente o sobre un color de recorte, y después se recorta.

## Cómo se gestiona la transparencia

FolderSkin elige la vía correcta para el modelo que elegiste:

- **Alfa nativo.** La solicitud pide un fondo transparente y el PNG que llega ya lo tiene. FolderSkin
  solo recorta el margen transparente. Así funcionan GPT Image 2.5 Flare y Sunburst.
- **Sin alfa.** El prompt pide la carpeta sola sobre un color de recorte liso. FolderSkin quita
  después ese color, elimina el color de recorte que se filtró en el borde suave (el paso que evita
  que un recorte parezca tener un halo de color) y recorta. Un fondo que falta se puede detectar: si
  el borde no es del color de recorte, el modelo ignoró la instrucción, y FolderSkin guarda la imagen
  como ilustración para su propia carpeta en lugar de aplicar un icono roto. A Recraft también se le
  indica el color como parámetro (`controls.background_color`), así que su fondo sale liso sea cual
  sea el estilo que pidas.

El color de recorte es el magenta, `#FF00FF`, porque casi nunca aparece en el arte de una carpeta.
Es el verde, `#00FF00`, con Gemini, que sobre magenta deja un borde rojizo oscuro alrededor del
motivo, y con todo lo que deba ser rosa o violeta, que un recorte por magenta se comería: los estilos
Neón, Aerógrafo años 70, Pop art y Synthwave, y cualquier idea que nombre el rosa o el violeta en uno
de los idiomas de FolderSkin (pink, lilac, rose, rosa, morado, ピンク, 보라, 粉红 y similares). El
verde sí tiene su sitio en el arte de una carpeta, en cada hoja y cada prado, así que un fondo verde
solo se quita donde llega al borde de la imagen, a partir del tono de verde que se pintó de verdad, y
los verdes pintados sobre la carpeta se quedan (`matte::cutout_connected`).

Una carpeta entera pintada sobre la plantilla de FolderSkin no sigue ninguna de las dos vías. Se
recorta siguiendo el propio contorno de la plantilla, como se explica arriba, y solo recurre al color
de recorte cuando la carpeta se movió.

El código de recorte está en `crates/folderskin-core/src/matte.rs` y tiene pruebas unitarias,
incluido el caso de un motivo realmente rosa sobre un fondo magenta.

## Los prompts

Tus palabras nunca se reescriben. `crates/folderskin-ai/src/recipe.rs` reúne en una sola receta lo
que necesita una imagen: tu idea, la forma y lo que conserva, el estilo, el texto que haya que
rotular, tus imágenes y la función de cada una, y lo que va alrededor del motivo. Después,
`prompts.rs` redacta esa receta como mejor la entiende cada familia de modelos, siempre en el mismo
orden: qué pintar, cómo se ve, cómo se encuadra, las imágenes, el texto y lo que hay que dejar fuera.

- **OpenAI y Gemini** reciben líneas encabezadas por un nombre (Style, Composition, Lettering,
  Constraints), y las imágenes se llaman image 1, image 2 y así sucesivamente.
- **Grok** recibe lo mismo, con las imágenes llamadas `<IMAGE_0>`, `<IMAGE_1>`, como las nombra
  Grok.
- **FLUX** (Black Forest Labs y el Modelo local) recibe prosa sencilla, con el motivo primero y sin
  instrucciones sobre lo que no hay que dibujar, porque FLUX no tiene prompt negativo y pinta lo que
  se le dice que evite. El prompt del Modelo local no pasa de los 400 tokens que lee su codificador
  de texto y, si hace falta, reduce el estilo a su medio.
- **Ideogram y Recraft** reciben un encargo de diseño breve, con el texto al principio.
- **Stability** recibe una lista corta. Tanto Stability como Ideogram reciben lo que hay que dejar
  fuera como su prompt negativo (ver más abajo).

Las partes que hacen el trabajo son de estructura, no de estilo:

- **Los prompts del modo Solo la imagen** piden una sola imagen continua que llene el encuadre, con
  el motivo grande y en el centro, y dejan para cielo o textura la franja que tapa la pestaña de la
  carpeta, porque la plantilla la recorta o la curva. En la carpeta de Windows, también dejan libre
  la esquina superior izquierda.
- **Los prompts del modo Carpeta entera** nombran las partes de la carpeta, de atrás hacia delante, y
  lo que se queda como está: la pestaña única de la carpeta de Mac y su franja de papel clara, o el
  escalón curvo de la carpeta de Windows. Sin eso, los modelos producen sin falta carpetas apiladas y
  pestañas dobles.
- **Los prompts de Icono libre** piden un solo objeto completo, centrado y sin recortar dentro de un
  cuadrado, sin suelo, paisaje ni marco alrededor.
- **Las imágenes de referencia** se nombran por su número y su función. Un motivo sigue siendo
  reconocible, una imagen de estilo aporta su medio, su paleta, su luz y su textura, y nada de su
  contenido, y una imagen de colores aporta solo sus colores.
- **Lo que hay que dejar fuera** se escribe según la forma y el estilo: siempre bordes, marcos,
  marcas de agua y firmas, el texto salvo que lo hayas pedido, los tópicos del propio estilo (como el
  monte Fuji en un grabado en madera) salvo que tu idea los pida, y la apariencia de clip art en un
  estilo realista.
- **Ningún prompt indica un tamaño.** Los modelos no pintan al número de píxeles que leen, así que
  el tamaño va en los propios parámetros de la solicitud (ver más abajo).

Hay pruebas que comprueban que están las frases clave en cada familia y cada forma, y la receta
lleva un número de versión (`RECIPE_VERSION`), así que un aspecto puede decir qué receta lo hizo.

## Estilos

Escribe / en el cuadro del prompt para ver treinta estilos, en cinco grupos: Foto y 3D, Materiales y
artesanía, Pintura y dibujo, Impresión, y Digital y gráfico. Escribe parte de un nombre para acotar
la lista. Un estilo va aparte, junto al cuadro, nunca dentro de tus palabras, así que la idea se
conserva palabra por palabra y el estilo se añade después. Para quitarlo, haz clic en su x.

Cada estilo es una fila de `crates/folderskin-ai/src/styles.json`, que leen la app, la línea de
comandos (`folderskin ai styles` los enumera) y el código que escribe los prompts. Una fila tiene el
nombre del estilo, una descripción de una línea, las palabras que usa el prompt para él (el medio, y
después su técnica, su luz, su color y su textura, y nunca el nombre de un artista), cómo rotula las
palabras, lo que suele añadir sin que nadie lo pida, tres comprobaciones que un resultado debería
superar, y el preajuste propio de cada proveedor para él, cuando lo tiene (el preajuste de estilo de
Stability, y el preajuste o el tipo de estilo de Ideogram). Los nombres de estilo de versiones
anteriores siguen funcionando: `travel` es ahora Póster de viaje (`screenprint`), `ukiyoe` es
Grabado en madera y `diorama` es Efecto maqueta.

El mismo menú tiene **Ideas** para empezar (cada una es un motivo, sin estilo) y **Tus prompts**.
Los botones de estilo de un chat nuevo funcionan igual: cada clic llena el cuadro con otra idea y
pone junto a él el estilo del botón.

## Texto en la imagen

Pon entre comillas las palabras que quieras en el aspecto: *un zorro que lee un mapa, con la palabra
“ESCAPADA”*. FolderSkin rotula exactamente lo que va entre comillas, deletreado letra por letra para
los modelos que siguen instrucciones, una sola vez, con las letras propias del estilo y colocado
según la forma: de lado a lado por el centro del panel delantero en una carpeta, y sobre el objeto o
debajo de él en un icono libre. Sin comillas, el prompt pide que no haya ningún texto. Que sean una
o dos palabras cortas, porque un texto largo sigue saliendo ilegible.

## Tus prompts

**Guardar como prompt**, en el menú /, guarda lo que hay en el cuadro y su estilo con el nombre que
le pongas. Desde entonces aparece en **Tus prompts**: elígelo y el cuadro y el estilo vuelven a
quedar como estaban. Si escribes un nombre que ya está guardado, FolderSkin te lo dice y reemplaza
ese prompt. La x junto a uno de los tuyos lo quita, y **Deshacer** lo recupera durante unos
segundos.

Los prompts guardados se conservan en `skills.json`, junto a la carpeta `skins`
([ARCHITECTURE.md](../ARCHITECTURE.md#saved-skins) dice dónde está, en inglés), como skills con el
formato `folderskin.skill/1` que describe `crates/folderskin-ai/src/skill.rs`. Una skill separa lo
que muestra una imagen (`idea`) de cómo se ve (`base_style`, o su propio `treatment`, `palette` y
`light`), así que un mismo estilo guardado puede ir con cualquier idea. Cada skill se comprueba antes
de escribirse: necesita un nombre de hasta 60 caracteres y algo que guardar, su estilo tiene que ser
uno de los de FolderSkin, su propio tratamiento tiene de 8 a 60 palabras y le dice al modelo qué
hacer en lugar de qué no hacer, y la skill entera cabe en 4 KB. Un prompt que nombra a alguien como
estilo (“in the style of” o “by” seguidos de un nombre) se guarda después de un aviso, porque
describir la técnica funciona mejor.

## Imágenes de referencia

Una imagen que añades a un prompt se usa como **Motivo**, salvo que indiques otra cosa. Haz clic en
su miniatura para usarla como **Estilo** (un estilo que imitar, sin tomar nada de lo que muestra) o
como **Colores** (su paleta y nada más). Las imágenes llegan al modelo en ese orden, después de la
plantilla, y el prompt nombra cada una por su número y su función.

## Qué se envía a cada proveedor

En `crates/folderskin-ai/src/request.rs`, cada solicitud se construye exactamente como la documenta
la referencia de la API de su proveedor, y se contrasta con el SDK del propio proveedor cuando lo
tiene. Las pruebas de ese archivo comprueban cada solicitud campo por campo. Una prueba de
`crates/folderskin-ai/tests/providers.rs` también envía cada solicitud a un servidor simulado en tu
equipo que responde como dice la documentación del proveedor, así que el método, la dirección, la
cabecera que lleva la clave, el tipo de contenido y cada campo se comprueban tal como viajan por la
red.

Las opciones van en los parámetros propios de cada proveedor, nunca en el texto del prompt. El tamaño
es el de la forma (1024 × 960 para la ilustración de la carpeta de Mac, 1024 × 800 para la de
Windows, una carpeta entera en sus propias proporciones y un icono libre en cuadrado): se envía
exacto cuando un proveedor acepta cualquier tamaño, y si no, como el tamaño o la relación de aspecto
más cercanos que ofrezca:

| Proveedor | Solicitud | Tamaño | También se envía |
|---|---|---|---|
| OpenAI | JSON a `images/generations`, o un formulario con cada imagen como `image[]` a `images/edits` | exacto, en una cuadrícula de 16 píxeles (1024 × 960) | `quality`: high para Flare, max para Sunburst, medium para GPT Image 2, para saber el precio de antemano |
| xAI Grok | solo JSON, con las imágenes incluidas como URL de datos (`image`, o `images` si son varias) | el `aspect_ratio` más cercano, o el encuadre de la propia plantilla | |
| Google Gemini | JSON a `generateContent` | `generationConfig.imageConfig`: 1K, con el `aspectRatio` más cercano o el encuadre de la plantilla | `responseModalities: ["IMAGE"]` |
| Black Forest Labs | JSON, con las imágenes como `input_image`, `input_image_2`, etcétera | exacto, dentro de un megapíxel (FLUX.2 cobra cada megapíxel empezado) | la reescritura del prompt desactivada (`disable_pup`, o `prompt_upsampling: false` en flex) |
| Recraft | JSON | el tamaño más cercano de la lista de V4.1 | salida en PNG, y el color de recorte como `controls.background_color` |
| Stability AI | un formulario | el `aspect_ratio` más cercano | un prompt negativo, y un preajuste de estilo cuando el estilo tiene uno |
| Ideogram | un formulario | la más cercana de las resoluciones de 3.0 (1024 × 960) | Magic Prompt desactivado, un prompt negativo, y un tipo o un preajuste de estilo |

El prompt negativo deja fuera el texto (salvo que lo pidas), las marcas de agua y las firmas, además
de lo que el estilo elegido suele añadir. OpenAI, Gemini y Grok no tienen prompt negativo, así que su
prompt dice lo mismo con palabras.

## Modelos retirados

Una elección que se guardó cuando un modelo estaba disponible pasa al modelo que lo sustituyó, y un
aspecto conserva el nombre del modelo que lo creó.

| Antes | Ahora | Motivo |
|---|---|---|
| OpenAI GPT Image 1 | GPT Image 2 | OpenAI lo retira el 23 de octubre de 2026 |
| Gemini 2.5 Flash Image | Gemini 3.1 Flash Image | Google lo retira el 2 de octubre de 2026 |
| FLUX 1.1 Pro | FLUX.2 pro | la generación anterior, que no aceptaba imágenes |
| Recraft V3 | Recraft V4.1 | la generación anterior, cuyos prompts se cortan a los 1000 caracteres, menos de lo que ocupan las propias instrucciones de FolderSkin |

Ideogram 4.0 todavía no está disponible: reescribe todos los prompts escritos como texto, y
FolderSkin conserva la idea tal como la escribiste.

## Costo

Tu proveedor factura cada solicitud a tu propia cuenta. La vista Generar muestra el precio aproximado
del modelo antes de que hagas clic en el botón, según la página de precios del proveedor para los
parámetros que envía FolderSkin. Una carpeta entera cuesta un poco más en los proveedores que también
cobran por las imágenes que reciben (xAI, Black Forest Labs). Debajo de cada imagen, Detalles muestra
lo que el proveedor dijo que costó la solicitud (xAI, Black Forest Labs, Recraft) o lo que consumió
(OpenAI, Gemini).

FolderSkin hace exactamente una solicitud por clic, y nunca reintenta por su cuenta. Eso también vale
cuando un proveedor termina sin una imagen, como a veces hace Gemini (`NO_IMAGE`): el chat lo indica
y ofrece **Reintentar**, y solo tu clic vuelve a enviar la solicitud.

## Compilación y compilación cruzada

La capa de proveedores usa `rustls` para TLS, cuyo backend criptográfico (`aws-lc-sys`) compila C.
Eso compila sin problemas en el runner de CI de cada plataforma, que es como se hacen las versiones
de FolderSkin. La compilación cruzada de un sistema de escritorio a otro (por ejemplo,
`cargo check --target x86_64-pc-windows-msvc` en un Mac) necesita una cadena de herramientas cruzada
de C para el destino, y si no, falla en el script de compilación de `aws-lc-sys`. El resto del
workspace se comprueba en compilación cruzada sin ella.

## Mensajes de error que puedes ver

| Mensaje | Qué pasó |
|---|---|
| “add your … API key first” | No hay ninguna clave guardada para ese proveedor |
| “that key was rejected by …” | El proveedor rechazó la clave |
| “… is rate limiting you right now” | 429: espera y vuelve a intentarlo |
| “… finished without painting a picture” | El proveedor respondió sin una imagen y sin decir por qué (el `NO_IMAGE` de Gemini): vuelve a intentarlo o reformula la idea |
| “… declined that prompt: its filter blocked the picture” | El filtro de seguridad del proveedor detuvo el prompt o la imagen, como la imagen difuminada de Stability o la comprobación de seguridad de Ideogram: reformula el prompt |
| “… said: … (error 400)” | El mensaje del propio proveedor, tal como llegó. Conviene informar de ello si nombra un campo que envió FolderSkin |
| “the model drew a scene instead of a folder on a plain backdrop” | Modo de carpeta entera sin un fondo que se pueda recortar: vuelve a intentarlo o cambia a Solo la imagen. La app guarda esa imagen como ilustración para su propia carpeta, y un icono libre como la imagen cuadrada que es, y te lo dice. |
| “the provider returned something that is not an image” | Una respuesta mal formada o que no es una imagen |

## Carpetas hechas en un asistente de chat

También puedes pintar una carpeta entera en ChatGPT, Grok o cualquier otro asistente de chat y
traerla con **Añadir tu foto**. Pide la carpeta sobre un fondo #FF00FF liso, o sobre uno
transparente. FolderSkin reconoce cualquiera de los dos y usa la imagen tal cual como icono,
recortada y ajustada, en lugar de volver a colocarla dentro de su propia carpeta. Cualquier otra
imagen se trata como ilustración para la plantilla.
[ARCHITECTURE.md](../ARCHITECTURE.md#artwork-or-a-finished-folder) tiene las reglas exactas (en
inglés), incluido por qué la foto de un objeto sobre papel magenta sigue siendo una imagen.

## Conservar un aspecto generado

Cada aspecto generado se guarda en cuanto llega, igual que una imagen importada, junto con el
proveedor, el modelo, tu prompt y la forma para la que se hizo. Después de reiniciar, está en la
galería, en Mis aspectos, y borrarlo ahí lo elimina del disco.
[ARCHITECTURE.md](../ARCHITECTURE.md#saved-skins) dice dónde están los archivos (en inglés). Si la
escritura falla (por ejemplo, con el disco lleno), el aspecto se queda durante el resto de la sesión
en lugar de perderse.

También guarda de qué se hizo, en `recipe` dentro del índice de los aspectos, para que un resultado
se pueda rastrear hasta su prompt y volver a hacerse: el prompt exactamente como se envió, el prompt
negativo en los proveedores que lo admiten, el estilo, las palabras que rotula, la función de cada
imagen y un hash de ella, la plantilla y su versión (`mac-folder/1`), el color de recorte y la
versión de la receta.

El chat conserva sus palabras y sus imágenes mientras miras otras vistas, incluida una imagen que
todavía se está creando, y cuando FolderSkin se vuelve a abrir empieza uno nuevo. Los chats
anteriores están en la lista de chats, cada uno con la forma para la que era.

Para compartir aspectos generados con todo el mundo, ponlos en un paquete de la comunidad:
etiquétalos y usa **Compartir con la comunidad** en la app, o convierte una carpeta de renders
guardados en un paquete con `folderskin-tools packs make`. [PACKS.md](PACKS.md) explica las dos
cosas, y [SKINS.md](SKINS.md) cuenta cómo queda una imagen sobre la carpeta.
