# Cómo una imagen se convierte en un icono de carpeta

Cada aspecto es una imagen: una que añadiste, una que pintó un modelo de IA o una de un paquete de la
comunidad. FolderSkin la convierte en un icono de carpeta de una de dos formas, y una imagen que se
ve bien por sí sola puede quedar mal sobre una carpeta. Esta página explica cómo decide FolderSkin,
qué le hace la carpeta a una imagen y cómo comprobar una antes de compartirla en un paquete
([PACKS.md](PACKS.md)).

Si quieres que un agente haga el trabajo,
[.claude/skills/folderskin-skins/SKILL.md](../../.claude/skills/folderskin-skins/SKILL.md) guía a
Claude Code paso a paso para crear un paquete.

## Ilustración o carpeta terminada

FolderSkin se fija en lo que rodea la imagen:

- **Una carpeta terminada** está sobre una transparencia real o sobre un magenta liso `#FF00FF`. El
  magenta se elimina, se quitan los márgenes sobrantes alrededor de la carpeta y la imagen se
  convierte en el icono tal como está dibujada: escalada para caber en el icono y centrada, sin
  recortar nunca nada. Es lo que pinta un modelo de imagen con los prompts de
  [PROMPTS.md](PROMPTS.md).
- **Todo lo demás es una ilustración**: una foto, una pintura, un patrón. FolderSkin la coloca sobre
  su propia plantilla de carpeta, así que recibe la misma pestaña, el mismo papel y el mismo contorno
  que cualquier otro aspecto.

El magenta tiene que ser un `#FF00FF` liso o muy parecido, así que la foto de un producto sobre papel
magenta o una puesta de sol de un rosa intenso siguen siendo ilustraciones.
[ARCHITECTURE.md](../ARCHITECTURE.md#artwork-or-a-finished-folder) tiene las reglas exactas (en
inglés). El resto de esta página trata de las ilustraciones.

## Zonas seguras

El compositor ajusta la misma imagen para cubrir dos rectángulos del lienzo de icono de 1024 × 1024
([crates/folderskin-core/src/geometry.rs](../../crates/folderskin-core/src/geometry.rs) tiene las
constantes):

- **Panel trasero**: la parte con la pestaña, de y = 36.5 a y = 973.5. Sus proporciones son muy
  parecidas a las de una imagen de 1024 × 958, así que aquí se ve *toda la altura* y se recortan
  unos 18 px por cada lado (1.8 %).
- **Panel delantero**: la tapa del papel, de y = 160.5 a y = 973.5, con esquinas redondeadas de
  55 px de radio. Es más ancho que la imagen, así que la imagen se escala al ancho del panel y solo
  se conserva el ~87 % central de su altura: se recorta más o menos un 6 % por arriba y otro tanto
  por abajo.

Así se reparte en franjas una imagen de 1024 × 958 (una imagen con la misma forma a otro tamaño se
escala igual). Las franjas se solapan porque los dos paneles muestran partes de la misma imagen que se
solapan: el panel trasero muestra toda la altura y el delantero, el centro.

| filas (de 958) | parte de la altura | dónde acaba |
|---|---|---|
| 0 – 62 | el 6.5 % superior | dentro de la pestaña |
| 62 – 127 | el 6.7 % siguiente | la franja de panel trasero sobre el panel delantero, junto a la hoja de papel |
| 60 – 898 | el ~87 % central | el panel delantero, la parte que de verdad se mira |
| 898 – 958 | el 6.3 % inferior | recortado |

De ahí salen dos reglas:

1. **Nada importante en el 12 % superior.** Esas filas son la pestaña y la franja fina sobre el panel
   delantero. Una cara, un horizonte o un logotipo ahí arriba queda partido en dos por la hoja de
   papel.
2. **Mantén el motivo en el centro.** El panel delantero pierde unos 60 px por arriba y por abajo, y
   sus esquinas están redondeadas 55 px a 1024 px, así que los detalles de las esquinas extremas
   desaparecen de todos modos en los tamaños de icono pequeños.

El arte plano y abstracto sobrevive a todo esto sin que tengas que pensarlo. Una fotografía con un
motivo claro necesita tenerlo en la franja central. FolderSkin mantiene las ilustraciones centradas,
así que si el motivo está muy arriba o muy abajo, recorta tú la imagen antes de añadirla o de ponerla
en un paquete.

`folderskin-tools guide` dibuja estas franjas:

```sh
cargo run -p folderskin-tools -- guide --out /tmp/guide.png
```

Escribe una plantilla de 1024 × 958 marcada con la pestaña, la franja de papel, el panel delantero y
las filas que recorta el panel delantero. Ábrela en un editor de imágenes como capa encima de tu
imagen.

## Comprobar una imagen

`render` dibuja cualquier imagen como la carpeta que hace con ella la app, con el mismo código, y
dice de cuál de los dos tipos es:

```sh
cargo run -p folderskin-tools -- render ~/Pictures/koi.webp --out /tmp/koi.png --size 512
# wrote /tmp/koi.png (512×512): artwork on FolderSkin's folder
```

Mira el resultado antes de compartir la imagen. `--focus x,y` mueve el recorte de una ilustración,
una forma rápida de ver cuánto de la imagen conservaría otro recorte. La propia app siempre recorta
alrededor del centro, así que incorpora a la imagen el recorte que quieras. `render --solid RRGGBB`
dibuja la plantilla en un color liso, para ver la plantilla en sí.

## Imágenes para un paquete

Las imágenes de un paquete se comparten sin pérdida, así que un paquete se ve exactamente como lo
hiciste. [PACKS.md](PACKS.md) tiene los límites: 1024 px por lado como máximo (el icono más grande
que dibuja cualquiera de los tres sistemas), 256 como mínimo, 1.5 MB por imagen como máximo y 64 MB
para todo el paquete. La app y `packs make` reducen y codifican cada imagen por ti, como WebP sin
pérdida:

| imagen | formato | por qué |
|---|---|---|
| una carpeta terminada | WebP sin pérdida, con su transparencia | cada píxel tal como se dibujó, borde incluido, a unos dos tercios del tamaño de un PNG |
| una ilustración: fotografías, pinturas, degradados, grano | WebP sin pérdida | sin bloques ni halos. Una imagen detallada de 1024 px ocupa unos 800 KB |
| cualquiera de las dos, demasiado detallada para 1.5 MB a 1024 px | WebP sin pérdida a 896 px y luego a 768 px | más pequeña en lugar de borrosa, y la app dice cuáles |

Si codificas a mano, un PNG también sirve, igual que `cwebp -lossless -z 9`. El JPEG y el WebP con
pérdida no se aceptan para un paquete nuevo: el servicio de la comunidad los rechaza, y
`packs check --require-lossless` también. Una ilustración no tiene transparencia que conservar, ya
que la plantilla aporta la forma de la carpeta. Una ilustración de 1024 × 958 es la que menos pierde
con el recorte, pero cualquier tamaño funciona.

Las carpetas terminadas de un paquete comparten una sola forma, así que salen del mismo tamaño una al
lado de otra. `packs make` y la importación de la comunidad las vuelven a dibujar con la mediana de
sus formas, y dejan en manos de una persona una carpeta que se aleje demasiado
([PACKS.md](PACKS.md#una-sola-forma-para-las-carpetas-de-un-paquete)).

## Licencias

Comparte solo imágenes que hayas hecho tú o que tengas permiso para compartir, con una de las
licencias que puede usar un paquete ([PACKS.md](PACKS.md#licencias)). Todo lo demás, como fotos de
bancos de imágenes, fondos de pantalla, capturas de pantalla del trabajo de otra persona o
resultados de modelos cuyas condiciones no has leído, se queda en tu equipo: suéltalo en la app para
usarlo ahí, donde nunca sale de tu máquina.
