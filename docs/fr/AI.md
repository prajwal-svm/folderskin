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
clé, rien ne part tant que vous n'avez pas cliqué sur **Générer**, et ce qui part, c'est votre
prompt, la taille qu'exige la forme et l'image de référence si vous en avez choisi une (pour un
dossier entier sans image de référence, le gabarit de dossier vierge de FolderSkin).

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

## Les deux formes

C'est le choix qui compte le plus, et il n'a rien à voir avec la qualité.

**Juste l'image** demande au modèle une image à plat de 1024 × 958, que FolderSkin plaque sur son
propre gabarit de dossier, exactement comme une photo que vous ajoutez. La
géométrie est la nôtre, si bien que chaque habillage s'aligne sur tous les autres, à toutes les
tailles d'icônes. Tous les fournisseurs en sont capables, y compris ceux qui ne gèrent pas la
transparence. C'est le choix par défaut, et le bon dans la plupart des cas.

**Dossier entier** demande au modèle de dessiner le dossier lui-même sur un fond transparent ou sur
un fond uni à détourer, et cette image devient directement l'icône, sans passer par le compositeur.
Vous renoncez à une géométrie exacte au pixel près, et vous gagnez une illustration qui peut avoir un
vrai relief et déborder du bord supérieur du dossier.

Quand le modèle sait partir d'une image (OpenAI, Grok, Gemini et FLUX.2) et que vous n'en avez pas
joint, FolderSkin envoie à la place son propre gabarit de dossier vierge : notre dossier, peint en
gris clair uni, centré sur la couleur de détourage unie, le tout en 1024 × 958 pixels
(`compositor::blank_template`). Le prompt demande au modèle de repeindre exactement ce dossier, en
gardant son contour, son onglet, sa bande de papier, sa taille et sa position, et de laisser le fond
uni. Le résultat garde la silhouette de FolderSkin au lieu du dossier que le modèle aurait inventé.
Comme le gabarit est posé sur la couleur de détourage, une telle génération passe toujours par le
détourage décrit ci-dessous, même avec un modèle capable de renvoyer de la transparence. Une image de
référence que vous joignez vous-même sert d'illustration, comme avant.

## La gestion de la transparence

FolderSkin choisit la bonne méthode selon le modèle retenu :

- **Alpha natif.** La requête demande un fond transparent, et le PNG renvoyé en a déjà un.
  FolderSkin se contente de rogner la marge transparente. GPT Image 2.5 Flare et Sunburst
  fonctionnent ainsi.
- **Pas d'alpha.** Le prompt demande le dossier seul sur une couleur de détourage unie : le magenta,
  `#FF00FF`, chez tous les fournisseurs sauf Google. FolderSkin retire ensuite cette couleur, retire
  le magenta qui a débordé dans le bord adouci (l'étape qui évite qu'un détourage ait l'air entouré
  d'un halo rose), puis rogne. Le magenta est utilisé parce qu'il n'apparaît presque jamais dans les
  illustrations de dossiers, et parce qu'un fond manquant se détecte : si la bordure n'est pas
  magenta, c'est que le modèle a ignoré la consigne, et FolderSkin le signale au lieu d'appliquer une
  icône ratée. Recraft reçoit aussi cette couleur en paramètre (`controls.background_color`), si bien
  que son fond sort uni quel que soit le style demandé.
- **Du vert pour Gemini.** Sur du magenta, Gemini laisse un liseré sombre et rougeâtre autour du
  sujet. Son prompt et son gabarit utilisent donc du vert, `#00FF00`, à la place. Or le vert a toute
  sa place dans les illustrations de dossiers (chaque feuille, chaque prairie). Un fond vert n'est
  donc retiré que là où il touche le bord de l'image, à partir de la nuance de vert que Gemini a
  réellement peinte, et les verts peints sur le dossier restent (`matte::cutout_connected`).

Le code de détourage se trouve dans `crates/folderskin-core/src/matte.rs` et est couvert par des
tests unitaires, y compris le cas d'un sujet vraiment rose sur un fond magenta.

## Les prompts

`crates/folderskin-ai/src/prompts.rs` compose le prompt à partir de vos mots et d'un contrat. Les
parties qui font le travail relèvent de la structure plus que du style :

- **Les prompts du mode Juste l'image** interdisent de dessiner un dossier, une icône, un appareil
  ou une maquette, et réservent le huitième supérieur et une bordure de 6 % comme zones mortes, car le
  gabarit les recadre ou les courbe.
- **Les prompts du mode Dossier entier** fixent la construction : exactement trois parties, un onglet, un
  bord de papier visible, un panneau avant, et une consigne explicite de ne pas ajouter de couches.
  Sans cette phrase, les modèles produisent à coup sûr des dossiers empilés et des onglets doublés.
- **Les prompts de gabarit** (`compose_on_template`) accompagnent le gabarit vierge : l'image jointe
  est le dossier exact à repeindre, sa forme et son cadrage restent tels quels, l'idée est peinte sur
  les panneaux arrière et avant, et la couleur de détourage reste unie.
- **Tous** se terminent par un contrat de sortie strict qui fixe l'isolement du sujet, et soit la
  couleur de détourage, soit le fond transparent. Ils n'indiquent jamais de taille : un modèle ne
  peint pas au nombre de pixels qu'il lit, si bien que la taille passe par les paramètres de la
  requête elle-même (voir plus bas).

Vous pouvez modifier ces prompts types. Ce sont de simples constantes de chaînes Rust, avec des tests
qui vérifient la présence des phrases essentielles.

## Ce que reçoit chaque fournisseur

Dans `crates/folderskin-ai/src/request.rs`, chaque requête est construite exactement comme le décrit
la référence de l'API de son fournisseur, et vérifiée avec le SDK du fournisseur lui-même quand il en
a un. Les tests de ce fichier contrôlent chaque requête champ par champ. Un test de
`crates/folderskin-ai/tests/providers.rs` envoie en outre chaque requête à un serveur factice qui
tourne sur votre ordinateur et répond comme l'indique la documentation du fournisseur. La méthode,
l'adresse, l'en-tête qui porte la clé, le type de contenu et chaque champ sont ainsi vérifiés tels
qu'ils partent sur le réseau.

Les options passent par les paramètres propres à chaque fournisseur, jamais par le texte du prompt.
La taille est celle de la forme (1024 × 958 pour l'illustration du style Mac). Elle est envoyée
telle quelle quand un fournisseur accepte n'importe quelle taille, et sinon sous la forme de la
taille ou des proportions les plus proches qu'il propose :

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
| « the model drew a scene instead of a folder on a plain backdrop » | Mode dossier entier sans fond détourable : réessayez ou passez à Juste l'image |
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
fournisseur, le modèle et votre prompt. Il se trouve dans la galerie, sous Mes habillages, après un
redémarrage, et le supprimer à cet endroit l'efface du disque.
[ARCHITECTURE.md](../ARCHITECTURE.md#saved-skins) indique où se trouvent les fichiers (en anglais).
Si l'écriture échoue (un disque plein, par exemple), l'habillage reste disponible jusqu'à la fin de
la session au lieu d'être perdu.

Pour partager des habillages générés avec tout le monde, mettez-les dans un pack de la communauté :
ajoutez-leur un tag et utilisez **Partager avec la communauté** dans l'app, ou transformez un dossier
de rendus enregistrés en pack avec `folderskin-tools packs make`. [PACKS.md](PACKS.md) décrit les
deux méthodes, et [SKINS.md](SKINS.md) explique comment une image se pose sur le dossier.
