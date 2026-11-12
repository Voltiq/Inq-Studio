# Inq Studio — manuel

Inq Studio est un lecteur et éditeur de PDF pour Windows. Ce manuel parcourt ce qu’il sait faire, dans l’ordre
où l’on en a généralement besoin. Le même texte s’ouvre dans l’application avec `F1`. *An English version is
in [GUIDE.md](GUIDE.md).*

## Installer et mettre à jour

Téléchargez le programme d’installation depuis la
[page des versions](https://github.com/Voltiq/Inq-Studio/releases/latest) et lancez-le. Il s’installe pour
l’utilisateur actuel et ne demande pas de droits d’administrateur. Il n’est pas encore signé : si
SmartScreen affiche un avertissement, choisissez *Informations complémentaires › Exécuter quand même*.

Après l’installation, un PDF s’ouvre dans Inq Studio avec *Ouvrir avec* dans l’Explorateur ; vous pouvez
aussi y en faire l’application par défaut.

Inq Studio recherche une nouvelle version à chaque démarrage. S’il y en a une, il demande s’il faut
l’installer : **Mettre à jour maintenant**, **Plus tard** (il redemande au prochain démarrage) ou **Ignorer
cette version** (il ne redemande que pour la version suivante). Cochez *Ne plus demander au démarrage* pour
recevoir un bref avis à la place de la question ; le même interrupteur se trouve dans les Réglages.
*Réglages › Mises à jour* vérifie à la demande et installe. Les mises à jour sont signées, et la signature
est vérifiée avant toute installation.

## Premiers pas

1. Ouvrez un PDF : le bouton du dossier, `Ctrl+O`, ou déposez des fichiers n’importe où sur la fenêtre.
2. Lisez, annotez ou modifiez avec les outils situés au-dessus de la page.
3. Enregistrez avec `Ctrl+S`, ou sous un nouveau nom avec `Ctrl+Maj+S`.

Au premier démarrage, une courte **visite** montre les parties de la fenêtre. Elle peut être relancée depuis
le bouton **Aide** (le point d’interrogation en haut à droite), qui ouvre aussi ce manuel.

## La fenêtre

- **Barre de titre** : l’icône et le nom de l’application, le document ouvert, et les boutons pour réduire,
  agrandir et fermer la fenêtre. Faites-la glisser pour déplacer la fenêtre ; double-cliquez pour l’agrandir.
- **Barre supérieure** : à gauche, ouvrir, enregistrer, enregistrer sous, annuler et rétablir ; au milieu,
  le nom du document, la page et les badges (*signé*, le type de protection par mot de passe, une
  déclaration PDF/A — cliquez sur un badge pour ouvrir sa boîte de dialogue) ; à droite, le thème, les
  **outils du document** (la clé), l’aide, les réglages et l’inspecteur.
- **Barre latérale** (`Ctrl+B`) : vignettes des pages, signets, polices, commentaires, historique.
- **Barre d’outils** au-dessus de la page : les outils décrits ci-dessous, la navigation entre les pages,
  le zoom, la recherche.
- **Inspecteur** (`Ctrl+I`) : ce qui est sélectionné, les options de l’outil actif et les propriétés du
  document.
- Plusieurs documents s’ouvrent en onglets. Un PDF ouvert depuis l’Explorateur rejoint la fenêtre déjà
  ouverte.

Chaque modification peut être annulée (`Ctrl+Z`) jusqu’à la fermeture du document ; l’onglet **Historique**
liste les étapes et permet de revenir à n’importe laquelle. Rien n’est écrit dans le fichier avant
l’enregistrement (`Ctrl+S`).

## Lire

Faites défiler, ou utilisez `Page préc.` / `Page suiv.`. `Ctrl`+molette zoome ; `Ctrl+0` ajuste à la
largeur. `Ctrl+F` recherche du texte. Avec l’outil **Sélectionner** (`V`), on sélectionne et copie du
texte ; la **Main** (`H`) fait glisser la page.

## Modifier le texte

Choisissez **Modifier le texte et les images** (`E`). Chaque ligne reçoit un contour.

- **Double-cliquez** sur une ligne pour la réécrire. Si la ligne fait partie d’un paragraphe, le paragraphe
  entier s’ouvre et il est remis en lignes dans sa colonne lorsque vous validez avec `Ctrl+Entrée`.
- **Faites glisser** une ligne pour la déplacer ; elle s’aligne sur les bords et les lignes de base des
  autres lignes (`Alt` déplace librement). Les flèches déplacent finement. `Suppr` la supprime. `Maj`+clic
  ou tracez un cadre pour sélectionner plusieurs lignes.
- L’inspecteur change la taille, la couleur et la police.

Avec **Réaligner le texte automatiquement** activé dans les Réglages (c’est le cas par défaut), un
paragraphe garde sa forme lorsque vous modifiez l’une de ses lignes : raccourcissez une ligne et les mots
qui continuent sa phrase y remontent ; allongez-la et le surplus passe dans les lignes suivantes ; supprimez
une ligne et celles du dessous remontent. Seules les lignes du même bloc de texte bougent. Les autres blocs
et le reste de la page restent où ils sont ; un texte qui gagne une ligne peut donc empiéter sur ce qui se
trouve dessous.

Ce que vous saisissez est écrit **dans la police que le document utilise déjà**. S’il manque un caractère à
cette police, Inq Studio le prend dans la même police installée sur votre ordinateur ; ce n’est que si la
police est introuvable que le caractère manquant est composé dans une police proche, et un avis vous le
signale. L’onglet **Polices** de la barre latérale indique, pour chaque police, ce qu’une modification peut
faire.

**Ajouter du texte** (`T`) : cliquez à l’endroit où il doit commencer, saisissez, validez avec `Ctrl+Entrée`.

### Texte des numérisations

Une page numérisée est une image ; ses mots sont des pixels, pas du texte. Après *Reconnaître le texte*
(voir plus bas), les mots peuvent être sélectionnés, et aussi réécrits : double-cliquez sur une ligne
reconnue avec l’outil de modification. Comme les anciens mots font partie de l’image, Inq Studio recouvre la
ligne d’un cache de la couleur du papier et dessine votre texte par-dessus dans la couleur de l’encre. Le
nouveau texte est composé en Helvetica ou en Arial ; il ne correspondra donc pas exactement à la police de
la numérisation. Le texte reconnu se modifie ligne par ligne.

## Images

Avec l’outil de modification, cliquez sur une image pour la sélectionner : faites glisser pour la déplacer,
faites glisser son coin pour la redimensionner, et remplacez-la, enregistrez-la ou supprimez-la dans
l’inspecteur. **Insérer une image** (`I`) place un PNG ou un JPEG.

## Commentaires et formes

Surlignez (`M`), soulignez (`U`) et barrez (`K`) en faisant glisser sur le texte. La note (`N`), le dessin
(`D`), le rectangle, l’ellipse, la ligne, la flèche, le polygone, la zone de texte (`B`), le tampon (`S`) et
les liens (`L`) fonctionnent en cliquant ou en faisant glisser sur la page. Avec l’outil Sélectionner,
cliquez sur l’un d’eux pour le modifier dans l’inspecteur, ou faites-le glisser pour le déplacer.

## Formulaires

Remplissez les champs de formulaire directement sur la page avec l’outil Sélectionner. *Outils du document ›
Aplatir les champs de formulaire* transforme les valeurs saisies en contenu de page ordinaire.

## Pages

Dans l’onglet **Pages** : faites glisser les vignettes pour réorganiser ; clic droit pour faire pivoter,
dupliquer, insérer une page vierge, insérer des pages d’un autre PDF, extraire et supprimer. Les *Outils du
document* ajoutent les filigranes, les en-têtes et pieds de page, les numéros Bates, les libellés de page,
le redimensionnement à un format de papier et la division en plusieurs fichiers. **Rogner** (`C`) masque
tout ce qui se trouve hors d’une zone.

## Caviarder

**Caviarder** (`X`) sert pour le contenu qui doit disparaître, et pas seulement être recouvert. Faites
glisser dessus — ou laissez *Tout marquer* trouver chaque occurrence d’un mot —, vérifiez les marques, puis
appliquez-les dans l’inspecteur. Le texte, les pixels des images et les dessins sous chaque marque sont
retirés du fichier lui-même. Après l’enregistrement, ils ne peuvent plus être récupérés.

## Numérisations : reconnaître le texte (OCR)

*Outils du document › Reconnaître le texte*. Choisissez la langue du texte et les pages. Les mots sont posés
de façon invisible sur la numérisation, qui devient ainsi consultable, sélectionnable et copiable ; l’image
reste telle quelle.

La reconnaissance s’effectue sur votre ordinateur avec les langues OCR installées dans Windows. Ce sont des
modules linguistiques de Windows, sans rapport avec la langue de l’interface d’Inq Studio.

- **Détecter automatiquement** (proposé lorsque plusieurs langues sont installées) lit la première page
  contenant du texte dans chaque langue installée et garde celle qui convient.
- **Ajouter une langue à Windows** : choisissez une langue et appuyez sur *Installer…*. Windows télécharge
  le module et demande les droits d’administrateur ; sur un ordinateur géré par un établissement scolaire
  ou une entreprise, cela peut ne pas être autorisé. La même chose se fait dans Windows sous *Paramètres ›
  Heure et langue › Langue et région*.

## Signatures

*Outils du document › Signatures* vérifie chaque signature du document et indique, en termes clairs :

- si le contenu signé est **inchangé**, et si quelque chose a été ajouté au fichier par la suite ;
- **qui a signé**, d’après le certificat, et si votre ordinateur fait confiance à l’autorité qui l’a émis.

*Signer…* signe le document avec une identité numérique :

- **Depuis Windows** : un certificat de votre magasin de certificats Windows, y compris les cartes à puce
  et les jetons.
- **Depuis un fichier** : un fichier `.pfx` ou `.p12` et son mot de passe.
- **En créer une** : une identité auto-signée, enregistrée en `.pfx`. Elle prouve qu’un document n’a pas
  changé depuis que vous l’avez signé, mais pas qui vous êtes — pour cela, il faut un certificat d’un
  service de confiance.

Une signature peut être affichée sur une page (nom, date, motif) ou être invisible. Avec un **serveur
d’horodatage** indiqué dans les Réglages, la signature porte en plus l’heure d’une autorité d’horodatage, ce
qui la garde vérifiable après l’expiration de votre certificat.

La signature enregistre le document. Les modifications ultérieures sont ajoutées à la fin du fichier ; la
signature reste donc valide et montre que quelque chose a été ajouté après elle.

## Protection par mot de passe

*Outils du document › Protéger par mot de passe*. Un **mot de passe d’ouverture** verrouille le document. Un
**mot de passe d’autorisation** avec des restrictions (pas d’impression, pas de copie…) demande aux
programmes de lecture de se retenir ; c’est une demande, pas un verrou. Un document protégé que vous ouvrez
et enregistrez conserve sa protection.

## Fichiers plus petits, archivage, comparaison

- **Optimiser** réduit la résolution des images qui en ont plus que nécessaire, fusionne les polices et
  images en double, et affiche la taille avant et après.
- **PDF/A** vérifie le document par rapport aux normes d’archivage PDF/A-1b, 2b et 3b, liste ce qui n’est
  pas conforme et convertit ce qui peut être corrigé. Les vérifications couvrent les règles que les
  documents enfreignent en pratique ; elles ne constituent pas une validation certifiée.
- **Comparer avec…** affiche deux documents côte à côte : mots supprimés en rouge, mots ajoutés en vert, et
  une liste des modifications. *Marquer les différences dans l’image* trouve aussi les images et graphiques
  modifiés.

## Accessibilité

*Outils du document › Accessibilité* liste ce sur quoi buteront les utilisateurs de lecteurs d’écran,
définit la langue du document, permet de donner un texte de remplacement aux figures et peut dessiner
l’ordre de lecture sur la page.

## Exporter et imprimer

*Outils du document › Exporter des pages ou du texte* enregistre des pages en PNG ou en JPEG à la résolution
de votre choix, ou le texte dans un fichier texte. `Ctrl+P` imprime.

## Réglages

Langue (anglais, allemand, espagnol, français ; s’applique après un redémarrage), thème, zoom, unités, votre
nom d’auteur, le mode d’enregistrement des fichiers, le réalignement automatique du texte, le serveur
d’horodatage, les mises à jour et l’emplacement du fichier journal. Vos documents restent sur votre
ordinateur : Inq Studio n’envoie rien. Les seules connexions qu’il établit sont la recherche d’une nouvelle
version au démarrage et, si vous en indiquez un, le serveur d’horodatage — qui reçoit une empreinte, jamais
le document.

Certains messages du moteur PDF (quelques erreurs et avis) apparaissent encore en anglais.

## Quand quelque chose ne va pas

- **Un PDF affiche des pages blanches.** Passez à la dernière version ; avant la 1.0.1, les fichiers de
  certains scanners étaient lus de façon incomplète. Si cela persiste, le fichier est peut-être
  endommagé : d’autres lecteurs réparent sans le dire.
- **Une ligne ne peut pas être modifiée.** L’onglet *Polices* de la barre latérale indique, pour chaque
  police, ce qu’une modification peut faire. Le texte qui fait partie d’une image demande d’abord
  *Reconnaître le texte*.
- **La langue dont j’ai besoin manque dans la boîte de dialogue OCR.** Installez son module avec *Ajouter
  une langue à Windows*.
- **Le texte saisi est invisible sur une numérisation.** Passez à la version 1.0.2 ou ultérieure.
- **L’application s’est fermée de façon inattendue.** Le démarrage suivant affiche le rapport d’incident.
  Le fichier journal et les rapports se trouvent uniquement sur votre ordinateur (*Réglages › Diagnostic ›
  Afficher dans le dossier*) et ne contiennent jamais le contenu des documents. Joignez-les lorsque vous
  signalez un problème sur
  [github.com/Voltiq/Inq-Studio/issues](https://github.com/Voltiq/Inq-Studio/issues).

## Raccourcis clavier

| | |
| --- | --- |
| `Ctrl+O` / `Ctrl+S` / `Ctrl+Maj+S` | Ouvrir / enregistrer / enregistrer sous |
| `Ctrl+Z` / `Ctrl+Y` | Annuler / rétablir |
| `Ctrl+F` / `Ctrl+P` | Rechercher / imprimer |
| `Ctrl+B` / `Ctrl+I` / `Ctrl+,` | Barre latérale / inspecteur / réglages |
| `F1` | Ce manuel |
| `Ctrl+W` / `Ctrl+Tab` | Fermer l’onglet / onglet suivant |
| `Ctrl+=` / `Ctrl+-` / `Ctrl+0` | Zoom avant / zoom arrière / ajuster à la largeur |
| `V` `H` `E` `T` `I` | Sélectionner, main, modifier, ajouter du texte, insérer une image |
| `M` `U` `K` `N` `D` `B` `S` `L` | Surligner, souligner, barrer, note, dessiner, zone de texte, tampon, liens |
| `X` `C` | Caviarder, rogner |
