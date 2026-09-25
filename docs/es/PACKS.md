# Aspectos y paquetes de la comunidad

Cualquiera puede compartir aspectos gratis con todas las personas que usan FolderSkin. Un conjunto
de aspectos compartido es un **paquete**, y un aspecto suelto es un paquete de uno. Los paquetes
viven en su propio repositorio,
[folderskin-community](https://github.com/prajwal-svm/folderskin-community), dentro de `packs/`, y
añadir uno no requiere ninguna cuenta. FolderSkin no trae aspectos propios: todos salen de los
paquetes, de tus propias imágenes o de los resultados de la IA.

## Añadir un paquete

La primera vez que se abre, FolderSkin te ofrece paquetes para empezar tu biblioteca. Después, abre
**Comunidad** en la app. Los filtros de arriba son las etiquetas de los paquetes. **Añadir** pone los
aspectos de un paquete en tu biblioteca, con las etiquetas del paquete, y el menú ⋯ de cada aspecto
indica de qué paquete viene y quién lo compartió. **Quitar** vuelve a sacar el paquete entero. Las
carpetas que ya usan uno de sus aspectos conservan su icono, porque el icono vive en la propia
carpeta.

**Añadir desde una carpeta** hace lo mismo con la carpeta de un paquete que tengas en tu equipo, y
así es también como pruebas un paquete antes de compartirlo.

La galería de [folderskin.app](https://folderskin.app/es/community/) tiene un botón **Instalar** en
cada paquete. Abre FolderSkin en ese paquete, dentro de Comunidad, y lo añade, igual que haría su
botón **Añadir** ([el enlace de instalación](#el-enlace-de-instalación), más abajo). Los paquetes
marcados como **Oficial** son los que avala el mantenedor.

Un paquete se añade entero o no se añade: primero se descarga y se comprueba cada imagen, y después
se guardan todas de una vez, así que una conexión que se corta o un disco lleno nunca dejan medio
paquete en tu biblioteca. Sus aspectos aparecen en el orden del propio paquete.

## Compartir los tuyos

Compartes desde la app, y FolderSkin envía el paquete a su servicio de la comunidad, en
`community.folderskin.app`, donde el mantenedor lo revisa. No necesitas cuenta de GitHub. FolderSkin
0.1.6 y las versiones anteriores también podían abrir un pull request en GitHub por ti. La 0.1.7
elimina esa opción.

1. Etiqueta los aspectos que quieras compartir (⋯ → Etiquetas). Para compartir un solo aspecto, usa
   ⋯ → **Compartir con la comunidad**. Para compartir varios, usa **Comunidad → Compartir tus
   aspectos** y elige una etiqueta.
2. Indica el nombre del paquete, sus etiquetas y una licencia, di de dónde salieron las imágenes y
   marca la casilla que confirma que puedes compartirlas.
3. La primera vez, FolderSkin verifica este equipo en tu navegador, con el nombre al que se
   atribuirán tus paquetes. Solo lo pide una vez por equipo.
4. Envíalo. FolderSkin comprueba que el paquete cumpla el contrato de abajo antes de que nada salga
   de tu equipo. **Tus envíos** muestra cada paquete que enviaste y, si alguno se rechaza, por qué.

Cada imagen se comparte sin pérdida, así que un paquete se ve exactamente como lo hiciste, incluido
el borde transparente de una carpeta terminada. FolderSkin convierte cada imagen a WebP sin pérdida
antes de enviarla, lo que lleva unos segundos por imagen, y va mostrando cuántas están
listas. Una imagen demasiado detallada para caber en 1.5 MB a 1024 px pasa a 896 px y luego a
768 px, siempre sin pérdida, y FolderSkin te dice cuáles. Las imágenes de un paquete suman 64 MB como
máximo. Uno más grande se rechaza, con la sugerencia de dividirlo en dos paquetes.

Una persona revisa cada paquete antes de que nadie más pueda verlo. Una vez aprobado, se publica
solo, en unos 15 minutos
([De la aprobación a la publicación](#de-la-aprobación-a-la-publicación), más abajo). Muchos
paquetes pueden compartir nombre: el que elijas es el que ven todos, y el paquete recibe su propio
identificador ([Identificadores de paquete](#identificadores-de-paquete)).

Un paquete también se puede proponer a mano, con un pull request a
[folderskin-community](https://github.com/prajwal-svm/folderskin-community) que añade una carpeta
dentro de `packs/`. Crea la carpeta con `packs make` ([Crear un paquete a partir de
imágenes](#crear-un-paquete-a-partir-de-imágenes)), que le da un identificador generado, y el pull
request ejecuta las mismas comprobaciones que la app. **Guardar en una carpeta**, en la app, escribe
una carpeta de paquete que cumple todas las reglas de abajo.

## Identificadores de paquete

Cada paquete tiene un identificador, que es el nombre de su carpeta y forma parte de cada enlace a
él. El identificador se crea una sola vez, al crear el paquete, a partir de su nombre y seis
caracteres aleatorios: un paquete llamado Classic Art recibe un identificador como
`classic-art-k7q2mx`. Los nombres se pueden repetir libremente (cien paquetes pueden llamarse
Classic Art) y solo el identificador tiene que ser único. `packs make` crea los identificadores de
los paquetes hechos a mano, y el servicio de la comunidad los de los paquetes compartidos desde la
app. Un identificador ya no cambia nunca, ni siquiera cuando cambia el nombre del paquete.

La parte aleatoria son seis caracteres de la `a` a la `z` y del `2` al `7`, sacados de la fuente
aleatoria segura del sistema. Un identificador nuevo nunca coincide con el nombre de una carpeta de
`packs/` ni con un identificador antiguo de `moved.json`.

### moved.json

Los paquetes creados antes de que se generaran los identificadores tenían identificadores sacados
solo de su nombre, como `classic-art`. Recibieron identificadores generados con `packs rename`, y
`moved.json`, junto a `packs/`, registra cada identificador antiguo y el identificador que tiene
ahora ese paquete:

```json
{ "version": 1, "moved": { "classic-art": "classic-art-k7q2mx" } }
```

- Cada identificador antiguo es un identificador de paquete que ninguna carpeta de `packs/` tiene, y
  nunca se asigna a un paquete nuevo.
- Cada identificador nuevo corresponde a un paquete de `packs/`. Nunca lleva a otro identificador
  antiguo: renombrar de nuevo un paquete hace que todo lo que llevaba a él apunte a su identificador
  más reciente.
- `packs index` y `packs catalog` copian este mapa en `index.json` y `head.json` como `"moved"`, para
  que la app, el sitio web y el servicio de la comunidad puedan seguir un paquete desde su
  identificador antiguo. Desde la 0.1.7, la app pasa al identificador nuevo los paquetes que
  añadiste con uno antiguo, y un enlace de instalación con un identificador antiguo sigue
  encontrando su paquete.
- Un paquete renombrado conserva la fecha de su primera publicación: `packs index` lo fecha por el
  commit más antiguo que añadió cualquiera de sus identificadores.
- Nada de esto va en `pack.json`. No acepta ningún campo que el contrato no nombre, y las versiones
  0.1.4 a 0.1.6 de la app rechazarían el paquete.

### packs rename

```sh
cargo run -p folderskin-tools -- packs rename --dir ../folderskin-community --all
cargo run -p folderskin-tools -- packs rename --dir ../folderskin-community classic-art
cargo run -p folderskin-tools -- packs rename --dir ../folderskin-community classic-art --to classic-art-k7q2mx
```

Este comando da identificadores generados a los paquetes, en un clon de git de
folderskin-community. Cada carpeta se mueve con `git mv`, así que su historial va con ella.
`featured.json` y `official.json` se reescriben con los identificadores nuevos, en el mismo orden, y
`moved.json` registra cada movimiento (se crea si no existe). Todo queda en el área de preparación,
listo para el commit.

`--all` renombra cada paquete cuyo identificador no sea generado, y no toca los demás. Solo se fija
en la forma: un identificador antiguo cuya última palabra tenga justo seis letras, como
`data-structures-in-bricks`, parece generado, así que renombra ese paquete indicando su
identificador. Si lo indicas por separado, un paquete pasa a un identificador generado nuevo, sea
cual sea el actual, o al de `--to`, que tiene que ser un identificador generado que ningún paquete
tenga ni haya tenido antes. Volver a ejecutarlo no cambia nada: un paquete que ya se movió aparece en
`moved.json`, y el comando indica que ya se movió.

## De la aprobación a la publicación

Aprobar un paquete lo publica. Nadie copia archivos a mano.

1. Cuando el mantenedor aprueba un paquete, el servicio de la comunidad le asigna su identificador.
   Si el servicio tiene un token de GitHub (`GITHUB_DISPATCH_TOKEN`), además lanza enseguida el
   flujo de trabajo Packs de folderskin-community, con un repository dispatch `pack-approved`.
2. Sin token, el flujo de trabajo encuentra el paquete por su cuenta. Cada 15 minutos le pregunta a
   `https://community.folderskin.app/v1/exports/pending` cuántos paquetes aprobados están
   esperando, lo que tarda segundos, y solo hace el resto si hay alguno. También se ejecuta cada día
   y cada semana, haya o no algo esperando, y cada vez que el mantenedor lo lanza a mano.
3. `community pull --no-done` escribe cada paquete aprobado en `packs/` y compara el tamaño y el
   SHA-256 de cada archivo con lo que el servicio registró al subirlo. El identificador que dio el
   servicio tiene que ser generado, y una carpeta que ya existe nunca se sobrescribe ni se numera.
   Las carpetas terminadas del paquete reciben una sola forma ([Una sola forma para las carpetas de
   un paquete](#una-sola-forma-para-las-carpetas-de-un-paquete)) antes de comprobarlo. Una carpeta
   demasiado alejada de esa forma se queda tal cual, y el registro de la ejecución lo avisa con una
   advertencia.
4. `packs check` comprueba cada paquete, igual que en un pull request.
5. github-actions[bot] hace un commit de cada paquete con el mensaje `Add the <name> pack` y lo sube
   a `main`.
6. Solo entonces `community done` le dice al servicio que el paquete está publicado. Si una
   comprobación o una subida falla, el paquete sigue esperando en el servicio, y la siguiente
   ejecución lo vuelve a intentar.
7. La misma ejecución reconstruye `index.json`, las vistas previas y `v2/`, copia `v2/` al espejo
   ([más abajo](#el-espejo)) y hace commit de todo. Un push hecho con el token del propio flujo de
   trabajo no lanza ningún otro flujo de trabajo, y por eso todo ocurre en una sola ejecución.

Así que un paquete se publica unos 15 minutos después de la aprobación, o en pocos minutos cuando el
propio servicio lanza el flujo de trabajo. Una ejecución que falla no publica nada, y GitHub le avisa
al mantenedor por correo de que el flujo de trabajo falló. GitHub puede lanzar tarde una ejecución
programada cuando está saturado, y desactiva la programación en un repositorio sin actividad durante
60 días. El dispatch no depende de ninguna de las dos cosas.

El flujo de trabajo necesita la clave de firma del mantenedor, el archivo entero que escribió
`community keygen`, como secreto del repositorio `FOLDERSKIN_ADMIN_KEY`. Sin ella, los paquetes
aprobados esperan en el servicio, y la ejecución lo indica. La variable del repositorio
`REQUIRE_GENERATED_IDS`, con el valor `true`, hace que cada comprobación rechace un paquete sin
identificador generado.

Traer los paquetes a mano sigue funcionando. `community pull` escribe los paquetes y avisa al
servicio enseguida, ya que quien lo ejecuta hace commit de lo que escribió.
`--no-done --pulled pulled.json` retiene ese aviso hasta `community done --from pulled.json`, que es
como lo ejecuta el flujo de trabajo. Avisar dos veces al servicio no hace ningún daño.

### El espejo

La app puede leer todo el árbol desde `https://packs.folderskin.app`, un bucket de Cloudflare R2 que
contiene el mismo `v2/` que el repositorio. El servicio de la comunidad es quien escribe en el
bucket, así que el flujo de trabajo no necesita un token de Cloudflare propio: `community mirror`
envía cada archivo a través del servicio, firmado con la clave del mantenedor.

```sh
cargo run -p folderskin-tools -- community mirror --tree ../folderskin-community \
  --public https://packs.folderskin.app --api https://community.folderskin.app --key ~/folderskin-admin.key
```

El comando le pregunta al espejo por cada archivo con una petición HEAD, y deja en paz los que ya
sirve con el mismo tamaño: cada nombre, salvo `head.json`, es el hash de su contenido. Los demás se
suben con `PUT /v1/admin/tree/<path>`, cada uno con su SHA-256 en `X-Content-SHA256` para que el
bucket lo compruebe. `head.json` va al final, y solo cuando todos los demás archivos están en su
sitio, así que el espejo nunca nombra un catálogo que no tiene. Una petición que puede salir bien
sola (sin respuesta, un 5xx o un 429) se intenta cinco veces en total, con una espera que se duplica
a partir de un segundo, y se respeta un `Retry-After` de hasta 30 segundos. El flujo de trabajo lo
ejecuta antes de hacer commit, así que el `head.json` de GitHub solo cambia cuando el espejo ya tiene
todo lo que nombra. La variable del repositorio `COMMUNITY_MIRROR_URL` activa el espejo, y
`head.json` solo lo incluye mientras está activo.

## El contrato

Un paquete es una carpeta:

```
packs/night-prints-h4x2qe/
  pack.json
  koi.webp
  fox-in-the-rain.webp
```

`pack.json`:

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

| campo | regla |
|---|---|
| `version` | `1` |
| `name` | de 1 a 40 caracteres |
| `author` | tu nombre de usuario de GitHub |
| `license` | `CC0-1.0`, `CC-BY-4.0` o `MIT` |
| `tags` | de 1 a 5 etiquetas. Todos los aspectos del paquete las reciben, y la primera identifica el paquete en los filtros de todo el mundo |
| `skins` | de 1 a 50 entradas |
| `skins[].file` | una imagen de la carpeta |
| `skins[].name` | de 1 a 60 caracteres |
| `skins[].tags` | opcional, hasta 3 más para ese aspecto |

No se permite ningún otro campo, así que una errata como `"tag"` hace fallar la comprobación en lugar
de pasar desapercibida.

### Límites

| | límite |
|---|---|
| aspectos por paquete | de 1 a 50 |
| cada imagen | sin pérdida: PNG, o WebP sin pérdida. 1.5 MB como máximo |
| todas las imágenes de un paquete | 64 MB como máximo |
| lados de la imagen | de 256 a 1024 px |
| nombres de archivo | letras, dígitos, `.`, `-` y `_`, terminados en `.png` o `.webp` |
| identificador, el nombre de la carpeta | un nombre y seis caracteres aleatorios, como `night-prints-h4x2qe` ([Identificadores de paquete](#identificadores-de-paquete)): letras minúsculas y dígitos, en palabras unidas por guiones simples, 40 caracteres como máximo |
| etiquetas | letras minúsculas, dígitos, espacios y guiones, 24 caracteres como máximo |
| `pack.json` | 64 KB como máximo |

Por qué 50 y 64 MB: un paquete es un conjunto temático, y todo el que lo añade lo descarga entero.
Cincuenta aspectos siguen siendo rápidos de revisar, y en 64 MB caben los cincuenta a 1.3 MB por
imagen, o cuarenta y dos al tamaño máximo.

Los paquetes publicados antes de FolderSkin 0.1.7 tenían un límite de 2 MB por imagen en PNG, JPEG o
cualquier WebP, y la app los sigue leyendo. Si se rehacen con `packs make`, cumplen las reglas de
arriba. `packs check` exige esas reglas con `--require-lossless` ([Comprobar un paquete por tu
cuenta](#comprobar-un-paquete-por-tu-cuenta)), y el servicio de la comunidad no acepta nada más.

### Imágenes

Cada imagen es de uno de dos tipos, que se distinguen igual que al soltar una imagen en la ventana:

- **Una carpeta terminada**, sobre un fondo transparente o sobre un magenta liso `#FF00FF` que
  FolderSkin elimina. Se convierte en el icono tal como está dibujada.
- **Todo lo demás** se coloca sobre la carpeta de FolderSkin. [SKINS.md](SKINS.md) muestra dónde
  recorta la carpeta una imagen, para que el motivo principal se conserve.

1024 px es el icono más grande que dibuja cualquiera de los tres sistemas, así que una imagen más
grande no aporta nada.

Todas las imágenes son sin pérdida, así que un paquete se ve exactamente como se hizo: sin bloques en
los degradados, sin halos alrededor de las letras y con el borde de una carpeta terminada tan limpio
como se dibujó. Un WebP sin pérdida ocupa más o menos un tercio menos que el mismo PNG, y por eso la
app y `packs make` escriben WebP. Una imagen detallada de 1024 px ocupa entre 0.6 y 1.5 MB, la
mayoría unos 800 KB. La que no cabe en 1.5 MB pasa a 896 px y luego a 768 px, siempre sin pérdida,
en lugar de difuminarse para caber.

### Una sola forma para las carpetas de un paquete

FolderSkin encaja la imagen entera de una carpeta terminada en el icono. Las carpetas hechas de una
en una salen recortadas al ras, y no hay dos renders con las mismas proporciones exactas: en un
paquete iban de 1.03 a 1.30 veces más anchas que altas. Una al lado de otra en el Finder, las más
achatadas parecían más pequeñas que las más altas. Por eso las carpetas terminadas de un paquete
comparten una sola forma:

- **La forma del paquete** es la mediana de las formas de sus carpetas. Una carpeta se mide por la
  parte de su imagen que es opaca en más de la mitad, ancho ÷ alto, así que un borde suave o una
  sombra tenue no cuentan.
- **Cada carpeta se vuelve a dibujar exactamente con esa forma.** Se recorta a su carpeta y se
  redimensiona a 962 px de ancho, que es el ancho de la propia carpeta de FolderSkin en su plantilla
  de 1024 px (`folderskin-tools template`), y al alto que marque la forma. Después se coloca donde
  está la carpeta de la plantilla, con el mismo borde izquierdo y apoyada en la misma línea base, en
  una imagen transparente de 1024 × 1024, y se guarda como WebP sin pérdida. Un PNG se convierte en
  un `.webp` con el mismo nombre, y `pack.json` se actualiza en consecuencia.
- **Una carpeta se deforma un 8 % como máximo.** Nadie nota tanto. Una carpeta que necesitaría más
  es un *caso atípico*: estiradas hasta ese punto, las letras y las caras se ven aplastadas, así que
  nunca se deforma. Lo que pase con ella depende del comando, y lo decide una persona.
- **Las ilustraciones no se tocan.** FolderSkin las coloca sobre su propia carpeta, así que no
  tienen forma propia que corregir. Tampoco se toca un paquete con una sola carpeta terminada.

`packs make` hace esto con cada paquete que crea, `community pull` con cada paquete que trae y
`packs normalize` con los paquetes que ya están en `packs/`. Hacerlo dos veces no cambia nada: una
carpeta que ya tiene la forma de su paquete, en su sitio, nunca se vuelve a dibujar, y un archivo
que ya contiene lo que recibiría no se reescribe.

| comando | un caso atípico |
|---|---|
| `packs make` | se deja fuera del paquete y se muestra en la lista. `--keep-outliers` lo conserva tal cual |
| `packs normalize` | se muestra en la lista y se deja tal cual. `--drop-outliers` lo quita de `pack.json` y borra su imagen |
| `community pull` | se conserva tal cual, con una advertencia en la salida que muestra el registro del flujo de trabajo Packs. Un aspecto que alguien compartió nunca se descarta sin que lo decida una persona |

```sh
cargo run -p folderskin-tools -- packs normalize --dir ../folderskin-community
cargo run -p folderskin-tools -- packs normalize --dir ../folderskin-community classic-art-5rxas2 --tolerance 0.3
cargo run -p folderskin-tools -- packs normalize --dir ../folderskin-community dreamscapes-ppfia6 --drop-outliers
```

`packs normalize` recorre cada paquete, o los que se le indiquen. De cada uno dice la forma y lo que
volvió a dibujar, y nombra cada caso atípico con lo que se desvía. Un paquete que no pasa
`packs check` no se toca hasta que lo pase. `--tolerance` cambia el 8 %, para un paquete cuyos casos
atípicos deban tomar su forma de todos modos, como la Mona Lisa en Classic Art. `--drop-outliers` es
para el mantenedor: nada más quita nunca un aspecto. `packs check --require-one-shape` rechaza un
paquete cuyas carpetas se diferencian en más de un 1 %
([Comprobar un paquete por tu cuenta](#comprobar-un-paquete-por-tu-cuenta)). Las carpetas
redibujadas con una sola forma nunca llegan a esa diferencia.

### Licencias

Los aspectos compartidos usan Creative Commons o MIT:

- `CC0-1.0`: cualquiera puede usarlos para lo que sea. Es la opción predeterminada, porque la
  mayoría de los aspectos se hacen con IA y CC0 es la que menos derechos reclama sobre ellos.
- `CC-BY-4.0`: cualquiera puede usarlos, dándote el crédito.
- `MIT`: cualquiera puede usarlos, y tu nombre los acompaña.

Comparte solo imágenes que hayas hecho tú o que tengas permiso para compartir.

## Crear un paquete a partir de imágenes

`packs make` convierte una carpeta de imágenes, como renders guardados de un modelo de imagen, en un
paquete que ya pasa las comprobaciones, dentro de `packs/` en un clon de folderskin-community. Los
comandos de abajo se ejecutan desde este repositorio, con folderskin-community clonado al lado:

```sh
cargo run -p folderskin-tools -- packs make ~/Downloads/3d-renders --dir ../folderskin-community \
  --name "3D" --tags 3d,glossy --author your-github-name --preview /tmp/3d.png
```

El paquete recibe su propio identificador, su nombre y seis caracteres aleatorios, como `3d-k7q2mx`,
y ese es el nombre de su carpeta. El informe lo indica. El identificador nunca coincide con el de una
carpeta de `packs/` ni con un identificador antiguo de `moved.json`.

Cada imagen se clasifica como lo hace la app cuando añades una. Una carpeta terminada, pintada sobre
magenta como pide el prompt para chat de [PROMPTS.md](PROMPTS.md), o sobre una transparencia real,
se recorta y se convierte en el propio icono. Todo lo demás es una ilustración para la carpeta de
FolderSkin. Cada imagen se reduce a 1024 px y se guarda como WebP sin pérdida, con el codificador
integrado que usa la app para compartir paquetes, así que no hay nada que instalar. Si una sigue
pasando de 1.5 MB, pasa a 896 px y luego a 768 px, y el informe lo indica. `--max-kb` limita las
imágenes a menos de 1.5 MB. Las imágenes que suman más de 64 MB se rechazan, con la sugerencia de
dividirlas en dos paquetes. El ajuste más exhaustivo de libwebp tarda varios segundos por imagen,
así que se procesan en todos los núcleos a la vez. [SKINS.md](SKINS.md#imágenes-para-un-paquete)
cuenta más sobre los formatos. El informe dice qué camino siguió cada imagen. Los aspectos toman el
nombre de sus archivos, así que nombra primero los archivos o corrige después los nombres en
`pack.json`. `--preview` dibuja cada aspecto como su carpeta en un solo PNG para revisarlo todo.

A partir de dos carpetas terminadas, reciben una sola forma ([Una sola forma para las carpetas de un
paquete](#una-sola-forma-para-las-carpetas-de-un-paquete)), y el informe dice cuáles se volvieron a
dibujar. Una carpeta que se desvía más de un 8 % de la forma de las demás se deja fuera, y el
informe dice cuánto se desvía. `--keep-outliers` la conserva tal cual. Un paquete hecho sin
conservar casos atípicos pasa `packs check --require-one-shape`.

`--id` rehace un paquete que ya existe a partir de imágenes nuevas: `--id 3d-k7q2mx` reemplaza todo
lo que hay en `packs/3d-k7q2mx/`, y el paquete conserva su identificador, así que todos los que lo
añadieron reciben la nueva versión como una actualización. La carpeta nueva se crea y se comprueba
primero aparte, y solo ocupa el lugar de la antigua si pasa las comprobaciones. Sin `--id`,
`packs make` siempre crea un paquete nuevo.

Los modelos de imagen a los que se les pide `#FF00FF` a menudo pintan en su lugar un frambuesa o un
rosa intenso uniformes (Grok lo hizo con el paquete Classic Art). `--flat-backdrop` elimina un fondo
liso de cualquier color: mide el fondo de cada imagen, quita solo lo que llega al borde (así un manto
rojo dentro de la carpeta se queda), se lleva con él una sombra paralela suave y le da al borde los
colores de la pintura en lugar de un contorno rosa. Sobre un fondo gris o negro liso se ciñe al ruido
propio de ese fondo y nunca avanza hacia arriba, así que un abrigo oscuro o una línea de tinta que
toca el borde de la carpeta no se confunden con el fondo. Revisa después la hoja de `--preview`
(está dibujada sobre un gris claro, para que se vea cualquier hueco). Una imagen sin fondo liso sigue
saliendo como ilustración.

Para ver una imagen como la mostrará la app, `render` la dibuja como su carpeta:

```sh
cargo run -p folderskin-tools -- render ../folderskin-community/packs/3d-k7q2mx/glass.webp --out /tmp/glass.png --size 512
```

## Comprobar un paquete por tu cuenta

Desde este repositorio, con folderskin-community clonado al lado:

```
cargo run -p folderskin-tools -- packs check --dir ../folderskin-community
```

Comprueba cada carpeta de `packs/` con las reglas que usa la app, y describe cada problema en una
frase. Si lo ejecutas dentro de un clon de folderskin-community, puedes omitir `--dir`: las
herramientas buscan en la carpeta actual por defecto. Limita cada imagen a los 2 MB con los que se
hicieron los paquetes publicados antes de la 0.1.7, y las imágenes de cada paquete a 64 MB en total.
`--require-lossless` aplica a cada imagen las reglas de los paquetes nuevos: PNG o WebP sin pérdida,
1.5 MB como máximo, que es lo que acepta el servicio de la comunidad y lo que escribe `packs make`.
Está desactivado salvo que se pida, mientras se rehacen los paquetes anteriores. `--max-kb` impone
un límite más bajo.

También compara los identificadores entre sí: dos carpetas cuyos nombres solo se diferencian en las
mayúsculas son una sola carpeta en macOS y Windows, y un paquete no puede tomar un identificador
antiguo de `moved.json`, que tiene que cumplir sus propias reglas ([moved.json](#movedjson)). Los
nombres nunca se comparan, ya que se pueden repetir. `--require-generated-ids` rechaza cualquier
paquete cuyo identificador no sea generado. Está desactivado salvo que se pida, y el flujo de
trabajo de folderskin-community lo pide cuando su variable `REQUIRE_GENERATED_IDS` es `true`.

`--require-one-shape` rechaza un paquete cuyas carpetas terminadas no tengan una sola forma, es
decir, con dos carpetas que se diferencien en más de un 1 % en ancho ÷ alto. Las carpetas
redibujadas con una sola forma siempre quedan dentro de ese margen, así que detecta un paquete que
nunca recibió una forma común y un caso atípico que alguien conservó. El mensaje nombra las dos
carpetas más alejadas y dice qué ejecutar ([Una sola forma para las carpetas de un
paquete](#una-sola-forma-para-las-carpetas-de-un-paquete)). Está desactivado salvo que se pida, y
pensado para el flujo de trabajo de folderskin-community.

## Cómo lee la app los paquetes

Comunidad tiene una vista de **Lista** y otra de **Galería**, y **Ver** abre cualquier paquete: cada
aspecto dibujado como la carpeta que produce, con su nombre, antes de añadir nada. **Recargar**
vuelve a leer la lista. Un paquete que añadiste y que ha cambiado desde entonces muestra
**Actualizar**, que cambia sus aspectos por la versión nueva. Las carpetas conservan sus iconos, y
una imagen favorita que esté en las dos versiones sigue siendo favorita.

- `index.json`, en folderskin-community, enumera todos los paquetes: su identificador, nombre,
  autor, licencia, etiquetas, número de aspectos y un hash de su contenido exacto (`pack.json` y
  cada imagen). La app guarda el hash junto con los aspectos que añade, y así sabe que un paquete
  tiene una actualización. Cada entrada indica también cuándo se publicó el paquete por primera vez
  (`"added"`, en segundos Unix: la hora del commit que añadió su `pack.json`, con su primer
  identificador) y, para un paquete que aparece en `official.json`, `"official": true`. `"moved"`,
  junto a los paquetes, es [moved.json](#movedjson). `folderskin-tools packs index` lo escribe,
  junto con `previews/<id>.png`, una tira con los cuatro primeros aspectos del paquete dibujados
  como carpetas. Ambos se generan en la rama `main` de folderskin-community: nunca los edites a
  mano.
- `v2/`, que escribe `packs catalog`, son los mismos paquetes en forma de catálogo en el que la app
  busca en tu equipo. Su `head.json` nombra el catálogo actual, enumera los paquetes `featured` y
  `official`, incluye `moved` y enumera los espejos que sirven el mismo árbol, como
  `https://packs.folderskin.app` ([El espejo](#el-espejo)). La app descarga cada archivo primero de
  los espejos y de GitHub si fallan, y en ambos casos comprueba cada uno con su hash. Desde la
  0.1.7, lee el propio `head.json` primero desde `https://packs.folderskin.app`.
- La app descarga las imágenes de un paquete solo cuando lo añades, de cuatro en cuatro, y muestra
  cuántas han llegado. Comprueba cada una con los límites de arriba y no guarda nada si no pasan
  todas. Después las guarda juntas, así que un paquete nunca queda añadido a medias.
- `FOLDERSKIN_COMMUNITY_URL` hace que la app apunte a otra copia de folderskin-community. Por
  ejemplo, sirve un clon con `python3 -m http.server` desde su raíz y ponle el valor
  `http://localhost:8000` para probar un paquete de principio a fin.

`index.json` y `head.json` ganan campos con el tiempo, y cada versión de la app lee los que conoce y
pasa por alto el resto. `pack.json` es lo contrario: no acepta ningún campo que el contrato no nombre,
así que nunca se le puede añadir nada. Todo lo nuevo sobre un paquete va en el índice.

## Paquetes destacados y oficiales

Hay dos listas junto a `packs/`, en la raíz de folderskin-community, y solo su mantenedor las edita.
Cada una es una lista JSON de identificadores de paquete, como
`["classic-art-k7q2mx", "colours-a2b3c4"]`:

| archivo | qué hace |
|---|---|
| `featured.json` | los paquetes que ofrece el primer inicio y que Comunidad muestra primero, en este orden |
| `official.json` | los paquetes marcados como **Oficial** en Comunidad, en la vista de un paquete y en el sitio web |

Las dos son opcionales. Cada identificador tiene que corresponder a un paquete de `packs/`, y uno que
aparezca dos veces cuenta una sola vez. `packs index` y `packs catalog` se detienen sin escribir nada
cuando alguna de las dos nombra un paquete que no está, así que un paquete renombrado o eliminado no
puede dejar un hueco. La comprobación de los pull requests ejecuta `packs catalog`, que lo detecta
antes de fusionar. `packs rename` reescribe las dos listas por su cuenta. El flujo de trabajo Packs
de folderskin-community reconstruye el índice cuando cambia un archivo de sus `paths`, así que las
dos listas deben estar ahí, junto a `packs/**`, igual que `moved.json`.

## El enlace de instalación

`folderskin://install?pack=<id>` abre FolderSkin en el paquete `<id>` dentro de Comunidad y lo
añade, exactamente como su botón **Añadir**, con el mismo progreso y el mismo mensaje al final.
Primero, la ventana pasa al frente. Si Comunidad no tiene ningún paquete con ese identificador, ni
siquiera después de volver a consultar GitHub, FolderSkin lo dice y sugiere buscarlo. Un paquete
que ya está en tu biblioteca se abre, con un aviso de que ya está ahí. Desde la 0.1.7, un enlace con
un identificador antiguo abre el paquete al que se movió ([moved.json](#movedjson)).

FolderSkin solo acepta un enlace si tiene exactamente esta forma: el esquema `folderskin`, `install`
como host (`folderskin://install?…`) o como ruta completa (`folderskin:install?…`), sin usuario,
contraseña ni puerto, y exactamente un `pack`, que tiene que ser un identificador de paquete (letras
minúsculas y dígitos, en palabras unidas por guiones simples, 40 caracteres como máximo). Cualquier
otro parámetro se pasa por alto, y cualquier otro enlace se ignora.

Los instaladores registran el esquema: el `Info.plist` de la app de macOS, los instaladores de
Windows y la entrada de escritorio de los `.deb` y `.rpm` de Linux. Una AppImage lo registra al
arrancar, ya que nada la instala. En Windows y Linux, un enlace inicia un segundo FolderSkin, que le
pasa el enlace al que ya está abierto y se cierra, así que nunca hay más de uno en marcha.

Para probarlo:

| | |
|---|---|
| macOS | Compila la app (`pnpm tauri build --bundles app`) y abre una vez `target/release/bundle/macos/FolderSkin.app`, lo que registra el esquema en macOS (una copia en `/Applications` es lo más seguro), y luego `open 'folderskin://install?pack=classic-art'`. macOS solo envía enlaces a una app empaquetada, así que `pnpm tauri dev` nunca recibe ninguno |
| Windows | Instala una compilación o ejecuta `pnpm tauri dev` (una compilación de desarrollo registra el esquema para sí misma), y luego `start "" "folderskin://install?pack=classic-art"` en un símbolo del sistema, o el mismo enlace en el cuadro Ejecutar (Windows+R) |
| Linux | Instala el `.deb` o el `.rpm`, abre la AppImage una vez o ejecuta `pnpm tauri dev`, y luego `xdg-open 'folderskin://install?pack=classic-art'` |
| vista previa en el navegador | `pnpm dev` y abre `http://localhost:14200/?install=classic-art` |

Cierra el FolderSkin instalado antes de `pnpm tauri dev` en Windows o Linux: si ya hay uno abierto,
el nuevo le pasa el relevo y se cierra. Una compilación de desarrollo que registró el esquema lo
conserva hasta que un instalador u otra compilación lo vuelva a registrar.

## Recuento de instalaciones

Cuando se añade un paquete de Comunidad, FolderSkin le comunica el identificador del paquete a su
servicio de la comunidad: `POST https://community.folderskin.app/v1/packs/<id>/installs`, sin
cuerpo. Es todo lo que envía: ninguna cuenta, ningún identificador de dispositivo, nada sobre tu
biblioteca ni tus carpetas (como todas las peticiones de FolderSkin, indica la versión de la app en
su User-Agent). Ocurre después de guardar el paquete, se rinde a los cinco segundos, y nada lo
espera ni informa de un fallo. Una compilación de desarrollo, y una que lee los paquetes de otra
copia (`FOLDERSKIN_COMMUNITY_URL`), no envían nada salvo que `FOLDERSKIN_COMMUNITY_API` indique un
servicio al que enviarlo.

El servicio cuenta una instalación una vez al día por cada red y paquete, y solo para los paquetes
del `index.json` publicado. Una instalación con un identificador antiguo cuenta para el paquete al
que se movió, así que las apps anteriores a la 0.1.7 siguen contando. Guarda un contador por paquete
y, durante el resto de ese día UTC, un hash con sal de la red de la que llegó la petición, para que
volver a añadir el mismo paquete ese día no cuente dos veces. La limpieza diaria borra esos hashes.
No se guarda ninguna dirección.

folderskin.app lee los recuentos desde `GET https://community.folderskin.app/v1/packs/installs`:
`{"version": 1, "installs": {"classic-art-k7q2mx": 42}}`, en caché durante cinco minutos.
[services/community/README.md](../../services/community/README.md) tiene los detalles (en inglés).
