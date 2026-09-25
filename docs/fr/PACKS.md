# Habillages et packs de la communauté

Tout le monde peut partager gratuitement des habillages avec tous les utilisateurs de FolderSkin.
Une série d'habillages partagée s'appelle un **pack**, et un habillage seul forme un pack à lui tout
seul. Les packs vivent dans leur propre dépôt,
[folderskin-community](https://github.com/prajwal-svm/folderskin-community), dans `packs/`, et en
ajouter un ne demande aucun compte. FolderSkin n'est livré avec aucun habillage : tous viennent des
packs, de vos propres images ou des résultats de l'IA.

## Ajouter un pack

À sa première ouverture, FolderSkin propose des packs pour démarrer votre bibliothèque. Ensuite,
ouvrez **Communauté** dans l'app. Les filtres en haut sont les tags des packs. **Ajouter** place les
habillages d'un pack dans votre bibliothèque, avec les tags du pack, et le menu ⋯ de chaque
habillage indique de quel pack il vient et qui l'a partagé. **Retirer** enlève le pack entier. Les
dossiers qui utilisent déjà l'un de ses habillages gardent leur icône, car l'icône est stockée dans
le dossier lui-même.

**Ajouter depuis un dossier** fait de même avec un dossier de pack présent sur votre ordinateur.
C'est aussi la façon de tester un pack avant de le partager.

La galerie de [folderskin.app](https://folderskin.app/fr/community/) affiche un bouton **Installer**
sur chaque pack. Il ouvre FolderSkin sur ce pack, dans la Communauté, et l'ajoute, exactement comme
son bouton **Ajouter** ([le lien d'installation](#le-lien-dinstallation), plus bas). Les packs
marqués **Officiel** sont ceux dont le mainteneur se porte garant.

Un pack s'ajoute en entier ou pas du tout : chaque image est d'abord téléchargée et vérifiée, puis
toutes sont enregistrées d'un coup. Une connexion coupée ou un disque plein ne laisse donc jamais
un demi-pack dans votre bibliothèque. Ses habillages apparaissent dans l'ordre du pack.

## Partager les vôtres

Vous partagez depuis l'app, et FolderSkin envoie le pack à son service communautaire,
`community.folderskin.app`, où le mainteneur le relit. Pas besoin de compte GitHub. FolderSkin
0.1.6 et les versions précédentes pouvaient aussi ouvrir une pull request sur GitHub pour vous. La
0.1.7 abandonne cette option.

1. Ajoutez un tag aux habillages que vous voulez partager (⋯ → Tags). Pour partager un seul
   habillage, utilisez ⋯ → **Partager avec la communauté**. Pour en partager plusieurs, utilisez
   **Communauté → Partager vos habillages** et choisissez un tag.
2. Indiquez le nom du pack, ses tags et une licence, précisez d'où viennent les images, et cochez
   la case qui confirme que vous avez le droit de les partager.
3. La première fois, FolderSkin vérifie cet ordinateur dans votre navigateur, sous le nom auquel
   vos packs seront crédités. Il ne le demande qu'une fois par ordinateur.
4. Envoyez-le. FolderSkin vérifie que le pack respecte le contrat ci-dessous avant que quoi que ce
   soit ne quitte votre ordinateur. **Vos envois** affiche chaque pack que vous avez envoyé et, si l'un
   d'eux est refusé, la raison.

Chaque image est partagée sans perte : un pack s'affiche exactement tel que vous l'avez créé, bord
transparent des dossiers finis compris. FolderSkin convertit chaque image en WebP sans perte avant
l'envoi, ce qui prend quelques secondes par image, et affiche le décompte au fur et à mesure. Une image trop
détaillée pour tenir dans 1,5 Mo en 1024 px passe à 896 px, puis à 768 px, toujours sans perte, et
FolderSkin indique lesquelles. Les images d'un pack pèsent 64 Mo au maximum. Un pack plus lourd est
refusé, avec la suggestion de le scinder en deux.

Une personne examine chaque pack avant que quiconque puisse le voir. Une fois approuvé, il est
publié automatiquement en 15 minutes environ
([De l'approbation à la publication](#de-lapprobation-à-la-publication), plus bas). Plusieurs packs
peuvent porter le même nom : celui que vous choisissez est celui que tout le monde voit, et le pack
reçoit son propre identifiant ([Identifiants de pack](#identifiants-de-pack)).

Un pack peut aussi être proposé à la main, par une pull request sur
[folderskin-community](https://github.com/prajwal-svm/folderskin-community) qui ajoute un dossier
dans `packs/`. Créez ce dossier avec `packs make` ([Créer un pack à partir
d'images](#créer-un-pack-à-partir-dimages)), qui lui donne un identifiant généré, et la pull
request lance les mêmes vérifications que l'app. **Enregistrer dans un dossier**, dans l'app, écrit
un dossier de pack qui respecte toutes les règles ci-dessous.

## Identifiants de pack

Chaque pack a un identifiant, qui est le nom de son dossier et figure dans chaque lien vers lui.
L'identifiant est créé une seule fois, à la création du pack, à partir de son nom et de six
caractères aléatoires : un pack appelé Classic Art reçoit un identifiant comme `classic-art-k7q2mx`.
Les noms peuvent se répéter librement (cent packs peuvent s'appeler Classic Art) et seul
l'identifiant doit être unique. `packs make` crée les identifiants des packs faits à la main, et le
service communautaire ceux des packs partagés depuis l'app. Un identifiant ne change plus jamais
ensuite, même quand le nom du pack change.

La partie aléatoire compte six caractères de `a` à `z` et de `2` à `7`, tirés par le générateur
aléatoire sécurisé du système. Un nouvel identifiant n'est jamais le nom d'un dossier de `packs/`, ni un
ancien identifiant de `moved.json`.

### moved.json

Les packs créés avant la génération des identifiants avaient un identifiant tiré de leur seul nom,
comme `classic-art`. Ils ont reçu des identifiants générés avec `packs rename`, et `moved.json`, à
côté de `packs/`, enregistre chaque ancien identifiant et l'identifiant actuel du pack :

```json
{ "version": 1, "moved": { "classic-art": "classic-art-k7q2mx" } }
```

- Chaque ancien identifiant est un identifiant de pack qu'aucun dossier de `packs/` ne porte, et il
  n'est jamais attribué à un nouveau pack.
- Chaque nouvel identifiant correspond à un pack de `packs/`. Il ne renvoie jamais vers un autre
  ancien identifiant : renommer à nouveau un pack redirige tout ce qui menait à lui vers son
  identifiant le plus récent.
- `packs index` et `packs catalog` recopient cette table dans `index.json` et `head.json` sous
  `"moved"`, pour que l'app, le site et le service communautaire puissent suivre un pack depuis son
  ancien identifiant. Depuis la 0.1.7, l'app déplace vers le nouvel identifiant les packs que vous
  avez ajoutés sous un ancien, et un lien d'installation avec un ancien identifiant trouve toujours
  son pack.
- Un pack renommé garde sa date de première publication : `packs index` le date d'après le plus
  ancien commit ayant ajouté l'un de ses identifiants.
- Rien de tout cela ne va dans `pack.json`. Il n'accepte aucun champ que le contrat ne nomme pas, et
  les versions 0.1.4 à 0.1.6 de l'app refuseraient le pack.

### packs rename

```sh
cargo run -p folderskin-tools -- packs rename --dir ../folderskin-community --all
cargo run -p folderskin-tools -- packs rename --dir ../folderskin-community classic-art
cargo run -p folderskin-tools -- packs rename --dir ../folderskin-community classic-art --to classic-art-k7q2mx
```

La commande donne des identifiants générés aux packs, dans un clone git de folderskin-community.
Chaque dossier est déplacé avec `git mv`, si bien que son historique le suit. `featured.json` et
`official.json` sont réécrits avec les nouveaux identifiants, dans le même ordre, et `moved.json`
enregistre chaque déplacement (il est créé s'il n'existe pas). Tout est ajouté à l'index, prêt à
être commité.

`--all` renomme chaque pack dont l'identifiant n'est pas généré, et ne touche pas aux autres. Il ne
se fie qu'à la forme : un ancien identifiant dont le dernier mot compte justement six lettres, comme
`data-structures-in-bricks`, a l'air généré, donc renommez ce pack en le désignant par son
identifiant. Désigné seul, un pack passe à un nouvel identifiant généré, quel que soit son
identifiant actuel, ou à celui de `--to`, qui doit être un identifiant généré qu'aucun pack ne porte
et n'a jamais porté. Relancer la commande ne change rien : un pack déjà déplacé est retrouvé dans
`moved.json` et signalé comme tel.

## De l'approbation à la publication

Approuver un pack le publie. Personne ne copie de fichiers à la main.

1. Quand le mainteneur approuve un pack, le service communautaire lui attribue son identifiant. S'il
   dispose d'un jeton GitHub (`GITHUB_DISPATCH_TOKEN`), le service lance aussi immédiatement le
   workflow Packs de folderskin-community, avec un repository dispatch `pack-approved`.
2. Sans jeton, le workflow trouve le pack lui-même. Toutes les 15 minutes, il demande à
   `https://community.folderskin.app/v1/exports/pending` combien de packs approuvés attendent, ce
   qui prend quelques secondes, et ne fait la suite que s'il y en a. Il tourne aussi chaque jour et
   chaque semaine, qu'il y ait quelque chose en attente ou non, et chaque fois que le mainteneur le
   lance à la main.
3. `community pull --no-done` écrit chaque pack approuvé dans `packs/`, et compare la taille et le
   SHA-256 de chaque fichier à ce que le service a enregistré lors de l'envoi. L'identifiant donné
   par le service doit être un identifiant généré, et un dossier déjà présent n'est jamais écrasé
   ni numéroté. Les dossiers finis du pack reçoivent une seule forme ([Une seule forme pour les
   dossiers d'un pack](#une-seule-forme-pour-les-dossiers-dun-pack)) avant la vérification. Un
   dossier trop éloigné de cette forme est gardé tel quel, et le journal de l'exécution le signale
   par un avertissement.
4. `packs check` vérifie chaque pack, comme pour une pull request.
5. Chaque pack est commité par github-actions[bot] avec le message `Add the <name> pack`, puis
   poussé sur `main`.
6. Ce n'est qu'ensuite que `community done` signale au service que le pack est publié. Une
   vérification ou un push qui échoue laisse le pack en attente auprès du service, et l'exécution
   suivante réessaie.
7. La même exécution reconstruit `index.json`, les aperçus et `v2/`, copie `v2/` sur le miroir
   ([voir plus bas](#le-miroir)) et commite le tout. Un push fait avec le jeton du workflow lui-même
   ne déclenche aucun autre workflow : c'est pourquoi tout se passe en une seule exécution.

Un pack est donc publié environ 15 minutes après son approbation, ou en quelques minutes quand le
service lance lui-même le workflow. Une exécution qui échoue ne publie rien, et GitHub prévient le
mainteneur par e-mail de l'échec du workflow. GitHub peut lancer en retard une exécution planifiée
quand il est surchargé, et il désactive la planification dans un dépôt sans activité depuis
60 jours. Le dispatch ne dépend ni de l'un ni de l'autre.

Le workflow a besoin de la clé de signature du mainteneur, c'est-à-dire du fichier entier écrit par
`community keygen`, dans le secret de dépôt `FOLDERSKIN_ADMIN_KEY`. Sans elle, les packs approuvés
attendent auprès du service, et l'exécution le signale. Réglée sur `true`, la
variable de dépôt `REQUIRE_GENERATED_IDS` fait refuser, à chaque vérification, tout pack sans
identifiant généré.

La récupération à la main fonctionne toujours. `community pull` écrit les packs et prévient aussitôt
le service, puisque la personne qui la lance commite ce qu'elle a écrit.
`--no-done --pulled pulled.json` retient ce signal jusqu'à `community done --from pulled.json`, et
c'est ainsi que le workflow la lance. Prévenir le service deux fois ne fait aucun mal.

### Le miroir

L'app peut lire toute l'arborescence depuis `https://packs.folderskin.app`, un bucket Cloudflare R2
qui contient le même `v2/` que le dépôt. C'est le service communautaire qui écrit dans le bucket, si
bien que le workflow n'a besoin d'aucun jeton Cloudflare à lui : `community mirror` envoie chaque
fichier via le service, signé avec la clé du mainteneur.

```sh
cargo run -p folderskin-tools -- community mirror --tree ../folderskin-community \
  --public https://packs.folderskin.app --api https://community.folderskin.app --key ~/folderskin-admin.key
```

La commande interroge le miroir sur chaque fichier par une requête HEAD, et laisse de côté ceux
qu'il sert déjà avec la même taille : chaque nom, sauf `head.json`, est le hash de son contenu. Les
autres sont envoyés avec `PUT /v1/admin/tree/<path>`, chacun avec son SHA-256 dans
`X-Content-SHA256` pour que le bucket le vérifie. `head.json` part en dernier, et seulement une fois
tous les autres fichiers en place, si bien que le miroir ne désigne jamais un catalogue qu'il ne
contient pas. Une requête qui peut réussir d'elle-même (pas de réponse, une erreur 5xx ou 429) est
tentée cinq fois en tout, avec une attente qui double à partir d'une seconde, et un `Retry-After`
est respecté jusqu'à 30 secondes. Le workflow la lance avant de commiter, si bien que le `head.json`
de GitHub ne change qu'une fois que le miroir a tout ce qu'il désigne. La variable de dépôt
`COMMUNITY_MIRROR_URL` active le miroir, et `head.json` ne le mentionne que tant qu'il est actif.

## Le contrat

Un pack, c'est un dossier :

```
packs/night-prints-h4x2qe/
  pack.json
  koi.webp
  fox-in-the-rain.webp
```

`pack.json` :

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

| champ | règle |
|---|---|
| `version` | `1` |
| `name` | de 1 à 40 caractères |
| `author` | votre nom d'utilisateur GitHub |
| `license` | `CC0-1.0`, `CC-BY-4.0` ou `MIT` |
| `tags` | de 1 à 5 tags. Chaque habillage du pack les reçoit, et le premier désigne le pack dans les filtres de tout le monde |
| `skins` | de 1 à 50 entrées |
| `skins[].file` | une image du dossier |
| `skins[].name` | de 1 à 60 caractères |
| `skins[].tags` | facultatif, jusqu'à 3 de plus pour cet habillage |

Aucun autre champ n'est autorisé : une faute de frappe comme `"tag"` fait échouer la vérification au
lieu d'être ignorée.

### Limites

| | limite |
|---|---|
| habillages par pack | de 1 à 50 |
| chaque image | sans perte : PNG, ou WebP sans perte. 1,5 Mo au maximum |
| toutes les images d'un pack | 64 Mo au maximum |
| côtés des images | de 256 à 1024 px |
| noms de fichiers | lettres, chiffres, `.`, `-` et `_`, terminés par `.png` ou `.webp` |
| identifiant, le nom du dossier | un nom et six caractères aléatoires, comme `night-prints-h4x2qe` ([Identifiants de pack](#identifiants-de-pack)) : des lettres minuscules et des chiffres, en mots reliés par des tirets simples, 40 caractères au maximum |
| tags | lettres minuscules, chiffres, espaces et tirets, 24 caractères au maximum |
| `pack.json` | 64 Ko au maximum |

Pourquoi 50 et 64 Mo : un pack est une série thématique, et chaque personne qui l'ajoute le
télécharge en entier. Cinquante habillages restent rapides à relire, et 64 Mo contiennent les
cinquante à 1,3 Mo l'image, ou quarante-deux à la taille maximale.

Les packs publiés avant FolderSkin 0.1.7 étaient limités à 2 Mo par image, en PNG, JPEG ou WebP de
tout type, et l'app les lit toujours. Refaits avec `packs make`, ils suivent les règles ci-dessus.
`packs check` impose ces règles avec `--require-lossless` ([Vérifier un pack
vous-même](#vérifier-un-pack-vous-même)), et le service communautaire n'accepte rien d'autre.

### Images

Chaque image appartient à l'une des deux catégories suivantes, distinguées de la même façon que pour
une image déposée sur la fenêtre :

- **Un dossier fini**, sur fond transparent ou sur un magenta uni `#FF00FF` que FolderSkin retire.
  Il devient l'icône exactement tel qu'il est dessiné.
- **Tout le reste** est plaqué sur le dossier de FolderSkin. [SKINS.md](SKINS.md) montre où le
  dossier recadre une image, pour que le sujet reste visible.

Aucun des trois systèmes ne dessine d'icône de plus de 1024 px : une image plus grande n'apporte
rien.

Chaque image est sans perte, si bien qu'un pack s'affiche exactement tel qu'il a été créé : pas de
blocs dans un dégradé, pas de halos autour des lettres, et un bord de dossier fini aussi net qu'au
dessin. Un WebP sans perte est environ un tiers plus léger que le même PNG, c'est pourquoi l'app et
`packs make` écrivent du WebP. Une image détaillée de 1024 px pèse entre 0,6 et 1,5 Mo, la plupart
autour de 800 Ko. Celle qui ne tient pas dans 1,5 Mo passe à 896 px, puis à 768 px, toujours sans
perte, plutôt que d'être floutée pour tenir.

### Une seule forme pour les dossiers d'un pack

FolderSkin fait tenir toute l'image d'un dossier fini dans l'icône. Les dossiers créés un par un
sortent recadrés au plus près, et deux rendus n'ont jamais tout à fait les mêmes proportions : dans
un pack, ils allaient de 1,03 à 1,30 fois plus larges que hauts. Côte à côte dans le Finder, les plus
trapus paraissaient plus petits que les plus hauts. Les dossiers finis d'un pack partagent donc une
seule forme :

- **La forme du pack** est la médiane de celles de ses dossiers. Un dossier se mesure sur la partie
  de son image qui est plus qu'à moitié opaque, largeur ÷ hauteur, si bien qu'un bord adouci ou une
  ombre légère ne comptent pas.
- **Chaque dossier est redessiné exactement à cette forme.** Il est recadré sur le dossier et
  redimensionné à 962 px de large, soit la largeur du dossier de FolderSkin dans son gabarit de
  1024 px (`folderskin-tools template`), et à la hauteur que donne la forme. Il est ensuite placé à
  l'endroit du dossier du gabarit, sur le même bord gauche et posé sur la même ligne de base, dans
  une image transparente de 1024 × 1024, puis enregistré en WebP sans perte. Un PNG devient un
  `.webp` du même nom, et `pack.json` suit.
- **Un dossier est déformé de 8 % au maximum.** Personne ne remarque un tel écart. Un dossier qui
  en demanderait plus est une *exception* : étirés à ce point, les lettres et les visages paraissent
  écrasés, donc il n'est jamais déformé. Ce qu'il en advient dépend de la commande, et c'est une
  personne qui décide.
- **Les illustrations ne sont pas touchées.** FolderSkin les plaque sur son propre dossier : elles
  n'ont pas de forme à corriger. Un pack qui ne compte qu'un seul dossier fini n'est pas touché non
  plus.

`packs make` fait ce travail pour chaque pack qu'il crée, `community pull` pour chaque pack qu'il
récupère, et `packs normalize` pour les packs déjà présents dans `packs/`. Le refaire ne change
rien : un dossier déjà à la forme de son pack, et à sa place, n'est jamais redessiné, et un fichier
qui contient déjà ce qu'il recevrait n'est pas réécrit.

| commande | une exception |
|---|---|
| `packs make` | écartée du pack, et listée. `--keep-outliers` la garde telle quelle |
| `packs normalize` | listée, et laissée telle quelle. `--drop-outliers` la retire de `pack.json` et supprime son image |
| `community pull` | gardée telle quelle, avec un avertissement dans la sortie, visible dans le journal du workflow Packs. Un habillage partagé par quelqu'un n'est jamais écarté sans qu'une personne le décide |

```sh
cargo run -p folderskin-tools -- packs normalize --dir ../folderskin-community
cargo run -p folderskin-tools -- packs normalize --dir ../folderskin-community classic-art-5rxas2 --tolerance 0.3
cargo run -p folderskin-tools -- packs normalize --dir ../folderskin-community dreamscapes-ppfia6 --drop-outliers
```

`packs normalize` passe en revue chaque pack, ou ceux qu'on lui désigne. Pour chacun, il indique la
forme et ce qu'il a redessiné, et nomme chaque exception en précisant son écart. Un pack qui ne passe
pas `packs check` est laissé tel quel jusqu'à ce qu'il le passe. `--tolerance` modifie les 8 %, pour
un pack dont les exceptions doivent quand même prendre la forme, comme la Joconde dans Classic Art.
`--drop-outliers` est réservé au mainteneur : rien d'autre ne retire jamais un habillage.
`packs check --require-one-shape` refuse un pack dont les dossiers diffèrent de plus de 1 %
([Vérifier un pack vous-même](#vérifier-un-pack-vous-même)). Des dossiers redessinés à une même
forme ne dépassent jamais cet écart.

### Licences

Les habillages partagés utilisent une licence Creative Commons ou MIT :

- `CC0-1.0` : tout le monde peut les utiliser pour n'importe quel usage. C'est la licence par
  défaut, car la plupart des habillages sont faits avec l'IA et CC0 est celle qui revendique le
  moins de droits sur eux.
- `CC-BY-4.0` : tout le monde peut les utiliser, en vous créditant.
- `MIT` : tout le monde peut les utiliser, et votre nom les accompagne.

Ne partagez que des images que vous avez créées ou que vous avez le droit de partager.

## Créer un pack à partir d'images

`packs make` transforme un dossier d'images, par exemple des rendus enregistrés depuis un modèle
d'image, en un pack qui passe déjà les vérifications, dans le `packs/` d'un clone de
folderskin-community. Les commandes ci-dessous se lancent depuis ce dépôt, avec folderskin-community
cloné à côté :

```sh
cargo run -p folderskin-tools -- packs make ~/Downloads/3d-renders --dir ../folderskin-community \
  --name "3D" --tags 3d,glossy --author your-github-name --preview /tmp/3d.png
```

Le pack reçoit son propre identifiant, son nom suivi de six caractères aléatoires, comme
`3d-k7q2mx`, et c'est le nom de son dossier. Le rapport l'indique. L'identifiant n'est jamais celui
d'un dossier de `packs/`, ni un ancien identifiant de `moved.json`.

Chaque image est triée comme l'app le fait quand vous en ajoutez une. Un dossier fini, peint sur
magenta comme le demande le prompt pour chat de [PROMPTS.md](PROMPTS.md), ou sur une vraie
transparence, est détouré et devient l'icône elle-même. Tout le reste est une illustration pour le
dossier de FolderSkin. Chaque image est réduite à 1024 px et enregistrée en WebP sans perte, avec
l'encodeur intégré que l'app utilise pour partager les packs : il n'y a rien à installer. Une image
qui dépasse encore 1,5 Mo passe à 896 px, puis à 768 px, et le rapport le signale. `--max-kb` limite
les images à moins de 1,5 Mo. Des images qui dépassent 64 Mo au total sont refusées, avec la
suggestion de les répartir en deux packs. Le réglage le plus poussé de libwebp prend plusieurs
secondes par image, donc elles sont traitées sur tous les cœurs à la fois.
[SKINS.md](SKINS.md#images-pour-un-pack) en dit plus sur les formats. Le rapport indique quel chemin
a pris chaque image. Les habillages prennent le nom de leurs fichiers : nommez donc les fichiers
d'abord, ou corrigez les noms dans `pack.json` ensuite. `--preview` dessine chaque habillage sous
forme de dossier dans un seul PNG, pour tout passer en revue.

Dès qu'un pack compte deux dossiers finis ou plus, ceux-ci reçoivent une seule forme ([Une seule
forme pour les dossiers d'un pack](#une-seule-forme-pour-les-dossiers-dun-pack)), et le rapport indique lesquels ont été
redessinés. Un dossier qui s'écarte de plus de 8 % de la forme des autres est écarté, et le rapport
précise de combien. `--keep-outliers` le garde plutôt tel quel. Un pack créé sans garder
d'exception passe `packs check --require-one-shape`.

`--id` refait un pack existant à partir de nouvelles images : `--id 3d-k7q2mx` remplace tout le
contenu de `packs/3d-k7q2mx/`, et le pack garde son identifiant, si bien que tous ceux qui l'ont
ajouté reçoivent la nouvelle version comme une mise à jour. Le nouveau dossier est d'abord créé et
vérifié à part, et ne prend la place de l'ancien que s'il passe les vérifications. Sans `--id`,
`packs make` crée toujours un nouveau pack.

Les modèles d'image à qui l'on demande du `#FF00FF` peignent souvent à la place un aplat framboise
ou rose vif (Grok l'a fait pour le pack Classic Art). `--flat-backdrop` retire un fond uni de
n'importe quelle couleur : il mesure le fond propre à chaque image, n'enlève que ce qui touche le
bord (une cape rouge à l'intérieur du dossier reste donc en place), emporte au passage une ombre
portée douce, et donne au bord les couleurs du tableau plutôt qu'un liseré rose. Sur un fond gris ou
noir uni, il s'en tient au bruit propre à ce fond et ne remonte jamais, si bien qu'un manteau sombre
ou un trait d'encre qui touche le bord du dossier n'est pas pris pour le fond. Regardez ensuite la
planche `--preview` (elle est dessinée sur un gris clair, pour qu'un trou se voie). Une image sans
fond uni ressort toujours comme une illustration.

Pour voir une image comme l'app l'affichera, `render` la dessine sous forme de dossier :

```sh
cargo run -p folderskin-tools -- render ../folderskin-community/packs/3d-k7q2mx/glass.webp --out /tmp/glass.png --size 512
```

## Vérifier un pack vous-même

Depuis ce dépôt, avec folderskin-community cloné à côté :

```
cargo run -p folderskin-tools -- packs check --dir ../folderskin-community
```

La commande vérifie chaque dossier de `packs/` avec les règles qu'utilise l'app, et décrit chaque
problème en une phrase. Lancée dans un clone de folderskin-community, elle peut se passer de
`--dir` : les outils regardent par défaut dans le dossier courant. Elle limite chaque image aux
2 Mo qui s'appliquaient aux packs publiés avant la 0.1.7, et les images de chaque pack à
64 Mo au total. `--require-lossless` soumet chaque image aux règles que suivent les nouveaux packs :
PNG ou WebP sans perte, 1,5 Mo au maximum, c'est-à-dire ce qu'accepte le service communautaire et
ce qu'écrit `packs make`. Cette option est désactivée sauf demande, le temps que les anciens packs
soient refaits. `--max-kb` impose une limite plus basse.

Elle vérifie aussi les identifiants entre eux : deux dossiers dont les noms ne diffèrent que par les
majuscules ne font qu'un sur macOS et Windows, et un pack ne peut pas prendre un ancien identifiant
de `moved.json`, qui doit respecter ses propres règles ([moved.json](#movedjson)). Les noms ne sont
jamais comparés, puisqu'ils peuvent se répéter. `--require-generated-ids` refuse tout pack dont
l'identifiant n'est pas généré. L'option est désactivée sauf demande, et le workflow de
folderskin-community la demande quand sa variable `REQUIRE_GENERATED_IDS` vaut `true`.

`--require-one-shape` refuse un pack dont les dossiers finis n'ont pas une seule forme, c'est-à-dire
dont deux dossiers diffèrent de plus de 1 % en largeur ÷ hauteur. Des dossiers redessinés à une
même forme restent toujours dans cette marge : l'option repère donc un pack qui n'a jamais reçu de
forme commune, ou une exception que quelqu'un a gardée. Le message nomme les deux dossiers les plus
éloignés et indique quoi lancer ([Une seule forme pour les dossiers
d'un pack](#une-seule-forme-pour-les-dossiers-dun-pack)). L'option est désactivée sauf demande, et
prévue pour le workflow de folderskin-community.

## Comment l'app lit les packs

La Communauté propose une vue **Liste** et une vue **Galerie**, et **Voir** ouvre n'importe quel
pack : chaque habillage y est dessiné sous forme de dossier, avec son nom, avant tout ajout.
**Actualiser** relit la liste. Un pack que vous avez ajouté et qui a changé depuis affiche **Mettre
à jour**, qui remplace ses habillages par la nouvelle version. Les dossiers gardent leurs icônes, et
une image présente dans les deux versions reste en favori si elle l'était.

- `index.json`, dans folderskin-community, liste chaque pack : son identifiant, son nom, son auteur,
  sa licence, ses tags, son nombre d'habillages et un hash de son contenu exact (`pack.json` et
  chaque image). L'app garde ce hash avec les habillages qu'elle ajoute : c'est ainsi qu'elle sait
  qu'un pack a une mise à jour. Chaque entrée indique aussi la date de première publication du pack
  (`"added"`, en secondes Unix : l'heure du commit qui a ajouté son `pack.json`, sous son premier
  identifiant) et, pour un pack listé dans `official.json`, `"official": true`. `"moved"`, à côté
  des packs, correspond à [moved.json](#movedjson). `folderskin-tools packs index` écrit ce fichier,
  ainsi que `previews/<id>.png`, une bande qui montre les quatre premiers habillages du pack sous
  forme de dossiers. Les deux sont générés sur la branche `main` de folderskin-community : ne les
  modifiez jamais à la main.
- `v2/`, qu'écrit `packs catalog`, présente les mêmes packs sous forme d'un catalogue dans lequel
  l'app cherche sur votre ordinateur. Son `head.json` désigne le catalogue actuel, liste les packs
  `featured` et `official`, contient `moved`, et liste les miroirs qui servent la même arborescence,
  comme `https://packs.folderskin.app` ([Le miroir](#le-miroir)). L'app récupère chaque fichier
  d'abord sur les miroirs, puis sur GitHub s'ils échouent, et vérifie chacun d'après son hash dans
  les deux cas. Depuis la 0.1.7, elle lit `head.json` lui-même d'abord sur
  `https://packs.folderskin.app`.
- L'app ne télécharge les images d'un pack que lorsque vous l'ajoutez, quatre à la fois, et affiche
  combien sont arrivées. Elle vérifie chacune d'après les limites ci-dessus et n'enregistre rien tant
  qu'elles ne passent pas toutes. Elle les enregistre ensuite ensemble : un pack n'est jamais ajouté
  à moitié.
- `FOLDERSKIN_COMMUNITY_URL` fait pointer l'app vers une autre copie de folderskin-community. Par
  exemple, servez un clone avec `python3 -m http.server` depuis sa racine et réglez la variable sur
  `http://localhost:8000` pour tester un pack de bout en bout.

`index.json` et `head.json` gagnent des champs avec le temps, et chaque version de l'app lit ceux
qu'elle connaît et ignore les autres. `pack.json`, c'est l'inverse : il n'accepte aucun champ que le
contrat ne nomme pas, donc rien ne doit jamais y être ajouté. Toute nouvelle information sur un pack
va dans l'index.

## Packs à la une et packs officiels

Deux listes se trouvent à côté de `packs/`, à la racine de folderskin-community, et seul son
mainteneur les modifie. Chacune est une liste JSON d'identifiants de packs, comme
`["classic-art-k7q2mx", "colours-a2b3c4"]` :

| fichier | rôle |
|---|---|
| `featured.json` | les packs que propose le premier lancement et que la Communauté affiche en premier, dans cet ordre |
| `official.json` | les packs marqués **Officiel** dans la Communauté, dans la vue d'un pack et sur le site |

Les deux sont facultatives. Chaque identifiant doit correspondre à un pack de `packs/`, et un
identifiant listé deux fois ne compte qu'une fois. `packs index` et `packs catalog` s'arrêtent sans
rien écrire quand l'une des deux listes nomme un pack absent : un pack renommé ou supprimé ne peut
donc pas laisser de trou. La vérification des pull requests lance `packs catalog`, qui repère le
problème avant la fusion. `packs rename` réécrit lui-même les deux listes. Le workflow Packs de
folderskin-community reconstruit l'index quand un fichier de ses `paths` change : les deux listes y
ont donc leur place, à côté de `packs/**`, tout comme `moved.json`.

## Le lien d'installation

`folderskin://install?pack=<id>` ouvre FolderSkin sur le pack `<id>` dans la Communauté et l'ajoute,
exactement comme son bouton **Ajouter**, avec la même progression et le même message à la fin. La
fenêtre passe d'abord au premier plan. Si la Communauté n'a aucun pack avec cet identifiant, même
après avoir interrogé à nouveau GitHub, FolderSkin le signale et propose de le rechercher. Un pack
déjà présent dans votre bibliothèque est ouvert, avec un message qui le précise. Depuis la 0.1.7, un
lien avec un ancien identifiant ouvre le pack vers lequel il a été déplacé ([moved.json](#movedjson)).

FolderSkin n'accepte un lien que s'il a exactement cette forme : le schéma `folderskin`, `install`
comme hôte (`folderskin://install?…`) ou comme chemin entier (`folderskin:install?…`), sans
utilisateur, mot de passe ni port, et exactement un `pack`, qui doit être un identifiant de pack
(des lettres minuscules et des chiffres, en mots reliés par des tirets simples, 40 caractères au
maximum). Tout autre paramètre est laissé de côté, et tout autre lien est ignoré.

Les installateurs enregistrent le schéma : le `Info.plist` de l'app macOS, les installateurs Windows
et l'entrée de bureau des `.deb` et `.rpm` Linux. Une AppImage l'enregistre à son démarrage, puisque
rien ne l'installe. Sous Windows et Linux, un lien lance un second FolderSkin, qui transmet le lien à
celui qui tourne déjà puis se ferme : il n'en tourne donc jamais qu'un seul.

Pour l'essayer :

| | |
|---|---|
| macOS | Compilez l'app (`pnpm tauri build --bundles app`) et ouvrez une fois `target/release/bundle/macos/FolderSkin.app`, ce qui enregistre le schéma auprès de macOS (une copie dans `/Applications` est le plus sûr), puis lancez `open 'folderskin://install?pack=classic-art'`. macOS n'envoie les liens qu'à une app empaquetée, donc `pnpm tauri dev` n'en reçoit jamais |
| Windows | Installez une version compilée, ou lancez `pnpm tauri dev` (une version de développement enregistre le schéma pour elle-même), puis `start "" "folderskin://install?pack=classic-art"` dans une invite de commandes, ou le même lien dans la boîte Exécuter (Windows+R) |
| Linux | Installez le `.deb` ou le `.rpm`, démarrez l'AppImage une fois, ou lancez `pnpm tauri dev`, puis `xdg-open 'folderskin://install?pack=classic-art'` |
| aperçu dans le navigateur | `pnpm dev` et ouvrez `http://localhost:14200/?install=classic-art` |

Quittez le FolderSkin installé avant `pnpm tauri dev` sous Windows ou Linux : si un exemplaire tourne
déjà, le nouveau lui passe la main et se ferme. Une version de développement qui a enregistré le
schéma le garde jusqu'à ce qu'un installateur ou une autre version compilée l'enregistre à nouveau.

## Nombre d'installations

Quand un pack de la Communauté a été ajouté, FolderSkin communique l'identifiant du pack à son
service communautaire : `POST https://community.folderskin.app/v1/packs/<id>/installs`, sans corps
de requête. C'est tout ce qu'il envoie : aucun compte, aucun identifiant d'appareil, rien sur votre
bibliothèque ni sur vos dossiers (comme toutes les requêtes de FolderSkin, celle-ci indique la
version de l'app dans son User-Agent). L'envoi a lieu après l'enregistrement du pack, abandonne au
bout de cinq secondes, et rien ne l'attend ni ne signale d'échec. Une version de développement, ou
une version qui lit les packs depuis une autre copie (`FOLDERSKIN_COMMUNITY_URL`), n'envoie rien,
sauf si `FOLDERSKIN_COMMUNITY_API` désigne un service destinataire.

Le service compte un ajout une fois par jour pour chaque réseau et chaque pack, et seulement pour
les packs de l'`index.json` publié. Un ajout sous un ancien identifiant compte pour le pack vers
lequel il a été déplacé, si bien que les versions de l'app antérieures à la 0.1.7 sont toujours
comptées. Le service garde un compteur par pack et, jusqu'à la fin de la journée UTC, un hash salé
du réseau d'où vient la requête, pour qu'un nouvel ajout du même pack ce jour-là ne compte pas deux
fois. Le nettoyage quotidien supprime ces hashs. Aucune adresse n'est conservée.

folderskin.app lit les compteurs depuis `GET https://community.folderskin.app/v1/packs/installs` :
`{"version": 1, "installs": {"classic-art-k7q2mx": 42}}`, mis en cache pendant cinq minutes.
[services/community/README.md](../../services/community/README.md) donne les détails (en anglais).
