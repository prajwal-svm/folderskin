# L'assistant IA

FolderSkin peut générer un habillage à partir d'une description, de deux façons :

- **Le Modèle local** peint sur votre propre ordinateur, gratuitement. Installez-le une fois
  (FLUX.2 [klein] 4B, un téléchargement de 4,6 Go sur Mac et de 5,2 Go ailleurs), et il fonctionne
  hors ligne, sans clé et sans rien envoyer nulle part. Il tourne sur les Mac à puce Apple sous
  macOS 14 ou plus récent, et sur les PC Windows et Linux.
- **Votre propre clé** : vous collez une clé d'API d'un fournisseur chez qui vous avez déjà un
  compte, la clé est enregistrée dans un fichier privé sur votre ordinateur, et FolderSkin dialogue
  directement avec ce fournisseur depuis votre machine.

Le choix se fait dans **Réglages → Fournisseur d'IA**. Une coche indique que le Modèle local est
installé, ou qu'un fournisseur a une clé.

![Réglages, Fournisseur d'IA : le Modèle local et sept fournisseurs, chacun coché ou marqué Pas de clé, avec en dessous le modèle installé sur cette machine](../images/ai-providers.webp)

Il n'y a ni serveur FolderSkin, ni proxy, ni clé intégrée, ni offre gratuite à financer. Avec une
clé, rien ne part tant que vous n'avez pas cliqué sur **Générer**, et ce qui part, c'est le prompt
que FolderSkin rédige à partir de vos mots, la taille qu'exige la forme, et les images qui
l'accompagnent : pour un dossier entier, le gabarit vierge de ce dossier fourni par FolderSkin, puis
les éventuelles images de référence que vous avez ajoutées.

## Où est stockée la clé

Chiffrée, dans le dossier propre à FolderSkin, lisible uniquement par votre compte utilisateur :

| | |
|---|---|
| macOS | `~/Library/Application Support/app.folderskin.desktop/keys.json` |
| Windows | `%APPDATA%\app.folderskin.desktop\keys.json` |
| Linux | `~/.config/app.folderskin.desktop/keys.json` |

Les clés sont scellées avec AES-256-GCM avant d'être écrites. La clé de chiffrement est dérivée
(HKDF-SHA256) d'un secret aléatoire stocké dans `keys.secret`, à côté de `keys.json`, et de
l'identifiant matériel de cet ordinateur. `keys.json` seul ne révèle donc rien, et les deux fichiers
copiés sur un autre ordinateur ne s'y ouvrent pas : saisissez à nouveau les clés sur le nouveau. Les
deux fichiers sont créés avec des permissions réservées au propriétaire (0600) et écrits de façon
atomique. Ce qu'aucun fichier ne peut faire, c'est tenir à l'écart un programme qui tourne déjà sous
votre compte et pourrait lire les deux. Seul le trousseau du système le pourrait, avec les demandes
de mot de passe décrites ci-dessous.

Pourquoi pas le trousseau du système : macOS lie un élément du trousseau à la signature exacte de
l'app qui l'a enregistré. Les versions open source sont généralement non signées ou signées ad hoc,
si bien que chaque recompilation ou mise à jour passerait pour une autre app et redemanderait votre
mot de passe de session. Une demande de mot de passe venant d'une app que vous venez de télécharger
ressemble exactement à ce qu'elle n'est pas. FolderSkin n'utilise donc pas du tout le trousseau.

La clé est lue au moment de la requête, n'apparaît jamais dans un message d'erreur et n'est jamais
renvoyée à la fenêtre de l'app. **Supprimer la clé**, dans la fenêtre du fournisseur, l'efface du
fichier. Supprimer `keys.json` les efface toutes.

## Ce pour quoi une image est faite

Chaque image est faite pour une forme. La pastille à côté du nom du modèle, sous le champ du prompt,
indique laquelle et en montre une petite image. Cliquez dessus, ou tapez @ dans le champ, pour en
choisir une autre :

- **Dossier Mac** : le dossier de FolderSkin, tel que le Finder l'affiche.
- **Dossier Windows** : le dossier que dessine Windows.
- **Icône libre** : une seule chose, comme une mascotte, un objet ou un personnage, sans dossier
  autour. Elle se pose telle quelle sur n'importe quel dossier.

Après @, tapez une partie d'un nom (`@win`) et appuyez sur Entrée ou Tab, ou cliquez sur l'une des
formes. La pastille change et le mot qui commence par @ disparaît du champ. Un nouveau chat part du
dossier sur lequel le panneau du dossier montre les habillages.

La forme appartient au chat. Chaque image est faite pour la forme choisie au moment de l'envoi, un
chat plus ancien s'ouvre sur la forme de sa dernière image, et l'habillage est enregistré comme fait
pour cette forme. La forme détermine le prompt, le gabarit, la taille et la façon dont le résultat
est détouré, avec chaque fournisseur comme avec le Modèle local.

## Juste l'image ou le dossier entier

Pour un dossier, c'est le choix qui compte le plus, et il n'a rien à voir avec la qualité.

**Juste l'image** demande au modèle une image à plat aux proportions du dossier (1024 × 960 pour le
dossier du Mac, 1024 × 800 pour celui de Windows), que FolderSkin plaque sur son propre dossier,
exactement comme une photo que vous ajoutez. La géométrie est la nôtre, si bien que chaque habillage
s'aligne sur tous les autres, à toutes les tailles d'icônes. Tous les fournisseurs en sont capables,
y compris ceux qui ne gèrent pas la transparence. C'est le choix par défaut, et le bon dans la
plupart des cas.

**Dossier entier** demande au modèle de peindre le dossier lui-même, et cette image devient
directement l'icône, sans passer par le compositeur. Vous renoncez à une géométrie exacte au pixel
près, et vous gagnez une illustration qui peut avoir un vrai relief et déborder du bord supérieur du
dossier.

Quand le modèle sait partir d'une image (OpenAI, Grok, Gemini et FLUX.2, ainsi que le Modèle
local), FolderSkin envoie en première image son propre gabarit vierge du dossier : le dossier peint
en gris clair uni, centré sur une couleur de détourage unie, aux proportions du dossier et à 1024
pixels au plus sur son grand côté (`Base::blank`). Le prompt demande au modèle de repeindre
exactement ce dossier, en gardant son contour, son onglet, les éléments qui en font ce dossier-là,
sa taille et sa position, et de laisser le fond tel quel. La couleur de détourage n'est jamais
nommée, car un modèle à qui l'on parle de magenta en met dans sa peinture. FolderSkin détoure
ensuite la peinture en suivant le contour même du dossier, si bien que le résultat garde la
silhouette de FolderSkin et les couleurs de la peinture jusqu'au bord. Si la peinture a déplacé ou
déformé le dossier, c'est la couleur de détourage qui sert à la découper. Les images de référence
que vous ajoutez viennent après le gabarit, chacune avec le rôle que vous lui avez donné, autant
que le modèle en accepte.

Une **Icône libre** est toujours peinte en entier, puis détourée : un seul sujet, complet, au milieu
d'un carré, sur un fond transparent ou sur une couleur de détourage.

## La gestion de la transparence

FolderSkin choisit la bonne méthode selon le modèle retenu :

- **Alpha natif.** La requête demande un fond transparent, et le PNG renvoyé en a déjà un.
  FolderSkin se contente de rogner la marge transparente. GPT Image 2.5 Flare et Sunburst
  fonctionnent ainsi.
- **Pas d'alpha.** Le prompt demande le dossier seul sur une couleur de détourage unie. FolderSkin
  retire ensuite cette couleur, retire ce qui en a débordé dans le bord adouci (l'étape qui évite
  qu'un détourage ait l'air entouré d'un halo coloré), puis rogne. Un fond manquant se détecte : si
  la bordure n'est pas de la couleur de détourage, c'est que le modèle a ignoré la consigne, et
  FolderSkin garde l'image pour la plaquer sur son propre dossier au lieu d'appliquer une icône
  ratée. Recraft reçoit aussi cette couleur en paramètre (`controls.background_color`), si bien que
  son fond sort uni quel que soit le style demandé.

La couleur de détourage est le magenta, `#FF00FF`, parce qu'il n'apparaît presque jamais dans les
illustrations de dossiers. C'est le vert, `#00FF00`, pour Gemini, qui laisse un liseré sombre et
rougeâtre autour d'un sujet posé sur du magenta, et pour tout ce qui doit être rose ou violet, qu'un
détourage au magenta rongerait : les styles Néon, Aérographe années 70, Pop art et Synthwave, et
toute idée qui nomme le rose ou le violet dans l'une des langues de FolderSkin (pink, lilac, rose,
rosa, morado, ピンク, 보라, 粉红, etc.). Or le vert a toute sa place dans les illustrations de
dossiers, dans chaque feuille, chaque prairie. Un fond vert n'est donc retiré que là où il touche le
bord de l'image, à partir de la nuance de vert réellement peinte, et les verts peints sur le dossier
restent (`matte::cutout_connected`).

Un dossier entier peint sur le gabarit de FolderSkin ne suit aucune de ces deux méthodes. Il est
détouré en suivant le contour même du gabarit, comme décrit plus haut, et ne se rabat sur la couleur
de détourage que si le dossier a bougé.

Le code de détourage se trouve dans `crates/folderskin-core/src/matte.rs` et est couvert par des
tests unitaires, y compris le cas d'un sujet vraiment rose sur un fond magenta.

## Les prompts

Vos mots ne sont jamais réécrits. `crates/folderskin-ai/src/recipe.rs` rassemble dans une seule
recette ce dont une image a besoin : votre idée, la forme et ce qu'elle garde, le style, le texte
éventuel, vos images et le rôle de chacune, et ce qui entoure le sujet. `prompts.rs` rédige ensuite
cette recette de la façon dont chaque famille de modèles la lit le mieux, toujours dans le même
ordre : ce qu'il faut peindre, son aspect, son cadrage, les images, le texte, et ce qu'il faut
exclure.

- **OpenAI et Gemini** reçoivent des lignes à intitulé (Style, Composition, Lettering,
  Constraints), où les images s'appellent image 1, image 2, et ainsi de suite.
- **Grok** reçoit la même chose, avec les images appelées `<IMAGE_0>`, `<IMAGE_1>`, comme Grok les
  nomme.
- **FLUX** (Black Forest Labs et le Modèle local) reçoit une prose simple, qui commence par le sujet
  et ne dit rien de ce qu'il ne faut pas dessiner, car FLUX n'a pas de prompt négatif et peint ce
  qu'on lui demande d'éviter. Le prompt du Modèle local tient dans les 400 tokens que lit son
  encodeur de texte, en réduisant au besoin le style à son médium.
- **Ideogram et Recraft** reçoivent un court brief de design, où le texte arrive tôt.
- **Stability** reçoit une courte liste. Stability et Ideogram reçoivent ce qu'il faut exclure comme
  prompt négatif (voir plus bas).

Les parties qui font le travail relèvent de la structure plus que du style :

- **Les prompts du mode Juste l'image** demandent une seule image continue qui remplit le cadre,
  avec le sujet en grand et au milieu, et réservent au ciel ou à une texture la bande que cache
  l'onglet du dossier, car le gabarit la recadre ou la courbe. Pour le dossier de Windows, ils
  laissent aussi le coin supérieur gauche dégagé.
- **Les prompts du mode Dossier entier** nomment les parties du dossier, de l'arrière vers l'avant,
  et ce qui reste tel quel : l'onglet unique du Mac et sa bande de papier pâle, ou le décrochement
  arrondi du dossier de Windows. Sans cela, les modèles produisent à coup sûr des dossiers empilés
  et des onglets doublés.
- **Les prompts pour une icône libre** demandent un seul objet complet, centré dans un carré sans
  être coupé, sans sol, décor ni cadre autour.
- **Les images de référence** sont désignées par leur numéro et leur rôle. Un sujet reste
  reconnaissable, une image de style donne son médium, sa palette, sa lumière et sa texture sans
  rien de son contenu, et une image de couleurs ne donne que ses couleurs.
- **Ce qu'il faut exclure** est rédigé pour la forme et le style : toujours les bordures, les
  cadres, les filigranes et les signatures, le texte si vous n'en avez pas demandé, les clichés
  propres au style (comme le mont Fuji pour une gravure sur bois) si votre idée ne les demande pas,
  et l'allure de clip art pour un style réaliste.
- **Aucun prompt n'indique de taille.** Un modèle ne peint pas au nombre de pixels qu'il lit, si
  bien que la taille passe par les paramètres de la requête elle-même (voir plus bas).

Des tests vérifient la présence des phrases essentielles pour chaque famille et chaque forme, et la
recette est versionnée (`RECIPE_VERSION`), si bien qu'un habillage peut dire quelle version l'a
créé.

## Les styles

Tapez / dans le champ du prompt pour afficher trente styles, en cinq groupes : Photo et 3D,
Matières et artisanat, Peinture et dessin, Impression, et Numérique et graphisme. Tapez une partie
d'un nom pour affiner la liste. Un style se place dans son propre emplacement, à côté du champ,
jamais dans vos mots, si bien que l'idée reste mot pour mot et que le style s'ajoute après elle.
Cliquez sur le x de l'emplacement pour le retirer.

Chaque style est une entrée de `crates/folderskin-ai/src/styles.json`, que lisent l'app, la ligne
de commande (`folderskin ai styles` les liste) et le code qui rédige les prompts. Une entrée donne
le nom du style, une description d'une ligne, les mots que le prompt emploie pour lui (le médium,
puis sa technique, sa lumière, sa couleur et sa texture, et jamais le nom d'un artiste), la façon
dont il dessine les lettres, ce qu'il a tendance à ajouter sans qu'on le demande, trois critères
qu'un résultat doit remplir, et le préréglage correspondant chez chaque fournisseur qui en a un (le
préréglage de style de Stability, et le préréglage ou le type de style d'Ideogram). Les noms de
styles des versions précédentes marchent toujours : `travel` correspond désormais à Affiche de
voyage (`screenprint`), `ukiyoe` à Gravure sur bois et `diorama` à Effet maquette.

Le même menu propose des **Idées** pour commencer (un sujet chacune, sans style) et **Vos
prompts**. Les boutons de style sous un nouveau chat fonctionnent de la même façon : chaque clic
remplit le champ avec une autre idée et place le style du bouton dans l'emplacement.

## Le texte sur l'habillage

Mettez entre guillemets les mots que vous voulez sur l'habillage : *un renard qui lit une carte,
avec le mot « ÉVASION »*. FolderSkin écrit exactement ce qui est entre guillemets, épelé lettre par
lettre pour les modèles qui suivent des consignes, une seule fois, dans le lettrage propre au style,
et à la place qui convient à la forme : en travers du milieu du panneau avant pour un dossier, et
sur l'objet ou en dessous pour une icône libre. Sans guillemets, le prompt ne demande aucun texte.
Tenez-vous-en à un ou deux mots courts, car un texte long ressort encore illisible.

## Vos prompts

**Enregistrer comme prompt**, dans le menu /, garde le contenu du champ, avec son style, sous le nom
que vous lui donnez. Il figure ensuite dans **Vos prompts** : choisissez-le, et le champ et
l'emplacement du style se remplissent à nouveau. Si vous tapez un nom déjà pris, l'app vous le
signale, et l'enregistrement remplace ce prompt. Le x à côté de l'un des vôtres le retire, avec
**Annuler** pendant quelques secondes.

Les prompts enregistrés sont conservés dans `skills.json`, à côté du dossier `skins`
([ARCHITECTURE.md](../ARCHITECTURE.md#saved-skins), en anglais, indique où il se trouve), sous forme
de *skills* au format `folderskin.skill/1` que décrit `crates/folderskin-ai/src/skill.rs`. Une skill
sépare ce que montre une image (`idea`) de son apparence (`base_style`, ou ses propres `treatment`,
`palette` et `light`), si bien qu'un même style enregistré peut accompagner n'importe quelle idée.
Chaque skill est vérifiée avant d'être écrite : il lui faut un nom de 60 caractères au plus et
quelque chose à enregistrer, son style doit être l'un de ceux de FolderSkin, son propre traitement
compte de 8 à 60 mots et dit au modèle quoi faire plutôt que quoi éviter, et la skill entière tient
en 4 Ko. Un prompt qui désigne quelqu'un comme style (un nom après « in the style of » ou après
« by ») est enregistré après une mise en garde, car décrire la technique donne de meilleurs
résultats.

## Les images de référence

Une image ajoutée à un prompt sert de **Sujet**, sauf si vous en décidez autrement. Cliquez sur sa
pastille pour qu'elle serve de **Style** (un style à reproduire, sans rien reprendre de ce qu'elle
montre) ou de **Couleurs** (sa palette, et rien d'autre). Les images partent vers le modèle dans cet
ordre, après le gabarit, et le prompt désigne chacune par son numéro et son rôle.

## Ce que reçoit chaque fournisseur

Dans `crates/folderskin-ai/src/request.rs`, chaque requête est construite exactement comme le décrit
la référence de l'API de son fournisseur, et vérifiée avec le SDK du fournisseur lui-même quand il en
a un. Les tests de ce fichier contrôlent chaque requête champ par champ. Un test de
`crates/folderskin-ai/tests/providers.rs` envoie en outre chaque requête à un serveur factice qui
tourne sur votre ordinateur et répond comme l'indique la documentation du fournisseur. La méthode,
l'adresse, l'en-tête qui porte la clé, le type de contenu et chaque champ sont ainsi vérifiés tels
qu'ils partent sur le réseau.

Les options passent par les paramètres propres à chaque fournisseur, jamais par le texte du prompt.
La taille est celle de la forme (1024 × 960 pour l'illustration du dossier du Mac, 1024 × 800 pour
celle de Windows, un dossier entier à ses propres proportions et une icône libre en carré). Elle
est envoyée telle quelle quand un fournisseur accepte n'importe quelle taille, et sinon sous la
forme de la taille ou des proportions les plus proches qu'il propose :

| Fournisseur | Requête | Taille | Autres paramètres |
|---|---|---|---|
| OpenAI | du JSON vers `images/generations`, ou un formulaire vers `images/edits` avec chaque image en `image[]` | exacte, sur une grille de 16 pixels (1024 × 960) | `quality` : high pour Flare, max pour Sunburst, medium pour GPT Image 2, pour que le prix soit connu d'avance |
| xAI Grok | du JSON uniquement, les images étant incluses sous forme d'URL de données (`image`, ou `images` s'il y en a plusieurs) | l'`aspect_ratio` le plus proche, ou le cadre du gabarit lui-même | |
| Google Gemini | du JSON vers `generateContent` | `generationConfig.imageConfig` : 1K, avec l'`aspectRatio` le plus proche ou le cadre du gabarit | `responseModalities: ["IMAGE"]` |
| Black Forest Labs | du JSON, avec les images en `input_image`, `input_image_2` et ainsi de suite | exacte, dans la limite d'un mégapixel (FLUX.2 facture chaque mégapixel entamé) | réécriture du prompt désactivée (`disable_pup`, ou `prompt_upsampling: false` sur flex) |
| Recraft | du JSON | la taille la plus proche dans la liste de V4.1 | une sortie en PNG, et la couleur de détourage en `controls.background_color` |
| Stability AI | un formulaire | l'`aspect_ratio` le plus proche | un prompt négatif, et un préréglage de style quand le style en a un |
| Ideogram | un formulaire | la plus proche des résolutions de 3.0 (1024 × 960) | Magic Prompt désactivé, un prompt négatif, et un type ou un préréglage de style |

Le prompt négatif écarte le texte (sauf si vous en demandez), les filigranes et les signatures, ainsi
que tout ce que le style choisi a tendance à ajouter. OpenAI, Gemini et Grok n'ont pas de prompt
négatif : leur prompt dit donc la même chose en toutes lettres.

## Modèles retirés

Un choix enregistré à l'époque où un modèle était proposé passe au modèle qui l'a remplacé, et un
habillage garde le nom du modèle qui l'a créé.

| Ancien modèle | Nouveau modèle | Raison |
|---|---|---|
| OpenAI GPT Image 1 | GPT Image 2 | OpenAI l'arrête le 23 octobre 2026 |
| Gemini 2.5 Flash Image | Gemini 3.1 Flash Image | Google l'arrête le 2 octobre 2026 |
| FLUX 1.1 Pro | FLUX.2 pro | la génération précédente, qui n'acceptait pas d'images |
| Recraft V3 | Recraft V4.1 | la génération précédente, dont les prompts s'arrêtent à 1 000 caractères, moins que n'en occupent les seules instructions de FolderSkin |

Ideogram 4.0 n'est pas encore proposé : il réécrit tout prompt rédigé en texte libre, alors que
FolderSkin garde l'idée exactement telle que vous l'avez écrite.

## Coût

Chaque requête est facturée sur votre propre compte par votre fournisseur. La vue Générer affiche le
prix approximatif du modèle avant que vous cliquiez sur le bouton, d'après la page de tarifs du
fournisseur pour les paramètres qu'envoie FolderSkin. Un dossier entier coûte un peu plus cher chez
les fournisseurs qui facturent aussi les images qu'on leur envoie (xAI, Black Forest Labs). Sous
chaque image, Détails indique ce que la requête a coûté selon le fournisseur (xAI, Black Forest Labs,
Recraft), ou ce qu'elle a consommé (OpenAI, Gemini).

FolderSkin fait exactement une requête par clic, et ne réessaie jamais de lui-même. C'est aussi vrai
quand un fournisseur termine sans image, comme Gemini le fait parfois (`NO_IMAGE`) : le chat le
signale et propose **Réessayer**, et seul votre clic renvoie la requête.

## Compilation et compilation croisée

La couche des fournisseurs utilise `rustls` pour le TLS, dont le moteur cryptographique
(`aws-lc-sys`) compile du C. Tout se compile sans problème sur le runner de CI propre à chaque
plateforme, et c'est ainsi que sont produites les versions de FolderSkin. La compilation croisée
d'un système de bureau vers un autre (par exemple
`cargo check --target x86_64-pc-windows-msvc` sur un Mac) demande une chaîne de compilation croisée
C pour la cible, sans quoi elle échoue dans le script de build de `aws-lc-sys`. Le reste du
workspace se vérifie en compilation croisée sans cela.

## Messages d'erreur possibles

| Message | Ce qui s'est passé |
|---|---|
| « add your … API key first » | Aucune clé enregistrée pour ce fournisseur |
| « that key was rejected by … » | Le fournisseur a refusé la clé |
| « … is rate limiting you right now » | 429 : attendez, puis réessayez |
| « … finished without painting a picture » | Le fournisseur a répondu sans image et sans dire pourquoi (`NO_IMAGE` chez Gemini) : réessayez ou reformulez l'idée |
| « … declined that prompt: its filter blocked the picture » | Le filtre de sécurité du fournisseur a bloqué le prompt ou l'image (l'image floutée de Stability ou le contrôle de sécurité d'Ideogram, par exemple) : reformulez le prompt |
| « … said: … (error 400) » | Le message du fournisseur, affiché tel quel. À signaler s'il cite un champ envoyé par FolderSkin |
| « the model drew a scene instead of a folder on a plain backdrop » | Mode dossier entier sans fond détourable : réessayez ou passez à Juste l'image. L'app garde alors l'image pour la plaquer sur son propre dossier (une icône libre reste l'image carrée qu'elle est), et le signale. |
| « the provider returned something that is not an image » | Une réponse mal formée, ou qui n'est pas une image |

## Dossiers créés dans un assistant conversationnel

Vous pouvez aussi peindre un dossier entier dans ChatGPT, Grok ou tout autre assistant
conversationnel, puis l'importer avec **Ajouter une photo**. Demandez le dossier sur un fond #FF00FF
uni, ou sur un fond transparent. FolderSkin reconnaît l'un comme l'autre et utilise l'image telle
quelle comme icône, détourée et rognée, au lieu de la plaquer une seconde fois sur son propre
dossier. Toute autre image est traitée comme une illustration pour le gabarit.
[ARCHITECTURE.md](../ARCHITECTURE.md#artwork-or-a-finished-folder) donne les règles exactes (en
anglais), y compris la raison pour laquelle la photo d'un objet posé sur du papier magenta reste une
image.

## Conserver un habillage généré

Chaque habillage généré est enregistré dès son arrivée, comme une image importée, avec le
fournisseur, le modèle, votre prompt et la forme pour laquelle il a été fait. Il se trouve dans la
galerie, sous Mes habillages, après un redémarrage, et le supprimer à cet endroit l'efface du
disque. [ARCHITECTURE.md](../ARCHITECTURE.md#saved-skins) indique où se trouvent les fichiers (en
anglais). Si l'écriture échoue (un disque plein, par exemple), l'habillage reste disponible jusqu'à
la fin de la session au lieu d'être perdu.

Il garde aussi ce qui a servi à le faire, sous `recipe` dans l'index des habillages, si bien qu'on
peut remonter d'un résultat à son prompt et le refaire : le prompt exactement tel qu'il a été
envoyé, le prompt négatif pour les fournisseurs qui en acceptent un, le style, le texte écrit
dessus, le rôle de chaque image et son empreinte, le gabarit et sa version (`mac-folder/1`), la
couleur de détourage, et la version de la recette.

Le chat garde son texte et ses images pendant que vous consultez d'autres vues, y compris une image
encore en cours de création, et un nouveau chat commence quand FolderSkin est rouvert. Les chats
précédents se trouvent dans la liste des chats, chacun avec sa forme.

Pour partager des habillages générés avec tout le monde, mettez-les dans un pack de la communauté :
ajoutez-leur un tag et utilisez **Partager avec la communauté** dans l'app, ou transformez un dossier
de rendus enregistrés en pack avec `folderskin-tools packs make`. [PACKS.md](PACKS.md) décrit les
deux méthodes, et [SKINS.md](SKINS.md) explique comment une image se pose sur le dossier.
