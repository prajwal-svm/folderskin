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
una clave, no se envía nada hasta que haces clic en **Generar**, y lo que se envía es tu prompt, el
tamaño que elegiste y la imagen de referencia si elegiste una (para una carpeta entera sin imagen de
referencia, la propia plantilla de carpeta en blanco de FolderSkin).

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

## Las dos formas

Esta es la elección que más importa, y no tiene que ver con la calidad.

**Solo la imagen** pide al modelo una imagen plana de 1024 × 958, y FolderSkin la coloca sobre su
propia plantilla de carpeta, exactamente igual que una foto que añades. La geometría es nuestra, así
que cada aspecto se alinea con todos los demás, en todos los tamaños de icono. Cualquier proveedor
puede hacerlo, incluidos los que no admiten transparencia. Es la opción predeterminada y la correcta
la mayoría de las veces.

**Carpeta entera** pide al modelo que dibuje la propia carpeta sobre un fondo transparente o sobre un
fondo liso que se pueda recortar, y esa imagen se convierte directamente en el icono, sin pasar por
el compositor. Renuncias a una geometría exacta al píxel y ganas una ilustración que puede tener
relieve de verdad y sobresalir por el borde superior de la carpeta.

Cuando el modelo puede partir de una imagen (OpenAI, Grok, Gemini) y no has adjuntado ninguna,
FolderSkin envía como esa imagen su propia plantilla de carpeta en blanco: nuestra carpeta, pintada
de un gris claro liso, centrada sobre un magenta liso al tamaño pedido (`compositor::blank_template`).
El prompt le pide al modelo que vuelva a pintar exactamente esa carpeta, conservando su contorno, su
pestaña, su franja de papel, su tamaño y su posición, y que deje el magenta liso. El resultado
conserva la silueta de FolderSkin en lugar de la carpeta que el modelo se habría inventado. Como la
plantilla está sobre magenta, esa generación siempre sigue la vía del recorte que se explica abajo,
incluso con un modelo que podría devolver transparencia. Una imagen de referencia que adjuntes tú se
usa como ilustración, igual que antes.

## Cómo se gestiona la transparencia

Los modelos se dividen en dos grupos, y FolderSkin elige la vía correcta para el modelo que
elegiste:

- **Alfa nativo.** La solicitud pide un fondo transparente y el PNG que llega ya lo tiene. FolderSkin
  solo recorta el margen transparente.
- **Sin alfa.** El prompt pide la carpeta sola sobre un magenta liso, `#FF00FF`. FolderSkin quita
  después ese color, elimina el magenta que se filtró en el borde suave (el paso que evita que un
  recorte parezca tener un halo rosa) y recorta. Se usa el magenta porque casi nunca aparece en el
  arte de una carpeta, y porque un fondo que falta se puede detectar: si el borde no es magenta, el
  modelo ignoró la instrucción, y FolderSkin lo dice en lugar de aplicar un icono roto.

El código de recorte está en `crates/folderskin-core/src/matte.rs` y tiene pruebas unitarias,
incluido el caso de un motivo realmente rosa sobre un fondo magenta.

## Los prompts

`crates/folderskin-ai/src/prompts.rs` compone el prompt a partir de tus palabras más un contrato. Las
partes que hacen el trabajo son de estructura, no de estilo:

- **Los prompts de ilustración** prohíben dibujar una carpeta, un icono, un dispositivo o una
  maqueta, y reservan el octavo superior y un borde del 6 % como espacio muerto, porque la plantilla
  recorta o curva esas zonas.
- **Los prompts de carpeta entera** fijan la construcción: exactamente tres partes, una pestaña, un
  borde de papel visible, un panel delantero y una instrucción explícita de no añadir capas. Sin esa
  frase, los modelos producen sin falta carpetas apiladas y pestañas dobles.
- **Los prompts de plantilla** (`compose_on_template`) acompañan a la plantilla en blanco: la imagen
  adjunta es la carpeta exacta que hay que volver a pintar, su forma y su encuadre se quedan como
  están, la idea se pinta sobre los paneles trasero y delantero, y el magenta se queda liso.
- **Todos** terminan con un contrato de salida estricto que fija el tamaño en píxeles, el aislamiento
  del motivo y el color de recorte o el fondo transparente.

Puedes editar estas plantillas de prompt. Son constantes de texto normales de Rust, con pruebas que
comprueban que están las frases clave.

## Costo

Tu proveedor factura cada solicitud a tu propia cuenta. La vista Generar muestra el precio aproximado
del modelo antes de que hagas clic en el botón. FolderSkin hace exactamente una solicitud por clic, y
nunca reintenta por su cuenta.

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
| “that key was rejected by …” | El proveedor respondió 401 o 403 |
| “… is rate limiting you right now” | 429: espera y vuelve a intentarlo |
| “the model drew a scene instead of a folder on a plain backdrop” | Modo de carpeta entera sin un fondo que se pueda recortar: vuelve a intentarlo o cambia a Solo la imagen |
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
proveedor, el modelo y tu prompt. Después de reiniciar, está en la galería, en Mis aspectos, y
borrarlo ahí lo elimina del disco. [ARCHITECTURE.md](../ARCHITECTURE.md#saved-skins) dice dónde
están los archivos (en inglés). Si la escritura falla (por ejemplo, con el disco lleno), el aspecto
se queda durante el resto de la sesión en lugar de perderse.

Para compartir aspectos generados con todo el mundo, ponlos en un paquete de la comunidad:
etiquétalos y usa **Compartir con la comunidad** en la app, o convierte una carpeta de renders
guardados en un paquete con `folderskin-tools packs make`. [PACKS.md](PACKS.md) explica las dos
cosas, y [SKINS.md](SKINS.md) cuenta cómo queda una imagen sobre la carpeta.
