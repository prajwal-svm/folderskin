<div align="center">

<img src="public/app-icon.png" alt="Logo de FolderSkin" width="112" height="112" />

# FolderSkin

[English](README.md) · [简体中文](README.zh-CN.md) · [日本語](README.ja.md) · [한국어](README.ko.md) · Français · [Español](README.es.md)

[![Télécharger pour macOS](https://img.shields.io/badge/Download_for_macOS-111827?style=for-the-badge&logo=apple&logoColor=white)](https://github.com/prajwal-svm/folderskin/releases/latest)
[![Télécharger pour Windows](https://img.shields.io/badge/Download_for_Windows-3A86FF?style=for-the-badge&logo=data:image/svg%2bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCA0NDggNTEyIj48cGF0aCBmaWxsPSIjZmZmIiBkPSJtMCA5My43bDE4My42LTI1LjN2MTc3LjRIMHptMCAzMjQuNmwxODMuNiAyNS4zVjI2OC40SDB6bTIwMy44IDI4TDQ0OCA0ODBWMjY4LjRIMjAzLjh6bTAtMzgwLjZ2MTgwLjFINDQ4VjMyeiIvPjwvc3ZnPg==)](https://github.com/prajwal-svm/folderskin/releases/latest)
[![Télécharger pour Linux](https://img.shields.io/badge/Download_for_Linux-FCC624?style=for-the-badge&logo=linux&logoColor=111827)](https://github.com/prajwal-svm/folderskin/releases/latest)

[![CI](https://github.com/prajwal-svm/folderskin/actions/workflows/ci.yml/badge.svg)](https://github.com/prajwal-svm/folderskin/actions/workflows/ci.yml) <!-- Badges SonarQube Cloud, masqués tant que le projet n'y existe pas (docs/CI.md) : [![Quality gate](https://sonarcloud.io/api/project_badges/measure?project=prajwal-svm_folderskin&metric=alert_status)](https://sonarcloud.io/summary/new_code?id=prajwal-svm_folderskin) [![Couverture](https://sonarcloud.io/api/project_badges/measure?project=prajwal-svm_folderskin&metric=coverage)](https://sonarcloud.io/component_measures?id=prajwal-svm_folderskin&metric=coverage) --> [![Téléchargements](https://img.shields.io/endpoint?url=https%3A%2F%2Fraw.githubusercontent.com%2Fprajwal-svm%2Ffolderskin%2Fbadges%2Fdownloads.json)](https://github.com/prajwal-svm/folderskin/releases) [![Version](https://img.shields.io/github/package-json/v/prajwal-svm/folderskin?label=version&color=3A86FF)](CHANGELOG.md) [![Dernier commit](https://img.shields.io/github/last-commit/prajwal-svm/folderskin?label=last%20commit&color=3A86FF)](https://github.com/prajwal-svm/folderskin/commits/main) [![Licence : GPL-3.0](https://img.shields.io/badge/license-GPL--3.0-3A86FF)](LICENSE) [![Conçu avec Tauri 2](https://img.shields.io/badge/built%20with-Tauri%202-24C8DB?logo=tauri&logoColor=white)](https://tauri.app) [![Packs de la communauté bienvenus](https://img.shields.io/badge/community%20packs-welcome-12b981)](https://github.com/prajwal-svm/folderskin-community) [![Étoiles](https://img.shields.io/github/stars/prajwal-svm/folderskin?style=social)](https://github.com/prajwal-svm/folderskin)

**Habillez n'importe quel dossier.**

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/images/folder-cycle-dark.gif" />
  <img src="docs/images/folder-cycle.gif" alt="Un même dossier essaie douze habillages l'un après l'autre : un mont Fuji en papier découpé, La Nuit étoilée, du pop art, de l'aquarelle, une affiche de voyage et d'autres encore" width="360" />
</picture>

Vos plus beaux souvenirs dorment dans le même dossier terne que vos vieilles paperasses. FolderSkin
donne à chaque dossier un habillage à l'image de ce qu'il contient : un plan de cinéma à l'heure dorée pour
les photos de l'été, une affiche de voyage vintage pour un séjour, du pop art pour un projet vidéo,
des pastels tout doux pour un anniversaire. Déposez un dossier sur la fenêtre, essayez plusieurs
habillages et appliquez celui qui vous plaît. Piochez dans les packs gratuits de la communauté,
utilisez une de vos photos, créez le vôtre à partir d'une couleur, d'un mot ou d'un emoji, ou
décrivez le style de votre choix et laissez l'IA le peindre.

<p>
  <a href="https://github.com/prajwal-svm/folderskin/releases/latest"><strong>Télécharger gratuitement</strong></a>
  · <a href="https://folderskin.app/fr/">folderskin.app</a>
  · <a href="https://folderskin.app/fr/community/">Packs de la communauté</a>
  · <a href="docs/fr/PACKS.md">Partager un pack d'habillages</a>
  · <a href="#compiler-depuis-les-sources">Compiler depuis les sources</a>
  · <a href="https://github.com/prajwal-svm/folderskin">⭐ Mettre une étoile sur GitHub</a>
</p>

Gratuit · Open source · Sans compte · Sans pistage

</div>

<div align="center">
  <img src="docs/images/app.webp" alt="FolderSkin 0.1.7 sur macOS : le pack Scientists Pop Art dans la bibliothèque, et Isaac Newton essayé sur le dossier Téléchargements" width="100%" />
</div>

<details><summary>Mode sombre</summary>

![FolderSkin en mode sombre](docs/images/app-dark.webp)

</details>

## Pour commencer

| Vous voulez… | Faites ceci |
| --- | --- |
| Obtenir vos premiers habillages | Au premier lancement, FolderSkin propose les packs de la communauté, avec Classic Art déjà sélectionné |
| Changer l'apparence d'un dossier | Déposez le dossier sur la fenêtre, cliquez sur un habillage, puis sur **Appliquer l'habillage** |
| Habiller aussi les dossiers qu'il contient | Activez **Inclure les sous-dossiers** sous le dossier, puis **Appliquer à** tous |
| Utiliser une de vos photos | Déposez l'image sur la fenêtre, ou cliquez sur **Ajouter une photo** |
| Créer le vôtre | **Créer le vôtre** : partez d'une couleur, d'une étiquette, d'un emoji ou d'une photo, modifiez tout ce que vous voulez, puis **Enregistrer et appliquer** |
| Faire peindre un habillage par une IA | **Générer avec l'IA**, avec votre propre clé d'API, ou le prompt pour le chat de Grok ou de ChatGPT sous **Pas de clé d'API ?** |
| Obtenir des habillages créés par d'autres | **Communauté**, puis ajoutez un pack |
| Partager les vôtres | Menu ⋯ d'un habillage → **Partager avec la communauté** |
| Retrouver un habillage | Les tags en haut, ⌘F / Ctrl+F, le bouton de filtre (couleur, pack, date d'ajout, etc.), ou l'étoile pour les **Favoris** |
| Revenir en arrière | **Rétablir**, ou **Retirer l'icône personnalisée** sur un dossier qui en a déjà une, remet l'icône du système |

## Ce qui reste sur votre ordinateur

Tout, sauf les rares choses qui ont besoin d'Internet. Pas de compte, rien à payer, rien qui vous
piste : la seule chose que FolderSkin signale, c'est l'ajout d'un pack de la communauté, par son
seul identifiant, pour que folderskin.app puisse afficher combien de fois chaque pack est ajouté.
Sur Mac, le téléchargement fait moins de 20 Mo.

| Reste sur votre ordinateur | Passe par Internet |
| --- | --- |
| Vos dossiers et les icônes que FolderSkin y écrit | Une requête à une IA, quand vous en faites une : elle part chez le fournisseur que vous avez choisi, avec votre clé |
| Chaque image que vous ajoutez et chaque habillage que vous créez | La Communauté et le premier lancement, qui lisent les packs partagés sur packs.folderskin.app, ou sur GitHub quand il ne répond pas |
| Vos clés d'API, chiffrées | La recherche de mises à jour : à l'ouverture, FolderSkin lit sur GitHub le fichier qui indique la dernière version |
| Favoris, tags et réglages | L'ajout d'un pack depuis la Communauté : son identifiant, et rien d'autre, part vers le service communautaire de FolderSkin, qui compte les ajouts une fois par jour et par réseau et ne conserve aucune adresse ([détails](docs/fr/PACKS.md#nombre-dinstallations)) |

La [politique de confidentialité](https://folderskin.app/fr/privacy/) détaille tout ce que FolderSkin
envoie, à qui, et ce que conserve le service communautaire.

## Mode d'emploi

À sa première ouverture, FolderSkin lance une courte animation de bienvenue, puis propose les packs
d'habillages de la communauté, avec Classic Art déjà sélectionné. Ajoutez ceux qui vous plaisent, ou
aucun : chaque pack s'ajoute en entier ou pas du tout, et vous pourrez toujours remplir la
bibliothèque plus tard depuis **Communauté**. L'accueil ne s'affiche qu'une seule fois.

<details><summary>Le premier lancement</summary>

![Le premier lancement propose les packs de la communauté, avec Classic Art sélectionné](docs/images/first-launch.png)

</details>

1. Faites glisser un dossier sur le panneau du dossier, à droite, ou cliquez sur le dossier vide pour
   en choisir un.
2. Cliquez sur un habillage de la bibliothèque pour l'essayer. Le panneau du dossier affiche aussitôt
   le résultat. Dans la barre latérale, choisissez **Tous les habillages**, **Mes habillages** ou
   **Favoris**. Les tags en haut affinent la sélection, et ⌘F / Ctrl+F lance une recherche.
3. Cliquez sur **Appliquer l'habillage**. Le dossier est marqué **Appliqué**, et **Afficher dans le
   Finder** l'ouvre.
4. Cliquez sur **Rétablir** pour remettre l'icône par défaut du système. Si le dossier a déjà une
   icône personnalisée, **Retirer l'icône personnalisée** apparaît dès que vous le choisissez.

Pour donner le même habillage aux dossiers qu'il contient, activez **Inclure les sous-dossiers** sous
le nom du dossier. FolderSkin les compte d'abord (à tous les niveaux, sans les dossiers cachés ni les
paquets d'applications), et le bouton devient **Appliquer à 25 dossiers**, avec le nombre trouvé. Au-delà
de dix, il demande confirmation avant de commencer. Le panneau du dossier montre chaque dossier au
fur et à mesure, **Arrêter** interrompt l'opération après le dossier en cours, et le récapitulatif
indique ce qui a changé, quels dossiers n'ont pas pu l'être et pourquoi, puis propose de continuer ou
de réessayer ceux-là. **Tout rétablir** retire exactement ce que l'opération a posé.

Pour utiliser votre propre image, déposez-la sur la fenêtre ou cliquez sur **Ajouter une photo**.
Elle est enregistrée dans **Mes habillages** et y reste jusqu'à ce que vous la supprimiez, ce que
FolderSkin vous fait confirmer. Un dossier fini sur fond magenta uni, comme ceux que produit le
prompt pour chat ci-dessous, est détouré et utilisé tel quel. Toute autre image est plaquée sur le
dossier de FolderSkin.

Chaque habillage que vous ajoutez a un menu ⋯ qui permet de le renommer (un double-clic sur son nom,
ou F2, y mène directement), de lui ajouter des tags (ils deviennent des filtres en haut), de voir
comment il a été créé (le modèle d'IA et le prompt, ou le pack et la personne qui l'a partagé), de le
partager ou de le supprimer. Les résultats de l'IA reçoivent dès leur arrivée un tag qui indique leur
style, comme `airbrush`.

**Réglages**, en bas de la barre latérale, regroupe le thème, vos clés d'API, les informations que
le partage préremplit et l'endroit où vos habillages sont enregistrés. Survolez le badge de version, à
côté du logo, pour afficher « À propos ».

## Habillages de la communauté

<table>
  <tr>
    <td align="center"><a href="https://folderskin.app/fr/community/?pack=classic-art-5rxas2"><img src="docs/images/packs/classic-art.webp" alt="Classic Art sur quatre dossiers : la Joconde, La Jeune Fille à la perle, La Neuvième Vague et Le Voyageur contemplant une mer de nuages" width="340" /><br /><b>Classic Art</b></a></td>
    <td align="center"><a href="https://folderskin.app/fr/community/?pack=scientists-pop-art-nb3dfx"><img src="docs/images/packs/scientists-pop-art.webp" alt="Scientists - Pop Art sur quatre dossiers : Marie Curie, Ada Lovelace, Nikola Tesla et Srinivasa Ramanujan en portraits façon bande dessinée" width="340" /><br /><b>Scientists - Pop Art</b></a></td>
  </tr>
  <tr>
    <td align="center"><a href="https://folderskin.app/fr/community/?pack=watercolour-world-rt2klu"><img src="docs/images/packs/watercolour-world.webp" alt="Watercolour World sur quatre dossiers : Kyoto, Venise, le Machu Picchu et Marrakech à l'aquarelle" width="340" /><br /><b>Watercolour World</b></a></td>
    <td align="center"><a href="https://folderskin.app/fr/community/?pack=countries-in-paper-lhadao"><img src="docs/images/packs/countries-in-paper.webp" alt="Countries in Paper sur quatre dossiers : l'Inde, le Mexique, le Kenya et l'Islande en papier découpé" width="340" /><br /><b>Countries in Paper</b></a></td>
  </tr>
</table>

FolderSkin n'est livré avec aucun habillage. Ce sont les utilisateurs qui partagent des habillages et
des packs d'habillages via FolderSkin, gratuitement pour tout le monde. Le premier lancement vous les
propose, et vous les retrouvez à tout moment dans **Communauté**. Ajouter un pack place ses
habillages dans
votre bibliothèque avec leurs tags, et le bouton **Installer** d'un pack, dans la galerie de
[folderskin.app](https://folderskin.app/fr/community/), ouvre FolderSkin et l'ajoute pour vous. Les
packs marqués **Officiel** sont ceux dont le mainteneur se porte garant. **Classic Art** est un bon
premier pack : seize tableaux du domaine public, de la Joconde à La Nuit étoilée, chacun peint sur
un dossier. Pour partager les vôtres, ouvrez le menu ⋯ d'un habillage et choisissez **Partager avec
la communauté**, ou passez par **Communauté → Partager vos habillages** pour en partager plusieurs.
Vous vérifiez votre ordinateur une seule fois dans le navigateur, sans compte, et une personne relit
le pack avant qu'il rejoigne la Communauté pour tout le monde. **Enregistrer dans un dossier**, dans
la même fenêtre, écrit plutôt le pack sous forme de fichiers. Les images sont partagées sans perte :
un pack s'affiche exactement tel que vous l'avez créé.
[docs/fr/PACKS.md](docs/fr/PACKS.md) décrit le contrat et ses limites : de 1 à 50 habillages par
pack, des images de 1024 px et 1,5 Mo au maximum, 64 Mo par pack, sous licence CC0, CC BY 4.0 ou MIT.

## Créer le vôtre

**Créer le vôtre**, dans la barre latérale, fabrique un habillage de A à Z, hors ligne. Partez d'une
couleur unie, d'une étiquette, d'un emoji, de deux tons, de verre, de rayures ou d'une photo avec une
légende, puis changez tout :

- n'importe quelle couleur, avec la transparence de votre choix, et des dégradés
- du texte dans dix-sept styles de police, qui peut s'incurver en arc
- des emojis, treize formes et dix motifs, jusqu'au grain de pellicule
- vos propres images, avec des retouches
- des ombres, des lueurs et des contours d'autocollant

L'interrupteur **Squelette du dossier** affiche la création sur le dossier ou à plat, et l'icône
d'à côté, aux tailles qu'utilise le Finder, montre si elle reste lisible. Créez un dossier en verre
transparent, ou choisissez **Icône libre** pour un autocollant qui n'a pas du tout la forme d'un
dossier. **Enregistrer et appliquer** la pose sur votre dossier, et **Modifier la création**, dans
son menu ⋯, la rouvre. [docs/fr/COMPOSER.md](docs/fr/COMPOSER.md) donne tous les détails.

<details><summary>L'éditeur</summary>

![Création d'un habillage : un insecte tiré de la bibliothèque d'icônes, incrusté dans un dossier bleu, avec la recherche d'icônes à côté](docs/images/composer.webp)

</details>

## Générer un habillage avec l'IA

FolderSkin peut créer un habillage à partir d'une description. Le **Modèle local** le peint sur
votre propre ordinateur, gratuitement : une fois installé, il ne demande aucune clé et
n'envoie rien nulle part. Il fonctionne sur les Mac à puce Apple sous macOS 14 ou plus récent, et sur
les PC Windows et Linux. Vous pouvez aussi utiliser **votre propre clé d'API** chez un fournisseur
que vous utilisez déjà. La clé est chiffrée et stockée sur votre ordinateur (sans aucune demande de
mot de passe du trousseau), FolderSkin n'a aucun serveur à lui, et rien ne part tant que vous n'avez
pas appuyé sur Entrée. Ouvrez **Générer avec l'IA**, décrivez une scène, choisissez un style, puis
**Dossier entier** (le modèle peint tout le dossier à partir du gabarit de FolderSkin, comme une
affiche) ou **Juste l'image** (une image à plat, plaquée sur le dossier de FolderSkin). Chaque
résultat est enregistré dans **Mes habillages** et peut être essayé tout de suite.

Choisissez où les images sont créées dans **Réglages → Fournisseur d'IA** : installez-y le Modèle
local, ou collez une clé. Le nom de chaque fournisseur renvoie à la page où vous en créez une.

| | Fournisseur | Modèles | Par image |
| :-: | --- | --- | --- |
| <picture><source media="(prefers-color-scheme: dark)" srcset="docs/images/providers/openai-dark.svg"><img src="docs/images/providers/openai.svg" width="20" height="20" alt=""></picture> | [OpenAI](https://platform.openai.com/api-keys) | GPT Image 2.5 Flare, GPT Image 2.5 Sunburst, GPT Image 1 | ~0,02–0,19 $ |
| <picture><source media="(prefers-color-scheme: dark)" srcset="docs/images/providers/xai-dark.svg"><img src="docs/images/providers/xai.svg" width="20" height="20" alt=""></picture> | [xAI Grok](https://console.x.ai) | Grok Imagine | ~0,02 $ |
| <picture><source media="(prefers-color-scheme: dark)" srcset="docs/images/providers/recraft-dark.svg"><img src="docs/images/providers/recraft.svg" width="20" height="20" alt=""></picture> | [Recraft](https://www.recraft.ai/profile/api) | Recraft V3 | ~0,04 $ |
| <img src="docs/images/providers/google.svg" width="20" height="20" alt=""> | [Google Gemini](https://aistudio.google.com/apikey) | Gemini 2.5 Flash Image | ~0,04 $ |
| <picture><source media="(prefers-color-scheme: dark)" srcset="docs/images/providers/bfl-dark.svg"><img src="docs/images/providers/bfl.svg" width="20" height="20" alt=""></picture> | [Black Forest Labs](https://dashboard.bfl.ai) | FLUX 1.1 Pro | ~0,04 $ |
| <img src="docs/images/providers/stability.svg" width="20" height="20" alt=""> | [Stability AI](https://platform.stability.ai/account/keys) | Stable Image Core | ~3 crédits |
| <picture><source media="(prefers-color-scheme: dark)" srcset="docs/images/providers/ideogram-dark.svg"><img src="docs/images/providers/ideogram.svg" width="20" height="20" alt=""></picture> | [Ideogram](https://ideogram.ai/manage-api) | Ideogram v3 | ~0,03–0,09 $ |

Pas de clé ? [docs/fr/PROMPTS.md](docs/fr/PROMPTS.md) propose un gabarit et un prompt pour le chat de
Grok ou de ChatGPT. L'app affiche le même prompt, déjà rempli, sous **Pas de clé d'API ?**

[docs/fr/AI.md](docs/fr/AI.md) présente les fournisseurs, l'endroit où la clé est stockée, la gestion
de la transparence pour les modèles qui ne savent pas renvoyer de canal alpha, et le sens de chaque
message d'erreur.

## Créer un pack

Une série thématique, par exemple des dossiers en 3D rendus avec un modèle d'image, devient un
pack de la communauté en une seule commande. Les packs vivent dans leur propre dépôt,
[folderskin-community](https://github.com/prajwal-svm/folderskin-community) : clonez-le à côté de
celui-ci. `folderskin-tools packs make` détoure les dossiers finis de leur fond magenta
(`--flat-backdrop` pour tout autre fond uni, comme un magenta qui a viré au rose ou un gris uni),
réduit et compresse chaque image pour qu'elle respecte les limites, donne une seule forme aux
dossiers finis et écrit `pack.json` :

```sh
cargo run -p folderskin-tools -- packs make ~/Pictures/renders --dir ../folderskin-community \
  --name "3D Folders" --tags 3d,glossy --author your-github-name \
  --preview /tmp/3d-folders.png
cargo run -p folderskin-tools -- packs check --dir ../folderskin-community
```

`render` montre n'importe quelle image sous la forme du dossier qu'elle donne, et `guide` dessine
les zones de sécurité du gabarit pour les illustrations plaquées sur le dossier.
[docs/fr/PACKS.md](docs/fr/PACKS.md) décrit le contrat et la façon de proposer un pack,
[docs/fr/SKINS.md](docs/fr/SKINS.md) la façon dont une image devient une icône, et
[.claude/skills/folderskin-skins/SKILL.md](.claude/skills/folderskin-skins/SKILL.md) apprend à
Claude Code à mener toute la boucle : vous pouvez donc tout simplement lui demander « fais un pack
à partir de ces rendus ».

## Comment ça marche

Le cœur en Rust (`crates/folderskin-core`) contient le gabarit du dossier sous forme de tracés
vectoriels, ajuste votre image pour couvrir le panneau arrière et le panneau avant, fait le rendu de
l'ensemble une seule fois à 2048 px avec `tiny-skia`, puis le réduit à chaque taille d'icône avec
Lanczos3. La webview ne dessine jamais la géométrie du dossier. Elle affiche les PNG rendus par le
cœur, si bien que la vignette de la galerie, l'aperçu et l'icône sur le disque sont les mêmes pixels
sur toutes les plateformes. [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) donne les détails (en
anglais).

## Notes par plateforme

| système | mécanisme | fichiers écrits dans le dossier | à noter |
|---|---|---|---|
| macOS | `NSWorkspace.setIcon` | le fichier invisible `Icon\r` que gère macOS | rien : le Finder se met à jour immédiatement |
| Windows | `desktop.ini` + `folderskin-<hash>.ico`, tous deux masqués + système, dossier marqué en lecture seule, puis `SHChangeNotify` sur le dossier et son parent | `desktop.ini`, `folderskin-<hash>.ico` | rien : le dossier se redessine dès la fin de l'opération |
| Linux | `.directory` pour KDE, plus `gio set metadata::custom-icon` pour Nautilus, Nemo et Caja | `.directory`, `.folderskin.png` | certains gestionnaires de fenêtres en mosaïque et gestionnaires de fichiers minimalistes ne lisent ni l'un ni l'autre |

Rétablir ne retire que ce que FolderSkin a écrit, et peut être lancé deux fois sans risque. Les
dossiers synchronisés dans le cloud (iCloud, OneDrive, Dropbox) synchroniseront les fichiers
auxiliaires sur vos autres machines. Le tableau complet, y compris le retour en arrière à la main,
se trouve dans [docs/PLATFORMS.md](docs/PLATFORMS.md) (en anglais).

## Télécharger

Gratuit · Open source · Sans compte · Sans pistage

| Plateforme | Paquet | Téléchargement |
| --- | --- | --- |
| macOS 12 ou plus récent · puce Apple et Intel | DMG universel | [![Télécharger pour macOS](https://img.shields.io/badge/Download_for_macOS-111827?style=for-the-badge&logo=apple&logoColor=white)](https://github.com/prajwal-svm/folderskin/releases/latest) |
| Windows 10 et 11 · x86_64 | `-setup.exe` ou MSI | [![Télécharger pour Windows](https://img.shields.io/badge/Download_for_Windows-3A86FF?style=for-the-badge&logo=data:image/svg%2bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCA0NDggNTEyIj48cGF0aCBmaWxsPSIjZmZmIiBkPSJtMCA5My43bDE4My42LTI1LjN2MTc3LjRIMHptMCAzMjQuNmwxODMuNiAyNS4zVjI2OC40SDB6bTIwMy44IDI4TDQ0OCA0ODBWMjY4LjRIMjAzLjh6bTAtMzgwLjZ2MTgwLjFINDQ4VjMyeiIvPjwvc3ZnPg==)](https://github.com/prajwal-svm/folderskin/releases/latest) |
| Linux · x86_64 | AppImage, DEB ou RPM | [![Télécharger pour Linux](https://img.shields.io/badge/Download_for_Linux-FCC624?style=for-the-badge&logo=linux&logoColor=111827)](https://github.com/prajwal-svm/folderskin/releases/latest) |
| Linux · ARM64 | AppImage, DEB ou RPM | [![Télécharger pour Linux](https://img.shields.io/badge/Download_for_Linux-FCC624?style=for-the-badge&logo=linux&logoColor=111827)](https://github.com/prajwal-svm/folderskin/releases/latest) |

L'app macOS est signée et notariée par Apple : elle s'ouvre comme n'importe quelle autre.
L'installateur Windows n'est pas encore signé, donc SmartScreen demande d'abord confirmation :
choisissez « Informations complémentaires », puis « Exécuter quand même ». Les paquets Linux
demandent glibc 2.35 ou plus récent (Ubuntu 22.04, Debian 12, Fedora 36 et versions ultérieures).

Une fois installé, FolderSkin se tient à jour tout seul : quand une nouvelle version sort, il
affiche les nouveautés, et **Mettre à jour et redémarrer** l'installe. Chaque mise à jour est
signée, et l'app vérifie la signature avant d'installer quoi que ce soit.

### La ligne de commande

`folderskin` fait depuis un terminal ce que fait l'app, et c'est pratique pour traiter des lots ou un
disque entier : poser des images sur des dossiers et les retirer, peindre des habillages avec le
Modèle local ou votre propre clé, et créer des packs. Sur Mac ou Linux :

```sh
curl -fsSL https://folderskin.app/install-cli.sh | sh
```

Sous Windows, dans PowerShell :

```powershell
irm https://folderskin.app/install-cli.ps1 | iex
```

Dans les deux cas, le téléchargement est vérifié avec son SHA-256, et aucun droit d'administrateur
n'est nécessaire. [crates/folderskin-cli/README.md](crates/folderskin-cli/README.md) (en anglais) donne
des exemples pour commencer et la liste de toutes les commandes.

## Compiler depuis les sources

Prérequis sur toutes les plateformes : [Rust](https://rustup.rs) (rustup installe à la première
utilisation la version indiquée dans `rust-toolchain.toml`), Node 22 ou plus récent, et pnpm 11
(`packageManager` dans `package.json` indique la version exacte).

- **macOS :** les outils en ligne de commande Xcode (`xcode-select --install`).
- **Windows :** Visual Studio Build Tools avec la charge de travail C++, et le runtime WebView2
  (déjà présent sous Windows 11).
- **Linux :**

  ```sh
  sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file \
    libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
  ```

Ensuite :

```sh
pnpm install
pnpm tauri dev      # run it
pnpm tauri build    # installers land in target/release/bundle/
```

Les vérifications, que la CI lance toutes ([docs/CI.md](docs/CI.md) liste chaque job, en anglais) :

```sh
cargo test --workspace
pnpm test
pnpm typecheck
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
```

## Contribuer

Les rapports de bug et les habillages sont les bienvenus. Les habillages arrivent par les
[packs de la communauté](docs/fr/PACKS.md). [CONTRIBUTING.md](CONTRIBUTING.md) décrit la marche à
suivre et les quelques règles de la maison, et [SECURITY.md](SECURITY.md) explique comment signaler
une vulnérabilité en privé. Les changements sont consignés dans [CHANGELOG.md](CHANGELOG.md), et
[docs/RELEASING.md](docs/RELEASING.md) explique comment une version est compilée, signée et publiée.
Ces documents sont en anglais.

Si FolderSkin a embelli vos dossiers, une [étoile sur GitHub](https://github.com/prajwal-svm/folderskin)
aide d'autres personnes à le découvrir.

## Licence

FolderSkin est un logiciel libre sous [GNU General Public License v3.0](LICENSE)
(`GPL-3.0-only`) : utilisez-le, étudiez-le, modifiez-le et partagez-le. Si vous partagez une version
modifiée, partagez son code source sous la même licence. Les versions jusqu'à la 0.1.6 sont sorties
sous licence MIT et le restent.

Le nom et le logo FolderSkin ne font pas partie de la licence. [TRADEMARKS.md](TRADEMARKS.md)
explique comment les utiliser. Les habillages des packs de la communauté ont leurs propres licences,
indiquées dans chaque pack.

Copyright 2026 les contributeurs de FolderSkin.

[Sécurité](SECURITY.md) · [Contribuer](CONTRIBUTING.md) · [GPL-3.0](LICENSE) · [Marques](TRADEMARKS.md)

## Historique des étoiles

<a href="https://www.star-history.com/#prajwal-svm/folderskin&Date">
 <picture>
   <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/svg?repos=prajwal-svm/folderskin&type=Date&theme=dark" />
   <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/svg?repos=prajwal-svm/folderskin&type=Date" />
   <img alt="Graphique de l'historique des étoiles" src="https://api.star-history.com/svg?repos=prajwal-svm/folderskin&type=Date" />
 </picture>
</a>
