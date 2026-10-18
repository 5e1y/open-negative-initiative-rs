# Table Frontier SP-3000 : ce qu'elle contient, avant le « Frontier Look Lab »

Matériau lu sans rien modifier : `Resources/Scanner/frontier.cube` (27³), `frontier.json`, `Scripts/scanner/*`,
`data/measurements.json.gz` (7 négatifs synthétiques, 63 rendus de la machine hors looks F, 2 241 lectures
chacun), `ScannerRender.swift`, `scanner.metal`. Scripts d'analyse hors dépôt (scratchpad de session).
**[M]** = mesuré sur les fichiers, **[I]** = inféré ou simulé. ΔE = distance Oklab × 100.

Contrôle de départ [M] : en recalant un offset de densité par rendu contre le cube publié, je retrouve
exactement les figures de `frontier.json` : cube contre rendus mesurés, ΔE médiane 0,27 / p90 0,81 / p99 2,54
(141 183 lectures). La reconstruction de la scène (`make_colorchecker.swift` porté en Python) colle aux négatifs
à 0,0005 de densité près sur les patchs.

Figures : `teinte-vs-chroma.png`, `couverture.png`, `courbes-papier.png`, à côté de ce fichier.

## 1. « Débaker » le cube : combien un modèle paramétrique en garde

Tous les modèles sont ajustés sur 20 000 à 24 000 lectures tirées au hasard des 63 rendus, résidu en Oklab,
puis évalués sur les 141 183 [M].

| Modèle | Paramètres | ΔE méd. / p90 / p99 |
|---|---|---|
| Cube 27³ publié | 59 049 | 0,27 / 0,81 / 2,54 |
| A : shaper 1D par canal (13 nœuds) → 3×3 → courbe 1D par canal (21 nœuds, log linéaire) | 111 | 1,71 / 3,87 / 6,74 |
| A, shaper à 25 nœuds | 147 | 1,66 / 3,83 / 6,65 |
| A2 : A + 3×3 en linéaire après les courbes | 120 | 1,61 / 3,56 / 5,95 |
| **P3** : 3×3 aux lignes de somme 1, sans shaper → courbe « papier » monotone par canal (15 nœuds) | 51 | 1,86 / 4,52 / 8,24 |
| **P1** : idem, une courbe commune + décalage et gain par canal | 27 | 1,91 / 4,57 / 8,47 |
| B : shaper → 3×3 → courbe tonale sur une luminance (préserve les rapports RVB) + saturation(L) | 80 | 1,76 / 4,29 / 8,07 |
| B + étage teinte/chroma par secteur | 98 | 1,59 / 3,63 / 6,47 |
| P3 + rotation de teinte proportionnelle à la chroma, par secteur | 82 | 1,71 / 4,08 / 7,20 |
| P3 + treillis correctif 5³ / 7³ / 9³ / 13³ | | 1,43 / 1,18 / 1,00 / **0,67** (p90 1,54) |

[M] Aucun modèle séparable ne descend sous ~1,6 de médiane, même en doublant les nœuds du shaper (1,71 → 1,66) :
c'est un plancher de structure, pas un défaut d'ajustement. La variante qui préserve la teinte (B) fait à peine
moins bien que les courbes par canal (1,76 contre 1,71) ; ni l'une ni l'autre ne capture la machine.

[M] Où se loge le résidu de P3 (médianes) : il croît avec la chroma rendue (1,13 sous C 0,03 → 3,91 au-dessus de
0,18), culmine aux densités moyennes (densité moyenne 0,5–0,8 : 3,15 ; au-dessus de 1,4 : 1,00), donc vers
L 0,4–0,6 (3,02). Par teinte rendue : 150–180° (verts-cyans) 3,95 et 270–300° (violets-bleus) 3,55, p90 9,5 ;
69 % du 1 % le pire sont des violets 270–300°, presque tous sur la bande au bord de P3. Sur les tons moyens P3 garde la chroma
(rapport 1,005 à 1,08) mais se trompe de **~5° de teinte** en moyenne : le résidu est de la teinte, pas de la
saturation.

[M] Nombres de P3 (densité au-dessus du support ; matrice appliquée avant les courbes) :

```
M = [0.923 0.049 0.028]      code sRGB de la courbe à  D = 0    0.25  0.5   0.75  1.0   1.25  1.5
    [0.099 0.925 -0.024]                           R  0.003 0.049 0.150 0.399 0.753 0.930 0.971
    [0.007 0.232 0.761]                            V  0.002 0.045 0.177 0.481 0.771 0.913 0.975
                                                   B  0.002 0.028 0.137 0.422 0.710 0.887 0.940
pente au neutre (dcode/dD) : R 1.52, V 1.40, B 1.29
P1 : gains par canal 1.26 / 1.16 / 1.10, décalages -0.073 / -0.015 / -0.071 (densité), même matrice à 0,01 près
```

[I] Les courbes sont un S franc : du code 0,05 au code 0,95 en ~1,25 de densité. La matrice ajoutée
(hors diagonale positive, surtout 0,23 de vert dans le bleu) **mélange** les canaux au lieu de les séparer : sur
un négatif sans diaphotie, la machine désature l'axe bleu-jaune. À ne pas surinterpréter (jauge partagée avec
les courbes), mais A et A2 trouvent le même signe.

## 2. Couverture : ce que les mesures contraignent vraiment

[M] Le noyau clampe l'entrée à `reached` : `reachedLow` (−0,53 / −0,57 / −0,68) est **sous** le domaine du cube
(−0,15), donc le plancher effectif est −0,15 sur les trois canaux ; `reachedHigh` vaut 1,81 / 1,78 / 1,97. Seuls
**38,5 %** des 19 683 nœuds sont dans cette boîte ; les autres ne servent qu'à l'interpolation du bord.

[M] Un nœud est dit « touché » si une lecture mesurée tombe dans une de ses huit cellules (distance de
Tchebychev < 1 pas = 0,106). Touchés : 28,1 % de tous les nœuds, **68,9 % des nœuds atteignables** ; à moins de
3 pas : 46,8 %. Tout dépend de l'écart entre canaux au nœud : < 0,6 → 100 % ; 0,6–1,0 → 93 % ; 1,0–1,5 → 59 % ;
> 1,5 → 16 % (`couverture.png`). Le reste est du lissage (dérivées secondes, 1e-3).

[M] Par couleur rendue au nœud : bleus 230–280° de chroma ≥ 0,15 touchés à 71–89 %, rouges-oranges 20–60° à
66–70 %. Les mesures elles-mêmes montent à C 0,224 en bleu et 0,227 en rouge-orange.

[I] Scènes simulées au bord de Rec.2020 (L 0,5 / 0,72 / 0,85, ±1 IL) passées par le modèle de film du
négatif synthétique puis par l'étage d'entrée de l'app :
- gain 1,0 : tout reste à moins de 0,5 pas des mesures (le pied du film plafonne déjà la saturation) ;
- gain 1,3 (fenêtre verte étroite) : les cyans-verts 152–217° sortent jusqu'à 2,9 pas (2,0 avec diaphotie et
  S 0,6), les jaunes-verts à +1 IL jusqu'à 4,6 pas, les **bleus sombres (L 0,5) jusqu'à 1,9 pas** ; les rouges
  et oranges restent à moins d'un pas, sauf 17–22° à L 0,5 (1,9).

## 3. Rotation de teinte quand la chroma monte

[M] Le rendu Base n'est pas neutre sur le gris : a −0,022, b +0,021 au gris moyen (dominante C 0,03 vers 136°,
vert-jaune), ~0 en haute lumière. Les teintes ci-dessous sont mesurées **dominante retirée** (écart au gris rendu
de même L), sur la bande 2 (L de scène 0,72, 12 crans de chroma). Sous ~0,3 de la chroma P3, la dominante noie la
teinte.

[M] Du cran 0,29 au cran 0,96 de la chroma P3 (chroma rendue ~0,035 → 0,09–0,15) :
- **bleus (scène 230–260°) : −9,6° en moyenne** (−4,3 à −15,6), vers le cyan ; le cube : −9,4° ;
- **rouges-oranges (20–60°) : −5,6°** (+0,6 à −13,4), vers le rouge ; le cube : −5,6°. Les oranges 42–58°
  font un crochet au dernier cran : la chroma recule pendant que la teinte perd ~10°.

[M] La rotation est **dans les mesures** : le cube la reproduit à 1–3° près là où il y a des données.
Au-delà de P3 jusqu'au bord Rec.2020 (cube seul), les rouges de scène 17–37° tournent encore de −6 à −21°, les
bleus de 0 à −0,8°. [I] Ces entrées restent à moins de 0,4 pas des lectures mesurées (rendus cliqués voisins) :
c'est de l'interpolation contrainte, pas une extrapolation libre ; une part vient du pied du film synthétique.

[M] Aucun modèle séparable ne la reproduit : bleus / rouges-oranges = A −0,5 / −14,4°, A2 −4,2 / −4,3°,
B −9,4 / −10,8°, P3 −6,5 / −0,8°, contre −9,6 / −5,6° mesurés. Il faut un réglage dédié (§6).

## 4. L'étage d'entrée : clamp, dose 0,6, diaphotie

Simulation [I] : scènes saturées (bord P3 et Rec.2020, trois L, −1/0/+1 IL) → modèle de film → `d = x − x(gris
18 %)` → `x = gain · S · d + neutral` → clamp → cube. Diaphotie caméra simulée par `inverse(S(1))` (Generic C-41).

- **Clamp** [M] : 5,7 % des lectures d'entraînement ont un canal sous −0,15 ; le cube, qui les clampe aussi,
  les rend à ΔE 0,34 / p90 1,01 : la machine y est presque insensible. [I] Gain 1,0 : jamais atteint. Gain 1,3 :
  20 % (P3) à 57 % (Rec.2020) des scènes saturées sans diaphotie, 0 à 13 % avec diaphotie et S 0,6 ; toujours le
  bas, coupe ≤ 0,10 de densité ; écart avec une extrapolation linéaire du bord : ΔE moyen 0,3 à 1,5, max 8,
  chroma −2 à +7 %, teinte 0,3 à 2,9° (7,5° sur les jaunes-verts à +1 IL). Le haut (`reachedHigh`) n'est jamais touché.
- **Diaphotie restante** [M] : `S(0,6) · inverse(S(1)) = 0,6 I + 0,4 inverse(F)`
  = `[[0.893 0.103 0.004] [0.015 0.980 0.005] [0.008 0.030 0.962]]` : 40 % du mélange de la caméra reste.
- **Effet** [I], contre l'entrée sans diaphotie que le cube a apprise : avec S 0,6, chroma ×0,91 à ×0,98
  (×0,95 en moyenne), teinte 1 à 2,7° (bleus +2,2 à +2,7° à L 0,72–0,85), ΔE 0,5 à 3. Sans séparation : chroma
  ×0,71 à ×0,92, teinte jusqu'à 6,8°, ΔE jusqu'à 9. Diaphotie caméra 1,5 fois plus forte : ×0,81 à ×0,94, 5°.
  La dose 0,6 récupère environ deux tiers de la perte.
- **Conséquence** [I] : le cube a appris la machine sur un négatif impossible à obtenir d'un vrai film (zéro
  diaphotie). Un vrai scan, même séparé à 0,6, tombe à l'intérieur du nuage d'entraînement, donc dans une zone
  contrainte, mais ~5 % moins saturé et légèrement tourné. Le vrai SP-3000 voit lui aussi un vrai film à travers
  son propre capteur, avec sa propre matrice interne : la dose 0,6, calée par le code sur des scans réels,
  absorbe peut-être cet écart. Rien ici ne permet de le vérifier ; seule une paire de scans réels (même vue,
  Frontier et caméra) trancherait.

## 5. Saturation et contraste des tons moyens : rien d'aplati par le cube

[M] Cube contre rendus mesurés, tons moyens (L 0,4–0,75, 43 514 lectures) : rapport de chroma 0,999 à 1,003
dans chaque tranche de chroma, écart moyen −0,0001, écart de L ≤ 0,0005. ColorChecker : chroma 0,1015 contre
0,1016 ; grille saturée L 0,55 : 0,1526 contre 0,1522.

[M] Contraste du gris (pente de L entre −1 et +1 IL, sur les 63 rendus) : 0,1710 mesuré, 0,1708 cube (rapport
0,95 à 1,09, moyenne 1,001). Le coin d'IL va de L 0,90 (+2 IL) à 0,07 (−3,5 IL). Seul écart : le pied à −4,5 IL
est relevé (0,035 contre 0,021), effet du lissage.

[I] Les aplatissements possibles sont ailleurs : l'étage d'entrée (×0,95 de chroma, §4) et tout modèle
paramétrique sans étage de teinte (§1, erreurs de teinte de ±5°).

## 6. Conséquences pour le Look Lab [I]

1. Ossature : 3×3 aux lignes de somme 1 (6 réglages) → courbe papier commune (~15 nœuds, éditée en graphe) +
   décalage et gain par canal (6) → rotation de teinte proportionnelle à la chroma et gain de chroma par secteur
   (12 secteurs × 2) → dominante du gris en fonction de la densité (a, b sur 3 à 5 nœuds) → clip. Partir des
   valeurs P1/P3 ci-dessus : ΔE ~1,9 médian dès le départ.
2. Le plancher séparable (~1,6) dit qu'un œil verra encore la différence sur les violets, les verts-cyans et
   les tons moyens saturés. Option honnête : un curseur « caractère mesuré » qui mélange un treillis correctif
   13³ (0,67 médian) appris sur le résidu du modèle paramétrique.
3. Régler en priorité ce qui est mesuré et visible : la torsion des bleus vers le cyan (−10°) et des oranges vers
   le rouge (−6°), la pente au neutre (~1,4 code par unité de densité), la dominante verte du gris moyen.
4. Ne pas chercher la vérité au-delà de ~2 pas des mesures (cyans et jaunes-verts saturés sous gain 1,3) : là,
   le cube comme le modèle inventent.
