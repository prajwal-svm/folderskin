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

Décrivez-en un après `Style:`, ou mélangez-en deux. Ce sont les trente styles que propose le champ
du prompt de FolderSkin quand vous tapez /. Sous **Pas de clé d'API ?**, l'app remplit le prompt
avec les mots exacts qu'elle emploie pour le style que vous choisissez.

**Photo et 3D**

- **Photo de studio** : une photo fidèle au réel, en douce lumière de studio
- **Photo argentique** : une photo 35 mm de cinéma, lumière naturelle et grain
- **Rendu produit** : un rendu 3D soigné, lumière de studio et vraies matières
- **3D isométrique** : un modèle 3D net, vu en perspective isométrique
- **Effet maquette** : une vraie scène photographiée comme une maquette

**Matières et artisanat**

- **Pâte à modeler** : de la pâte à modeler faite main, en lumière chaude
- **Verre** : du verre sculpté translucide, avec réfractions et caustiques
- **Néon** : des tubes néon lumineux sur fond très sombre
- **Émail** : émail cloisonné brillant, cerné d'or en relief
- **Broderie** : une broderie au passé plat, dense, en fil brillant et en relief
- **Papier découpé** : du papier découpé en couches, avec de vraies ombres entre elles
- **Vitrail** : un vitrail aux tons de pierres précieuses, éclairé par derrière

**Peinture et dessin**

- **Peinture à l'huile** : une riche peinture à l'huile classique, en lumière dramatique
- **Aquarelle** : des lavis libres, lumineux et transparents
- **Gouache** : une peinture mate et opaque, en aplats harmonieux
- **Crayon** : un dessin au graphite détaillé, du gris clair au noir profond
- **Aérographe années 70** : des dégradés soyeux, du chrome brillant et des reflets en étoile

**Impression**

- **Gravure sur bois** : traits gravés, aplats de couleur et veines du bois
- **Linogravure** : un relief gravé audacieux, en noir et une encre
- **Risographie** : des tons directs granuleux, légèrement décalés
- **Affiche de voyage** : des aplats de couleur des années 50, en lumière franche
- **Pop art** : contours noirs, couleurs primaires et trame de points
- **Collage** : papier découpé surréaliste et fragments de photos

**Numérique et graphisme**

- **Pixel art** : des pixels 16 bits nets et une palette réduite
- **Plan technique** : un tracé technique blanc sur bleu profond
- **Low poly** : de la 3D à facettes, en triangles unis
- **Anime** : une illustration nette, en aplats façon celluloïd
- **Art nouveau** : lignes en coup de fouet, contours dorés et tons de pierres précieuses
- **Art déco** : une géométrie symétrique audacieuse, en or et noir
- **Synthwave** : du chrome des années 80 éclairé au néon, dans la brume

## Des idées pour commencer

Chaque ligne est un sujet, écrit pour le style en gras. Collez-la après `Scene:` et décrivez ce
style après `Style:`. Ce sont les idées que remplissent les boutons de style sous un nouveau chat,
deux par style. Un mot entre guillemets est destiné à être écrit sur le dossier.

- **Affiche de voyage.** Un minuscule hydravion rouge qui se pose sur un lagon turquoise au coucher du soleil, des silhouettes de palmiers et un soleil orange bas sur l'horizon, avec le mot « ÉVASION ».
- **Affiche de voyage.** Un téléphérique qui grimpe entre des sommets enneigés vers un petit hôtel alpin, en aplats de bleus et de blancs avec une touche de rouge.
- **Gravure sur bois.** Une carpe koï géante qui bondit hors d'une rivière nocturne, sous une lune pâle, en indigo et vermillon.
- **Gravure sur bois.** Un renard coiffé d'un chapeau de paille qui traverse sous la pluie un pont éclairé de lanternes, avec de fines lignes de pluie.
- **Aérographe années 70.** Une cassette audio chromée qui flotte au-dessus d'une route du désert au crépuscule, sous un ciel qui passe du violet à la mandarine.
- **Aérographe années 70.** Un patin à roulettes rutilant en orbite autour d'une planète à anneaux, avec des reflets chromés et des halos d'objectif sur fond de ciel étoilé violet profond.
- **Collage.** Un astronaute vintage qui flotte entre des nuages en papier, une tasse de café fumante à la main.
- **Collage.** Une main géante qui arrose la silhouette d'une ville miniature comme une plante d'intérieur, sous un ciel jaune moutarde.
- **Art nouveau.** Une femme dont les cheveux se changent en vagues océanes, parmi des lys, dans des tons sourds de bleu canard, de crème et de corail.
- **Art nouveau.** Un paon perché sur un croissant de lune parmi des volutes de vigne, dans des verts et des bleus de pierres précieuses.
- **Peinture à l'huile.** Un chat en cape de velours qui tient un minuscule ordinateur portable, éclairé à la bougie, dans des rouges et des ors profonds.
- **Peinture à l'huile.** Une baleine qui dérive au-dessus d'un port endormi à l'aube, avec des nuages doux et une chaude lumière matinale.
- **Photo argentique.** Une cabine téléphonique rouge isolée sur une crête enneigée à l'heure dorée, avec de longues ombres.
- **Photo argentique.** Un cabriolet vintage garé devant un diner illuminé au bord de la route, par une nuit de pluie, avec des reflets mouillés.
- **Effet maquette.** Un petit bureau de poste animé, construit dans un tiroir en bois, où de minuscules employés trient le courrier sous des lampes chaleureuses.
- **Effet maquette.** Un minuscule camping au sommet d'un livre ouvert géant, avec une tente, un feu de camp et des sapins en papier dans une douce lumière du soir.
- **Risographie.** Un disque vinyle qui se lève comme le soleil sur des dunes, en bleu canard, jaune et orange.
- **Risographie.** Un bateau en papier qui navigue dans une ville de livres empilés, en bleu et orange.
- **Pâte à modeler.** Un minuscule phare sur une île rocheuse, qui projette un faisceau arc-en-ciel à travers des nuages cotonneux, dans des couleurs pastel.
- **Pâte à modeler.** Un escargot qui porte une petite maison aux fenêtres éclairées à travers une forêt moussue, sous une lumière douce.

Ou laissez le nom du dossier choisir la scène : écrivez par exemple après `Scene:`
*« une affiche pleine d'esprit sur un dossier appelé “Impôts 2025” »*.

## Pourquoi le magenta

La plupart des apps de chat renvoient des images sans transparence. Un fond #FF00FF uni n'apparaît
presque jamais dans une vraie illustration, si bien que FolderSkin peut le repérer, le retirer et
adoucir proprement le bord. L'assistant intégré à l'app utilise la même astuce, avec du vert au lieu
du magenta pour les modèles de Google et pour les images roses ou violettes, qu'il détoure lui-même.

Si une app de chat renvoie une image vraiment transparente autour du dossier, ça marche aussi :
FolderSkin utilise la transparence telle quelle.
