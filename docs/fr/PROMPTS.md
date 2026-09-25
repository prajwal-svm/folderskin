# Peindre des dossiers avec Grok ou ChatGPT

Pas besoin de clé d'API pour créer vos propres habillages. Toute app de chat capable de retoucher une
image (Grok, ChatGPT) peut vous peindre un dossier si vous lui donnez notre gabarit et le prompt
ci-dessous. Le prompt demande un fond magenta uni, que FolderSkin retire avant d'ajouter le dossier à
**Mes habillages**.

## 1. Joignez le gabarit

Téléchargez [`prompts/folder-template.png`](../prompts/folder-template.png) et joignez-le à votre
message. C'est un dossier FolderSkin vierge sur fond transparent. Le modèle le repeint, si bien que
chaque résultat garde la même forme : l'onglet en haut à gauche, une feuille de papier crème entre
les panneaux, et le panneau avant qui porte l'image.

## 2. Collez le prompt

Remplissez les deux lignes en majuscules. Gardez le reste tel quel : chaque phrase est là pour éviter
une erreur précise (un dossier penché, une ombre portée, du rose qui déborde sur l'image).

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

Vous voulez du texte dessus ? Ajoutez une ligne avant le dernier paragraphe :
`Add the title "YOUR WORDS" in bold poster lettering on the front panel.` Tenez-vous-en à un ou
deux mots : un texte long ressort illisible.

## 3. Importez-le dans FolderSkin

Enregistrez l'image, puis déposez-la sur la fenêtre de FolderSkin ou utilisez **Ajouter une photo**.
Le magenta est retiré automatiquement et le dossier apparaît dans **Mes habillages**, prêt à être
appliqué.

Si les bords montrent une frange rose, c'est que le modèle s'est éloigné du magenta pur.
Demandez-lui de repeindre le fond exactement en #FF00FF, puis réessayez.

## Des styles qui vont bien à un dossier

Choisissez-en un, ou mélangez-en deux :

- affiche de voyage vintage, aplats de couleur, texture d'impression granuleuse
- estampe ukiyo-e sur bois, aux contours épais
- affiche des années 1970 à l'aérographe, avec du chrome brillant
- collage photo surréaliste, bords de papier découpé et trame de points
- affiche Art nouveau aux bordures ornementées et aux fins filets dorés
- peinture à l'huile de la Renaissance, à la lumière dramatique
- photo cinématographique à l'heure dorée, grain de pellicule 35 mm
- diorama miniature photographié avec un objectif à bascule (tilt-shift)
- impression en risographie à trois encres
- rendu en pâte à modeler tout en douceur, comme un décor d'animation image par image
- affiche constructiviste aux diagonales marquées
- bureau d'ordinateur du début des années 90, icônes en pixels et tramage

## Des idées pour commencer

Chaque ligne est une consigne complète, sujet et style réunis. Collez-la après `Scene:` et supprimez
la ligne `Style:`. Ce sont les idées que remplissent les boutons de style de l'app, deux par style.

- **Affiche de voyage.** Un minuscule hydravion rouge qui se pose sur un lagon turquoise au coucher du soleil, des silhouettes de palmiers et un soleil orange bas sur l'horizon, en affiche de voyage vintage aux aplats de couleur, avec une texture d'impression granuleuse et le mot ÉVASION en grosses lettres rétro.
- **Affiche de voyage.** Un téléphérique qui grimpe entre des sommets enneigés vers un petit hôtel alpin, en affiche de voyage des années 1950 : aplats de bleus et de blancs, une touche de rouge, un léger grain d'impression.
- **Estampe.** Une carpe koï géante qui bondit par-dessus une grande vague sous une lune pâle, en estampe ukiyo-e aux épais contours noirs, indigo et vermillon sur papier washi.
- **Estampe.** Un renard coiffé d'un chapeau de paille qui traverse sous la pluie un pont éclairé de lanternes, en estampe de l'époque d'Edo aux aplats de couleur et aux fines lignes de pluie.
- **Aérographe années 70.** Une cassette audio chromée qui flotte au-dessus d'une route du désert bordée de néons, au crépuscule, en affiche des années 1970 à l'aérographe, avec des reflets brillants et un ciel qui passe du violet à la mandarine.
- **Aérographe années 70.** Un patin à roulettes rutilant en orbite autour d'une planète à anneaux, en illustration à l'aérographe des années 70, avec des reflets chromés, des halos d'objectif et un ciel étoilé violet profond.
- **Collage.** Un astronaute vintage qui flotte entre des nuages en papier découpé, une tasse de café fumante à la main, en collage photo surréaliste avec une trame de points et des bords de papier déchiré.
- **Collage.** Une main géante qui arrose la silhouette d'une ville miniature comme une plante d'intérieur, en collage de magazine rétro, avec une impression tramée, un grain de papier et un ciel jaune moutarde.
- **Art nouveau.** Une femme dont les cheveux se changent en vagues océanes, encadrée de lys et de filets dorés ornementés, en affiche Art nouveau aux tons sourds de bleu canard, de crème et de corail.
- **Art nouveau.** Un paon perché sur un croissant de lune parmi des volutes de vigne, en affiche Art nouveau aux fins contours dorés, dans des verts et des bleus de pierres précieuses.
- **Peinture à l'huile.** Un chat en cape de velours qui tient un minuscule ordinateur portable, en peinture à l'huile de la Renaissance, avec un éclairage dramatique à la bougie, des rouges et des ors profonds, et un vernis craquelé.
- **Peinture à l'huile.** Une baleine qui dérive au-dessus d'un port endormi à l'aube, en peinture à l'huile romantique, avec des nuages doux, une touche de pinceau visible et une chaude lumière matinale.
- **Photogramme.** Une cabine téléphonique rouge isolée sur une crête enneigée à l'heure dorée, en photogramme de cinéma 35 mm au grain doux et aux ombres longues.
- **Photogramme.** Un cabriolet vintage garé sous l'enseigne au néon d'un diner, par une nuit de pluie, en photogramme à l'ambiance sombre, avec des reflets mouillés et des couleurs bleu-vert et orange.
- **Mini diorama.** Un petit bureau de poste animé, construit dans un tiroir en bois, où de minuscules employés trient le courrier sous des lampes chaleureuses, en diorama miniature photographié en tilt-shift.
- **Mini diorama.** Un minuscule camping au sommet d'un livre ouvert géant, avec une tente, un feu de camp et des sapins en papier, en photo miniature tilt-shift dans une douce lumière du soir.
- **Risographie.** Un disque vinyle qui se lève comme le soleil sur des dunes, en risographie trois couleurs, bleu canard, jaune et orange, avec un grain visible et un léger décalage de repérage.
- **Risographie.** Un bateau en papier qui navigue dans une ville de livres empilés, en risographie deux couleurs, bleu et orange, à l'impression granuleuse et légèrement décalée.
- **Pâte à modeler.** Un minuscule phare sur une île rocheuse, qui projette un faisceau arc-en-ciel à travers des nuages cotonneux, en scène d'animation en pâte à modeler aux couleurs pastel, avec des empreintes de doigts dans la pâte.
- **Pâte à modeler.** Un escargot qui porte une petite maison aux fenêtres éclairées à travers une forêt moussue, en décor douillet d'animation en pâte à modeler, sous une lumière douce.

Ou laissez le nom du dossier choisir la scène : écrivez par exemple après `Scene:`
*« une affiche pleine d'esprit sur un dossier appelé “Impôts 2025” »*.

## Pourquoi le magenta

La plupart des apps de chat renvoient des images sans transparence. Un fond #FF00FF uni n'apparaît
presque jamais dans une vraie illustration, si bien que FolderSkin peut le repérer, le retirer et
adoucir proprement le bord. La même astuce sert quand l'assistant intégré à l'app demande un dossier
entier à un modèle.

Si une app de chat renvoie une image vraiment transparente autour du dossier, ça marche aussi :
FolderSkin utilise la transparence telle quelle.
