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
panel and the tab; the main subject sits in the middle of the front panel, fully inside it.
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

Elige uno, o mezcla dos:

- póster de viaje vintage, colores planos, textura de impresión granulada
- grabado ukiyo-e en madera con contornos gruesos
- póster de los años setenta hecho con aerógrafo y cromados brillantes
- collage fotográfico surrealista con bordes de papel recortado y trama de puntos
- póster art nouveau con bordes ornamentados y finas líneas doradas
- pintura al óleo renacentista con luz dramática
- fotografía cinematográfica a la hora dorada, grano de película de 35 mm
- diorama en miniatura fotografiado con un objetivo tilt-shift
- impresión en risografía con tres tintas
- render suave de plastilina, como un decorado de animación stop-motion
- póster constructivista con diagonales marcadas
- escritorio de un PC de principios de los noventa, iconos pixelados y tramado

## Ideas para empezar

Cada línea es un encargo completo, con motivo y estilo juntos. Pégala después de `Scene:` y borra la
línea `Style:`. Son las ideas con las que los botones de estilo de la app completan el prompt, dos
por estilo.

- **Póster de viaje.** Un hidroavión rojo diminuto que se posa en una laguna turquesa al atardecer, con siluetas de palmeras y un sol naranja bajo, como un póster de viaje vintage de colores planos, con textura de impresión granulada y la palabra ESCAPADA en letras retro gruesas.
- **Póster de viaje.** Un teleférico que sube entre picos nevados hacia un pequeño hotel alpino, como un póster de viaje de los años cincuenta: azules y blancos planos, un toque de rojo, un grano de impresión suave.
- **Grabado.** Una carpa koi gigante que salta sobre una gran ola bajo una luna pálida, como un grabado ukiyo-e en madera con gruesos contornos negros, índigo y bermellón sobre papel washi.
- **Grabado.** Un zorro con sombrero de paja que cruza bajo la lluvia un puente iluminado con farolillos, como un grabado del periodo Edo con bloques de color planos y finas líneas de lluvia.
- **Aerógrafo años 70.** Una cinta de casete cromada que flota sobre una carretera del desierto llena de neones al anochecer, como un póster de los setenta hecho con aerógrafo, con brillos satinados y un cielo que pasa del morado al mandarina.
- **Aerógrafo años 70.** Un patín de ruedas reluciente que orbita un planeta con anillos, como una ilustración de aerógrafo de los setenta, con reflejos cromados, destellos de lente y un cielo estrellado violeta intenso.
- **Collage.** Un astronauta vintage que flota entre nubes de papel recortado con una taza de café humeante en la mano, como un collage fotográfico surrealista con trama de puntos y bordes de papel rasgado.
- **Collage.** Una mano gigante que riega la silueta de una ciudad diminuta como si fuera una planta de interior, como un collage de revista retro con impresión de trama, grano de papel y un cielo amarillo mostaza.
- **Art nouveau.** Una mujer cuyo cabello se convierte en olas del océano, enmarcada por lirios y ornamentadas líneas doradas, como un póster art nouveau en tonos apagados de verde azulado, crema y coral.
- **Art nouveau.** Un pavo real posado en una luna creciente entre enredaderas en espiral, como un póster art nouveau con finos contornos dorados y verdes y azules de piedras preciosas.
- **Pintura al óleo.** Un gato con una capa de terciopelo que sostiene una laptop diminuta, como una pintura al óleo renacentista con una dramática luz de vela, rojos y dorados profundos y barniz agrietado.
- **Pintura al óleo.** Una ballena que flota a la deriva sobre un puerto dormido al amanecer, como una pintura al óleo romántica con nubes suaves, pinceladas visibles y una cálida luz de la mañana.
- **Fotograma.** Una cabina telefónica roja solitaria en una cresta nevada a la hora dorada, como un fotograma cinematográfico de 35 mm con grano suave y sombras largas.
- **Fotograma.** Un convertible vintage estacionado bajo el letrero de neón de un diner en una noche lluviosa, como un fotograma de ambiente sombrío con reflejos mojados y colores verde azulado y naranja.
- **Diorama en miniatura.** Una pequeña oficina de correos llena de actividad construida dentro de un cajón de madera, con trabajadores diminutos clasificando cartas bajo lámparas cálidas, como un diorama en miniatura con efecto tilt-shift.
- **Diorama en miniatura.** Un campamento diminuto sobre un libro abierto gigante, con una tienda de campaña, una fogata y pinos de papel, como una foto en miniatura con tilt-shift y una suave luz de atardecer.
- **Risografía.** Un disco de vinilo que sale como el sol sobre las dunas del desierto, como una risografía de tres colores en verde azulado, amarillo y naranja, con grano visible y un ligero desajuste de registro.
- **Risografía.** Un barco de papel que navega por una ciudad de libros apilados, como una risografía de dos colores en azul y naranja, con una impresión granulada y ligeramente desfasada.
- **Plastilina.** Un faro diminuto en una isla rocosa que proyecta un rayo arcoíris entre nubes esponjosas, como una escena de animación stop-motion de plastilina en colores pastel, con huellas de dedos en la plastilina.
- **Plastilina.** Un caracol que lleva una casita con ventanas iluminadas por un bosque cubierto de musgo, como un acogedor decorado de animación con plastilina bajo una luz suave.

O deja que el nombre de la carpeta elija la escena: escribe después de `Scene:` algo como
*“un póster ingenioso sobre una carpeta llamada ‘Impuestos 2025’”*.

## Por qué el magenta

La mayoría de las apps de chat devuelven imágenes sin transparencia. Un fondo #FF00FF liso casi nunca
aparece en una ilustración real, así que FolderSkin puede encontrarlo, quitarlo y suavizar el borde
de forma limpia. El mismo truco se usa cuando el asistente de la app le pide una carpeta entera a un
modelo.

Si una app de chat te devuelve una imagen que de verdad es transparente alrededor de la carpeta,
también funciona: FolderSkin usa la transparencia tal cual.
