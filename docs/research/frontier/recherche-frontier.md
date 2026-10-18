# Recherche Frontier SP-3000 : synthèse pour le Frontier Look Lab

Synthèse de onze recherches en parallèle (brevets Fuji, manuels, papier Crystal Archive, science spectrale,
sources japonaises, émulateurs existants, descriptions des labos, données et bidouilles, méthodes de
décomposition, dons) et d'une décomposition de la table du dépôt. Le détail chiffré de la table est dans
[analyse-table-frontier.md](analyse-table-frontier.md). **[M]** mesuré sur nos fichiers, **[S]** dit par une
source (lien en fin de fichier), **[I]** inféré.

## En bref

1. **La table du dépôt est fidèle à la machine qu'elle a mesurée** [M] : ΔE 0,27 de médiane contre les 63 rendus
   réels. Elle n'aplatit ni la saturation ni le contraste des tons moyens (rapport de chroma 0,999 à 1,003, pente
   du gris 0,1708 contre 0,1710).
2. **Les bleus violets et la peau jaune viennent de la table elle-même** [M] (corrigé après mesure étage par
   étage, voir [neutre-et-frontier.md](neutre-et-frontier.md)) : la table tourne les bleus de +14 à +20° vers le
   violet et la peau de ~+18° vers le jaune en perdant du rouge, contre ONI Pro à clarté égale. L'entrée, le
   clamp et le clip n'y ajoutent presque rien (diaphotie +4°, dose 0,6 −2,8°). [I] La mire synthétique n'a ni
   diaphotie ni vrai masque : la machine, réglée pour de vrais films, corrige une bavure absente.
3. **Ce qui ne se voit pas dans une table** [I] : Hypertone et Hyper-sharpness sont des traitements **locaux** ;
   les réglages de la machine mesurée (gradation, saturation, Hypertone) ne sont écrits nulle part ; en fenêtre
   étroite (gain 1,3), cyans-verts, jaunes-verts et bleus sombres sortent des mesures.
4. **Aucun modèle simple ne remplace la table** [M] : le meilleur modèle réglable plafonne à ΔE ~1,6 à 1,9
   (contre 0,27). Il faut un étage de teinte selon la chroma, et un petit treillis correctif 13³ (ΔE 0,67) si
   on veut garder le caractère mesuré sous les réglages.
5. **Pas de raccourci caché** [S] : aucun mode brut, aucun fichier de table, aucune mire IT8 publiée sur
   SP-3000. Le manuel de service (1011 pages) n'est lisible qu'en sommaire.

## Comment travaille un Frontier, d'après Fuji

Chaîne reconstituée à partir des brevets Fuji [S] (aucun ne nomme le SP-3000) :

1. Prescan, puis équilibre de gradation par canal contre un film de référence (US6081343, US6160634).
2. Linéarisation, matrice 3×3 du capteur vers la densité Status M, passage en densité (US6765691 : LUT0 à LUT2,
   Matrix1).
3. Décalage CMYD par image (les clics D, C, M, Y ; US6765691 LUT3).
4. Matrice 3×3 de la densité du négatif vers l'exposition du papier, puis courbe sensitométrique du papier
   (US6765691 : Matrix2, LUT4). Densité du gris visée vers 0,6 à 0,7.
5. Compression de dynamique par masque flou, nulle sous un seuil (US6807316, US5796870) : c'est la famille
   d'Hypertone [I].
6. Matrices de couleur et gamma, avec une saturation couplée au contraste local (US6122076) ; chroma plafonnée
   au bord du gamut, par teinte (US6823083).
7. Netteté qui sépare bords et grain (US6055340) : la famille d'Hyper-sharpness [I].

[I] Notre table 27³ capture les étapes 2 à 4 et 6 en bloc, sur des patchs uniformes ; les étapes 5 et 7 lui
échappent par construction.

## Les réglages de la machine

Manuel *Condition Setup and Maintenance* PP3-B1271E2 (menu 0225, conditions négatif) [S] :

| Réglage | Valeurs |
|---|---|
| Tone Adjustment | Standard, All Hard, All Soft, Highlight Hard/Soft, Shadow Hard/Soft |
| Hypertone | Oui/Non ; niveau hautes lumières et ombres : Lower, Low, Normal, High, Higher ; Mode 1 (normal) ou Mode 2 (contre-jour, plus de contraste) |
| Hyper-sharpness | Low 3 à High 3 |
| Saturation | −9 à +9 |
| Gradation | Hard 2, Hard 1, Normal, Soft 1, Soft 2, Soft 3 |
| Bright Mode Colour Intensity | 4 à −2 |
| Pas des touches | C, M, Y : 8 ; D : 15 |

Feuille de départ d'un labo (Harman, Phoenix 200) [S] : Hypertone Oui, Full correction, Standard, niveaux
Normal, Mode 1 ; netteté Non ; gradation ombres −4, milieux −2, hautes lumières 0 ; saturation −3 ; pas CMY 5,
D 10. Le manuel ne donne aucune valeur d'usine.

## Le look décrit par les labos

Consensus [S] (labos et photographes, jamais mesuré) : ombres plus froides, cyan-bleu ; peau dorée (Noritsu :
pêche ou rose) ; plus saturé et plus contrasté que Noritsu ; noirs profonds, parfois bouchés ; hautes
lumières difficiles à brûler (Hypertone) ; grain lissé ; axe jaune-bleu accentué (Noritsu : magenta-vert).

Désaccords [S] : les verts (« plus bleus » chez un labo, « plus verts » chez deux autres), la saturation
(Replichrome dit le Frontier *moins* saturé), les hautes lumières. Les rouges, les oranges et le ciel ne sont
décrits par personne : ton œil est la seule référence là-dessus.

## Pourquoi les bleus virent au violet et la peau au jaune

Mesuré étage par étage dans [neutre-et-frontier.md](neutre-et-frontier.md) §4 [M] : un bleu de scène à 247° sort
à 259–264° (Pro : 243–246°), et 15 à 33 % des bleus passent 270° ; une peau à 58° sort à 77–80° avec une
composante rouge divisée par ~4 (Pro : 62–63°). La table en porte l'essentiel ; la diaphotie de la caméra ajoute
+4° aux bleus et ~8 % de perte de chroma à la peau. La dominante verte du gris relevée plus haut appartient au
réglage Base de la mire, pas à l'axe neutre de l'app. Le Look Lab corrige par la matrice (vert dans le bleu),
les décalages par canal du papier (la peau) et la teinte par secteur. Valeurs neutre et Frontier de chaque
réglage : même fichier, §2.

## Le Look Lab : premier jeu de paramètres

Ossature tirée de la décomposition, valeurs de départ mesurées (ΔE ~1,9 dès l'ouverture). Les réglages du
SP-3000 servent de vocabulaire et de plages. Rien ne s'applique seul : tout est un réglage ou un clic.

| Étage | Réglage | Forme | Départ |
|---|---|---|---|
| Entrée | Dose de séparation | curseur 0 à 1 | 0,6 (actuel) |
| Entrée | Gain de fenêtre | curseur | 1,0 / 1,3 |
| Équilibre | D, C, M, Y | clics existants | tables actuelles |
| Diaphotie | Matrice 3×3, lignes de somme 1 | 6 curseurs hors diagonale | P3 : [0,923 0,049 0,028] [0,099 0,925 −0,024] [0,007 0,232 0,761] |
| Papier | **Courbe papier** | **graphe** à ~15 points, commune aux trois canaux | courbe P1 |
| Papier | Décalage et gain par canal | 6 curseurs | gains 1,26 / 1,16 / 1,10 ; décalages −0,073 / −0,015 / −0,071 |
| Papier | Gradation | ombres, milieux, hautes lumières | 0 (Harman : −4, −2, 0) |
| Couleur | Torsion de teinte selon la chroma | 12 secteurs | bleus −10°, oranges −6° |
| Couleur | Gain de chroma par secteur | 12 secteurs | 1 |
| Couleur | Saturation globale | −9 à +9 | 0 |
| Couleur | Dominante du gris selon la densité | a, b sur 3 à 5 points | a −0,022, b +0,021 au gris moyen |
| Couleur | Teinte des ombres | a, b | 0 (le look labo : vers cyan) |
| Local | Hypertone | marche/arrêt, ombres et hautes lumières (5 crans), rayon | arrêt |
| Local | Hyper-sharpness | Low 3 à High 3 | bloc Details d'ONI |
| Fin | Caractère mesuré | curseur 0 à 1 (treillis correctif 13³) | 1 |
| Fin | Point blanc | clip, puis blanc ramené vers L 95 | clip |

Les blocs d'ONI repris : courbes (le graphe papier), Spectrogram (secteurs de teinte), saturation, Details.

## Comment on mesure

- Chaque réglage du Lab se mesure contre les 63 rendus réels : ΔE Oklab et ΔE00, médiane, p90, p99, et où
  tombe le pire.
- Jumeau : le même modèle avec la matrice forcée à l'identité doit faire nettement pire, sinon la mesure ne
  voit rien.
- Hors des données (plus de 2 pas), on ne règle qu'à l'œil, et on le dit.

## Ce qui ferait avancer le plus

- **Les réglages de la machine mesurée** (Hypertone, gradation, saturation) : sans eux, on ne sait pas quelle
  part du look est dans la table.
- **Une paire de scans réels** : la même vue scannée par un SP-3000 et par ta caméra. C'est la seule façon de
  trancher la dose 0,6, Hypertone et le contraste local.
- Le corps du manuel de service (sections 0223, 0224, 0240, 0741), introuvable en ligne.
- Comparaisons visuelles publiées, à regarder à l'œil : drunkweddingphotographer (Gold 200), lightleakss
  (Portra 400), analognotes (Portra 400), Carmencita (Round 2).

## Sources

- Manuel PP3-B1271E2 : https://fotobrell.com/wp-content/uploads/pp3-b1271e2-1.pdf
- Feuille Harman Phoenix : https://harmanphoto.co.uk/wp/wp-content/uploads/sites/3/2024/10/Phoenix-Scanning-Parameters-011024.pdf
- Manuel de service SP-3000 (sommaire) : https://www.manualslib.com/manual/1690966/Fujifilm-Sp-3000.html
- Brevets : https://patents.google.com/patent/US6765691 , US6081343A, US6160634, US6807316B2, US5796870A, US6122076A, US6823083B1, US6055340A, JP3668014B2
- Nakamura 1999, Frontier 350 : https://www.imaging.org/common/uploaded%20files/pdfs/Papers/1999/PICS-0-42/1003.pdf
- Fuji R&D n° 49 (2004) : https://asset.fujifilm.com/www/jp/files/2019-10/da0eca2de68480ca18c6c08713c5855c/rd_report_ff_rd049_004.pdf
- Hypertone : https://fujixweekly.com/2021/09/12/fujifilm-frontier-scanners-dynamic-range-priority/ , https://www.photrio.com/forum/threads/halo-on-scans.161511/
- Looks comparés : https://carmencitafilmlab.com/?p=35292 , https://www.optik-oldschool.com/blogs/blog/why-we-prefer-frontier-scans-for-opticolour200 , https://jbflanc.substack.com/p/the-art-of-film-scanning-frontier , https://lightleakss.substack.com/p/scanning-35mm-film-noritsu-vs-frontier , https://thedrunkweddingphotographer.com/summerimperfect/noritsu-vs-frontier , https://analognotes.substack.com/p/noritsu-vs-frontier , https://contactsheet.app/magazine/fuji-vs-noritsu
- Émulateurs : https://support.gettotallyrad.com/replichrome/faq.html , https://alchemycolor.com/film-emulation/ , https://github.com/andreavolpato/agx-emulsion
- Diaphotie et densitométrie : https://nordiclab.org/research/ideal-film-scanning/ , https://github.com/AhaiMk01/ART/issues/46
- Décomposition : https://ueaeprints.uea.ac.uk/id/eprint/52673/ (racines polynomiales), https://docs.darktable.org/usermanual/4.6/en/module-reference/processing-modules/sigmoid
