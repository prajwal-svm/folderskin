# Comment une image devient une icône de dossier

Chaque habillage est une image : une image que vous avez ajoutée, une image peinte par un modèle
d'IA ou une image tirée d'un pack de la communauté. FolderSkin la transforme en icône de dossier de
l'une de deux façons, et une image réussie en elle-même peut malgré tout mal rendre sur un dossier.
Cette page explique comment FolderSkin décide, ce que le dossier fait à une image, et comment en
vérifier une avant de la partager dans un pack ([PACKS.md](PACKS.md)).

Si vous voulez qu'un agent fasse le travail,
[.claude/skills/folderskin-skins/SKILL.md](../../.claude/skills/folderskin-skins/SKILL.md) guide
Claude Code pas à pas dans la création d'un pack.

## Illustration ou dossier fini

FolderSkin regarde ce qui entoure l'image :

- **Un dossier fini** repose sur une vraie transparence, ou sur un magenta uni `#FF00FF`. Le magenta
  est retiré, l'image est rognée au ras du dossier, et elle devient l'icône exactement telle qu'elle
  est dessinée : mise à l'échelle pour tenir dans l'icône et centrée, jamais recadrée. C'est ce que
  peint un modèle d'image à partir des prompts de [PROMPTS.md](PROMPTS.md).
- **Tout le reste est une illustration** : une photo, une peinture, un motif. FolderSkin la plaque
  sur son propre gabarit de dossier, si bien qu'elle reçoit le même onglet, le même papier et le même
  contour que tous les autres habillages.

Le magenta doit être un `#FF00FF` uni, ou très proche, si bien qu'une photo de produit sur papier
magenta ou un coucher de soleil rose vif restent des illustrations.
[ARCHITECTURE.md](../ARCHITECTURE.md#artwork-or-a-finished-folder) donne les règles exactes (en
anglais). La suite de cette page porte sur les illustrations.

## Zones de sécurité

Le compositeur ajuste la même image pour qu'elle couvre deux rectangles du canevas d'icône de
1024 × 1024 ([crates/folderskin-core/src/geometry.rs](../../crates/folderskin-core/src/geometry.rs)
contient les constantes) :

- **Panneau arrière** : la partie avec l'onglet, de y = 36,5 à y = 973,5. Ses proportions sont très
  proches de celles d'une image de 1024 × 958, si bien que *toute la hauteur* y est visible et
  qu'environ 18 px sont rognés de chaque côté (1,8 %).
- **Panneau avant** : ce qui recouvre le papier, de y = 160,5 à y = 973,5, avec des coins arrondis
  de 55 px de rayon. Il est plus large que l'image, si bien que l'image est mise à l'échelle de la
  largeur du panneau et que seuls les ~87 % du milieu de sa hauteur restent visibles : environ 6 %
  sont rognés en haut, et autant en bas.

Voici ce que cela donne en bandes d'une image de 1024 × 958 (une image de mêmes proportions à une
autre taille se comporte de la même façon). Les bandes se chevauchent parce que les deux panneaux
montrent des parties de la même image qui se chevauchent : le panneau arrière montre toute la
hauteur, le panneau avant le milieu.

| lignes (sur 958) | part de la hauteur | où elles se retrouvent |
|---|---|---|
| 0 – 62 | les 6,5 % du haut | dans l'onglet |
| 62 – 127 | les 6,7 % suivants | la bande de panneau arrière au-dessus du panneau avant, à côté de la feuille de papier |
| 60 – 898 | les ~87 % du milieu | le panneau avant, la partie que l'on regarde vraiment |
| 898 – 958 | les 6,3 % du bas | rognées |

Deux règles en découlent :

1. **Rien d'important dans les 12 % du haut.** Ces lignes correspondent à l'onglet et à la fine bande
   au-dessus du panneau avant. Un visage, un horizon ou un logo placé là est coupé en deux par la
   feuille de papier.
2. **Gardez le sujet au milieu.** Le panneau avant perd environ 60 px en haut et en bas, et ses coins
   sont arrondis de 55 px à 1024 px, si bien que les détails tout au bout des coins disparaissent de
   toute façon aux petites tailles d'icônes.

Les illustrations en aplats et abstraites supportent tout cela sans qu'on y pense. Une photo avec un
sujet bien net a besoin de ce sujet dans la bande du milieu. FolderSkin garde les illustrations
centrées : si le sujet est trop haut ou trop bas, recadrez l'image vous-même avant de l'ajouter ou
de la mettre dans un pack.

`folderskin-tools guide` dessine ces bandes :

```sh
cargo run -p folderskin-tools -- guide --out /tmp/guide.png
```

La commande écrit un gabarit de 1024 × 958 sur lequel sont marqués l'onglet, la bande de papier, le
panneau avant et les lignes que le panneau avant rogne. Ouvrez-le dans un éditeur d'images comme
calque par-dessus votre image.

## Vérifier une image

`render` dessine n'importe quelle image sous la forme du dossier qu'en fait l'app, avec le même code,
et indique de quelle catégorie elle relève :

```sh
cargo run -p folderskin-tools -- render ~/Pictures/koi.webp --out /tmp/koi.png --size 512
# wrote /tmp/koi.png (512×512): artwork on FolderSkin's folder
```

Regardez le résultat avant de partager l'image. `--focus x,y` déplace le recadrage d'une
illustration, ce qui permet de voir vite ce qu'un autre recadrage garderait de l'image. L'app, elle,
recadre toujours autour du centre : intégrez donc directement dans l'image le recadrage que vous
voulez. `render --solid RRGGBB` dessine le gabarit dans une couleur unie, pour examiner le gabarit
lui-même.

## Images pour un pack

Les images d'un pack sont partagées sans perte : un pack s'affiche exactement tel que vous l'avez
créé. [PACKS.md](PACKS.md) donne les limites : 1024 px de côté au maximum (la plus grande icône que
dessine chacun des trois systèmes), 256 au minimum, 1,5 Mo au maximum par image et 64 Mo pour tout
le pack. L'app et `packs make` réduisent et encodent chaque image pour vous, en WebP sans perte :

| image | format | pourquoi |
|---|---|---|
| un dossier fini | WebP sans perte, avec sa transparence | chaque pixel tel qu'il est dessiné, bord compris, pour environ deux tiers de la taille d'un PNG |
| une illustration : photos, peintures, dégradés, grain | WebP sans perte | ni blocs ni halos. Une image détaillée de 1024 px pèse environ 800 Ko |
| l'une ou l'autre, trop détaillée pour 1,5 Mo en 1024 px | WebP sans perte en 896, puis en 768 px | plus petite plutôt que floue, et l'app indique lesquelles |

Si vous encodez à la main, un PNG convient aussi, tout comme `cwebp -lossless -z 9`. Le JPEG et le
WebP avec perte ne sont pas acceptés pour un nouveau pack : le service communautaire les refuse, et
`packs check --require-lossless` aussi. Une illustration n'a pas de transparence à conserver,
puisque le gabarit fournit la forme du dossier. Une illustration en 1024 × 958 est celle qui perd le
moins au recadrage, mais toutes les tailles fonctionnent.

Les dossiers finis d'un pack partagent une seule forme, si bien qu'ils ont la même taille côte à
côte. `packs make` et la récupération de la communauté les redessinent à la médiane de leurs formes,
et laissent à une personne le soin de décider pour un dossier trop éloigné
([PACKS.md](PACKS.md#une-seule-forme-pour-les-dossiers-dun-pack)).

## Licences

Ne partagez que des images que vous avez créées ou que vous avez le droit de partager, sous l'une des
licences qu'un pack peut utiliser ([PACKS.md](PACKS.md#licences)). Tout le reste, comme les photos
de banques d'images, les fonds d'écran, les captures d'écran du travail de quelqu'un d'autre ou les
images générées dont vous n'avez pas lu les conditions, reste sur votre ordinateur : déposez-le sur
l'app pour l'utiliser chez vous, où il ne quitte jamais votre machine.
