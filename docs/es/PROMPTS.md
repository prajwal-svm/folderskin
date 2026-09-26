# Pintar carpetas con Grok o ChatGPT

No necesitas una clave de API para crear tus propios aspectos. Cualquier app de chat que pueda editar
una imagen (Grok, ChatGPT) puede pintarte una carpeta si le das nuestra plantilla y el prompt de
abajo. El prompt pide un fondo magenta liso, que FolderSkin elimina antes de añadir la carpeta a
**Mis aspectos**.

## 1. Adjunta la plantilla

Descarga [`prompts/folder-template.png`](../prompts/folder-template.png) y adjúntala a tu mensaje. Es
una carpeta de FolderSkin en blanco sobre un fondo transparente. El modelo la vuelve a pintar, así
que cada resultado conserva la misma forma: la pestaña arriba a la izquierda, una hoja de papel color
crema entre los paneles y el panel delantero con la imagen.

## 2. Pega el prompt

Completa las dos líneas en mayúsculas. Deja el resto como está: cada frase está ahí para evitar un
error concreto (una carpeta inclinada, una sombra paralela, rosa que se cuela en la ilustración).

```text
Repaint the attached folder icon. Keep its exact shape: the same folder silhouette, the tab on
the top left, the thin cream paper sheet between the back and front panels, and the same size
and position in the frame. Straight-on front view, no perspective, no tilt.

Scene: DESCRIBE THE SCENE
Style: DESCRIBE THE STYLE

Paint the scene across the whole folder. The sky or background continues up into the back
panel and the tab. The main subject sits in the middle of the front panel, fully inside it.
Keep the paper sheet as a clean cream strip. Rich colour, strong light, fine texture, like a
collectible poster.

Paint everything outside the folder pure flat magenta #FF00FF: no shadow, no glow, no
gradient, no border, no other objects. Do not use magenta or hot pink inside the folder.
Square image.
```

¿Quieres texto encima? Añade una línea antes del último párrafo:
`Add the title "YOUR WORDS" in bold poster lettering on the front panel.` Que sean una o dos
palabras: un texto largo sale ilegible.

## 3. Llévala a FolderSkin

Guarda la imagen y suéltala en la ventana de FolderSkin o usa **Añadir tu foto**. El magenta se
elimina automáticamente y la carpeta aparece en **Mis aspectos**, lista para aplicar.

Si los bordes muestran un halo rosa, el modelo se alejó del magenta puro. Pídele que vuelva a pintar
el fondo exactamente en #FF00FF y vuelve a intentarlo.

## Estilos que le van bien a una carpeta

Describe uno después de `Style:`, o mezcla dos. Son los treinta estilos que ofrece el propio cuadro
del prompt de FolderSkin cuando escribes /. En **¿Sin clave de API?**, la app completa el prompt con
las palabras exactas que usa para el estilo que elijas.

**Foto y 3D**

- **Foto de estudio**: una foto fiel a la realidad con luz suave de estudio
- **Foto analógica**: una foto cinematográfica de 35 mm con luz natural y grano
- **Render de producto**: un render 3D pulido con luz de estudio y materiales reales
- **3D isométrico**: un modelo 3D limpio visto en ángulo isométrico
- **Efecto maqueta**: una escena real fotografiada como una maqueta diminuta

**Materiales y artesanía**

- **Plastilina**: plastilina hecha a mano con luz cálida
- **Vidrio**: vidrio esculpido translúcido con refracción y cáusticas
- **Neón**: tubos de neón brillantes sobre una oscuridad profunda
- **Esmalte**: esmalte cloisonné brillante con contornos dorados en relieve
- **Bordado**: bordado denso a punto de satén con hilo brillante en relieve
- **Papel recortado**: papel recortado en capas con sombras reales entre ellas
- **Vitral**: vidrio emplomado en tonos de joya iluminado desde atrás

**Pintura y dibujo**

- **Pintura al óleo**: rico óleo clásico con luz dramática
- **Acuarela**: aguadas sueltas, luminosas y transparentes
- **Gouache**: pintura mate y opaca en formas planas y armoniosas
- **Lápiz**: un dibujo a grafito detallado con toda la gama de tonos
- **Aerógrafo años 70**: degradados sedosos, cromo brillante y destellos de estrella

**Impresión**

- **Grabado en madera**: líneas talladas, color plano y veta de la madera
- **Linograbado**: relieve tallado y audaz en negro y una tinta
- **Risografía**: tintas planas granuladas con un leve desajuste
- **Póster de viaje**: color plano de mediados de siglo con luz intensa
- **Pop art**: contornos negros, colores primarios y puntos de trama
- **Collage**: papel recortado surrealista y fragmentos de fotos

**Digital y gráfico**

- **Pixel art**: píxeles nítidos de 16 bits y una paleta limitada
- **Plano técnico**: trazos técnicos blancos sobre azul intenso
- **Low poly**: 3D facetado en triángulos de sombreado plano
- **Anime**: ilustración limpia con sombreado cel
- **Art nouveau**: líneas en latigazo, contornos dorados y tonos de joya
- **Art déco**: geometría simétrica audaz en oro y negro
- **Synthwave**: cromo ochentero iluminado por neón y bruma

## Ideas para empezar

Cada línea es un motivo, pensado para el estilo en negrita. Pégala después de `Scene:` y describe ese
estilo después de `Style:`. Son las ideas con las que los botones de estilo de un chat nuevo llenan
el cuadro, dos por estilo. Una palabra entre comillas es texto para rotular en la carpeta.

- **Póster de viaje.** Un hidroavión rojo diminuto que se posa en una laguna turquesa al atardecer, con siluetas de palmeras y un sol naranja bajo, y la palabra “ESCAPADA”.
- **Póster de viaje.** Un teleférico que sube entre picos nevados hacia un pequeño hotel alpino, en azules y blancos planos con un toque de rojo.
- **Grabado en madera.** Una carpa koi gigante que salta de un río iluminado por la luna, bajo una luna pálida, en índigo y bermellón.
- **Grabado en madera.** Un zorro con sombrero de paja que cruza bajo la lluvia un puente iluminado con farolillos, con finas líneas de lluvia.
- **Aerógrafo años 70.** Una cinta de casete cromada que flota sobre una carretera del desierto al anochecer, bajo un cielo que pasa del morado al mandarina.
- **Aerógrafo años 70.** Un patín de ruedas reluciente que orbita un planeta con anillos, con reflejos cromados y destellos de lente sobre un cielo estrellado violeta intenso.
- **Collage.** Un astronauta vintage que flota entre nubes de papel con una taza de café humeante en la mano.
- **Collage.** Una mano gigante que riega la silueta de una ciudad diminuta como si fuera una planta de interior, bajo un cielo amarillo mostaza.
- **Art nouveau.** Una mujer cuyo cabello se convierte en olas del océano, entre lirios, en tonos apagados de verde azulado, crema y coral.
- **Art nouveau.** Un pavo real posado en una luna creciente entre enredaderas en espiral, en verdes y azules de piedras preciosas.
- **Pintura al óleo.** Un gato con una capa de terciopelo que sostiene una laptop diminuta, a la luz de una vela, en rojos y dorados profundos.
- **Pintura al óleo.** Una ballena que flota a la deriva sobre un puerto dormido al amanecer, con nubes suaves y una cálida luz de la mañana.
- **Foto analógica.** Una cabina telefónica roja solitaria en una cresta nevada a la hora dorada, con sombras largas.
- **Foto analógica.** Un convertible vintage estacionado frente a un diner de carretera iluminado en una noche lluviosa, con reflejos mojados.
- **Efecto maqueta.** Una pequeña oficina de correos llena de actividad construida dentro de un cajón de madera, con trabajadores diminutos clasificando cartas bajo lámparas cálidas.
- **Efecto maqueta.** Un campamento diminuto sobre un libro abierto gigante, con una tienda de campaña, una fogata y pinos de papel bajo una suave luz de atardecer.
- **Risografía.** Un disco de vinilo que sale como el sol sobre las dunas del desierto, en verde azulado, amarillo y naranja.
- **Risografía.** Un barco de papel que navega por una ciudad de libros apilados, en azul y naranja.
- **Plastilina.** Un faro diminuto en una isla rocosa que proyecta un rayo arcoíris entre nubes esponjosas, en colores pastel.
- **Plastilina.** Un caracol que lleva una casita con ventanas iluminadas por un bosque cubierto de musgo, bajo una luz suave.

O deja que el nombre de la carpeta elija la escena: escribe después de `Scene:` algo como
*“un póster ingenioso sobre una carpeta llamada ‘Impuestos 2025’”*.

## Por qué el magenta

La mayoría de las apps de chat devuelven imágenes sin transparencia. Un fondo #FF00FF liso casi nunca
aparece en una ilustración real, así que FolderSkin puede encontrarlo, quitarlo y suavizar el borde
de forma limpia. El asistente de la app usa el mismo truco, con verde en lugar de magenta para los
modelos de Google y para el arte rosa o violeta, que recorta él mismo.

Si una app de chat te devuelve una imagen que de verdad es transparente alrededor de la carpeta,
también funciona: FolderSkin usa la transparencia tal cual.
