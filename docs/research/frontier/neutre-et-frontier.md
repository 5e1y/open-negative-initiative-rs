# Look Lab : valeurs neutres et Frontier, et d'où viennent le bleu violet et la peau jaune

Suite de `analyse-table-frontier.md`. **[M]** = mesuré (sur les rendus réels, le cube, ou une réplique du code),
**[I]** = inféré ou simulé. ΔE = distance Oklab × 100. Figure : `courbe-papier-neutre-frontier.png`.

## 1. Ce que « neutre » veut dire ici

- **Neutre** = la conversion Pro d'ONI, répliquée depuis `pipeline.metal`, `Print.swift`, `Separation.swift`,
  `ChromaDecompression.swift`. Le rouleau est équilibré par Classic, chaque image sur la fenêtre du rouleau.
  Tout le reste est au repos :
  - gains : les médianes alignées ;
  - fenêtres par canal : bord bas = 2e plus basse queue du rouleau, bord haut = 2e plus haute ; gamma 1 ;
    poignées au repos ;
  - décompression de chroma : facteur de rouleau 1, donc passe sautée ;
  - séparation Generic C-41 à 0,6, `chromaKept` ;
  - Lighting : exposition 1,5 (le repos d'un négatif), contraste 0 ; Hunt actif ; Color Density 0 ;
  - aucune courbe, puis clip de sortie et épreuve en sRGB.

  Le rouleau est fait des négatifs synthétiques de −2 à +3 IL. Les queues à 8 % des cellules y sont approchées
  par les centiles 0,5 et 99,5 [I]. La fenêtre verte de ce rouleau vaut 1,60 de densité, donc le gain de
  fenêtre de la face Scanner y vaut 1,3 / 1,60 = **0,81** [M].
- **Frontier** = ajusté sur les 63 rendus réels de la machine [M].
- **Même modèle, même entrée** : la densité qui entre dans le cube. Côté neutre, c'est
  `0,81 · S(0,6) · (x − médiane) + neutral`, pour les 15 680 lectures des 7 négatifs.

  Ossature : matrice 3×3 (lignes de somme 1) → courbe papier commune (15 nœuds) + décalage et gain par canal
  → étage Oklab. L'étage Oklab porte : décalage de teinte, torsion par 0,1 de chroma, gain de chroma
  (12 secteurs), pente de chroma selon L, dominante grise a/b sur 5 niveaux de L.

| Ajustement | ΔE méd. / p90 / p99 |
|---|---|
| Neutre (ONI Pro), papier seul | 0,36 / 1,43 / 2,48 |
| **Neutre (ONI Pro), modèle complet** | **0,32 / 1,17 / 2,13** |
| Jumeau : matrice identité, sans étage Oklab (doit être pire) | 1,26 / 3,43 / – |
| Frontier, papier seul | 1,91 / 4,53 / 8,45 |
| **Frontier, modèle complet** | **1,65 / 4,07 / 7,33** |
| Valeurs Frontier rendues contre la cible ONI Pro, et inversement | 23,0 et 14,9 de médiane |

[M] Le modèle reproduit ONI Pro presque exactement : c'est une bonne origine pour les curseurs. Les deux looks
sont très loin l'un de l'autre, surtout en luminosité : Pro au repos imprime le gris 18 % à L 0,74, le
Frontier à 0,55.

## 2. Tableau des réglages

| Étage | Réglage | Neutre | Frontier | Unité, plage | Ce que ça fait, en clair |
|---|---|---|---|---|---|
| Entrée | Dose de séparation | 0,6 | 0,6 | 0 à 1 | Retire la bavure d'une couche de colorant dans l'autre |
| Entrée | Gain de fenêtre | 1,3 / fenêtre verte | idem | ×, 0,8 à 1,3 mesurés | Étire le contraste du rouleau avant le papier |
| Équilibre | D, C, M, Y | 0 | 0 | clics ±24 | Densité et couleur globales, comme la machine |
| Matrice | R ← V, R ← B | −0,181 ; −0,008 | +0,047 ; +0,024 | densité, ±0,3 | Combien le rouge emprunte aux autres canaux (négatif = sépare, positif = mélange) |
| Matrice | V ← R, V ← B | −0,023 ; +0,041 | +0,100 ; −0,023 | idem | Idem pour le vert |
| Matrice | B ← R, B ← V | +0,053 ; +0,063 | +0,011 ; +0,233 | idem | Idem pour le bleu ; 0,23 de vert dans le bleu, c'est la marque Frontier |
| Papier | Courbe papier | liste §3 | liste §3 | code 0 à 1 | Le contraste et la luminosité de tout le tirage |
| Papier | Gain R, B relatif au vert | 1,088 ; 0,947 | 1,090 ; 0,951 | ×, 0,8 à 1,2 | Contraste propre de chaque canal |
| Papier | Décalage R, B relatif au vert | −0,025 ; −0,119 | −0,050 ; −0,047 | densité, ±0,2 | Équilibre propre de chaque canal ; **principal moteur de la peau** (§4) |
| Papier | Gradation ombres / milieux / hautes lumières | 0 / 0 / 0 | 0 / 0 / 0 | crans −9 à +9 | Retouches au-dessus de la courbe ; la courbe porte déjà le look |
| Couleur | Décalage de teinte, 12 secteurs (0°, 30°, … 330°) | +0,5 +0,6 +0,8 +0,3 +0,7 +1,9 +1,1 −2,1 −3,8 −1,8 +1,0 +1,3 | +0,4 −0,2 −4,7 −0,6 +10,5 +13,0 +2,2 −8,7 −11,2 −10,7 −10,3 −5,8 | degrés, ±20 | Tourne une famille de couleurs, quelle que soit sa saturation |
| Couleur | Torsion par 0,1 de chroma, 12 secteurs | −0,8 −0,4 +0,4 +0,4 +0,1 +0,1 −0,1 −0,9 −0,8 +0,3 +1,0 +0,1 | −3,9 +0,1 +5,9 +2,3 −4,3 −1,2 +7,8 +11,2 +9,8 +10,5 +9,7 +2,3 | °/0,1 C, ±15 | Tourne davantage les couleurs les plus vives |
| Couleur | Gain de chroma, 12 secteurs | 1,00 1,00 1,01 1,03 1,04 1,06 1,08 1,07 1,01 0,95 0,94 0,98 | 0,96 0,89 0,95 1,00 0,91 0,83 0,97 1,14 1,12 1,00 1,00 1,03 | ×, 0,5 à 1,5 | Rend une famille plus ou moins vive |
| Couleur | Saturation selon L | +0,02 | +0,06 | par unité de L, ±0,3 | Plus de couleur dans les clairs que dans les sombres |
| Couleur | Saturation globale | 0 | 0 | crans −9 à +9 | Réserve pour l'œil ; le modèle n'en a pas besoin |
| Couleur | Dominante du gris a, à L 0,1 / 0,3 / 0,5 / 0,7 / 0,9 | +0,005 +0,004 +0,002 +0,003 +0,002 | +0,009 +0,002 −0,009 +0,002 +0,004 | Oklab, ±0,03 | Teinte rouge (+) ou verte (−) des gris selon leur clarté |
| Couleur | Dominante du gris b, mêmes niveaux | +0,001 −0,003 −0,001 −0,002 −0,001 | +0,007 +0,004 +0,011 +0,012 +0,004 | Oklab, ±0,03 | Teinte jaune (+) ou bleue (−) des gris |
| Couleur | Teinte des ombres | = dominante à L 0,1 | = dominante à L 0,1 | | Le premier point de la dominante |
| Local | Hypertone | arrêt | à régler à l'œil | marche/arrêt | Compression locale du contraste ; ONI n'a pas d'équivalent, aucun rendu mesuré ne l'isole |
| Local | Hyper-sharpness | arrêt (bloc Details d'ONI) | à régler à l'œil | Low 3 à High 3 | Netteté de la machine ; hors de la table de couleur |
| Fin | Caractère mesuré (treillis 13³) | 0 | 1 | 0 à 1 | Ce que le modèle ne sait pas dire ; à 1, le cube mesuré |
| Fin | Point blanc | clip | clip | | Les deux tondent net à 1 |

Lecture des secteurs [I] : ce sont les teintes Oklab **en sortie de la courbe papier**, pas celles de la scène :
0° rose-rouge, 60° orange-peau, 90° jaune, 150° vert, 240° bleu, 270° bleu-violet, 300° violet. Le décalage
s'applique en entier dès C 0,03, la torsion grandit avec la chroma. Les deux ajustements ne partagent pas la
même jauge entre matrice, courbe et étage : une valeur seule ne se compare pas d'un look à l'autre, le jeu
entier oui.

## 3. Courbe papier, deux listes de points [M]

Code sRGB du canal vert en fonction de la densité d'entrée du cube après la matrice. Rouge et bleu s'en
déduisent par les gains et décalages relatifs.

| Densité | −0,200 | −0,043 | 0,114 | 0,271 | 0,429 | 0,586 | 0,743 | 0,900 | 1,057 | 1,214 | 1,371 | 1,529 | 1,686 | 1,843 | 2,000 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Neutre | 0,000 | 0,000 | 0,139 | 0,341 | 0,519 | 0,659 | 0,771 | 0,860 | 0,929 | 0,984 | 1,000 | 1,000 | 1,000 | 1,000 | 1,000 |
| Frontier | 0,001 | 0,001 | 0,015 | 0,059 | 0,129 | 0,272 | 0,475 | 0,680 | 0,825 | 0,906 | 0,950 | 0,974 | 0,986 | 0,998 | 1,000 |

[I] La courbe neutre est l'image de Pro au repos sur ce rouleau : une épaule douce, des noirs posés au pied du
film, plus claire d'environ 1,8 IL. Sa position horizontale dépend du gain de fenêtre (0,81 ici).

## 4. D'où viennent le bleu violet et la peau jaune

**Montage** [I]. Les scènes passent par le film synthétique, puis par une diaphotie caméra (`inverse(S(1))`,
puis 1,5 fois plus), puis par le vrai chemin de la face Scanner. Ce chemin est :
`d = x − x(gris 18 %)`, gain, `S(dose)`, clamp, cube, clip par canal, puis `ScannerFinish` au repos et le clip
de sortie.

- Bleus : 96 scènes, h 230 à 265°, L 0,4 à 0,7, 50 à 100 % de la chroma P3.
- Peaux : ColorChecker peau sombre et claire, Monk 3 à 8, et 72 peaux h 40 à 75°, L 0,5 à 0,8, C 0,04 à 0,10.
- Chaque scène à −1, 0, +1 IL.
- Effets signés, moyennés sur les trois expositions, une étape changée à la fois depuis la configuration de
  l'app (diaphotie caméra, gain 1, dose 0,6). Pour l'étape machine, la référence est la même scène rendue par
  ONI Pro sans diaphotie, à luminosité égale (exposition −0,27 pour le gris à L 0,548).

| Étape | Bleus Δh | Bleus ΔC | Peaux Δh | Peaux ΔC | Peaux Δa | Peaux Δb |
|---|---|---|---|---|---|---|
| **Le cube Frontier, contre ONI Pro à L égale** | **+14 à +20°** | +0,01 à +0,03 | **+18 à +20°** | **−0,006 à −0,017** | −0,026* | −0,003* |
| Diaphotie caméra (contre aucune) | +4,0° | −0,013 | +5,0° | −0,010 | −0,008 | −0,008 |
| Diaphotie 1,5 fois plus forte (contre caméra) | +3,1° | −0,008 | +3,3° | −0,005 | −0,004 | −0,004 |
| Dose 0,6 (contre 0) | −2,8° | +0,008 | −2,9° | +0,005 | +0,003 | +0,004 |
| Dose 1,0 (contre 0,6) | −1,4° | +0,005 | −1,6° | +0,003 | +0,002 | +0,003 |
| Gain 1,3 (contre 1,0) | +0,5° | +0,009 | +0,9° | +0,006 | +0,001 | +0,005 |
| Gain du rouleau 0,81 (contre 1,0) | +0,1° | −0,010 | −0,5° | −0,005 | −0,001 | −0,005 |
| Clamp (contre extrapolation) | 0,0° | 0 | 0,0° | 0 | 0 | 0 |
| Clip par canal (contre sans) | 0,0° | 0 | 0,0° | 0 | 0 | 0 |
| `ScannerFinish` au repos, clip de sortie | 0 (aller-retour exact à 3e-16) | | 0 | | | |

\* contre la scène, pas contre Pro.

**Résultat, bleus** [M sur le cube, I pour la scène] :
- La scène à 247° sort à 259–264° (gain 1) ; Pro la garde à 243–246°.
- 15 à 33 % des bleus passent 270°, jusqu'à 294°. Les bleus de scène à 265° sortent à 277–282°.
- **L'observation est reproduite**, et la cause est le cube lui-même, pas l'entrée : diaphotie +4°, la dose 0,6
  en reprend −2,8°, gain et clamp presque rien.
- La « torsion vers le cyan » du premier rapport (−9,6°) se mesurait **le long** d'une ligne de chroma. Elle
  est réelle mais plus petite que le **décalage d'ensemble vers le violet** de la machine. Les deux mesures sont
  vraies en même temps.

**Résultat, peaux** [M sur le cube et les rendus, I pour la scène] :
- Scène h 58°, C 0,068. App Frontier : h 77–80°, C 0,049–0,058, a +0,008, b +0,048 à +0,056. Pro dans l'app :
  h 62–63°, C 0,057–0,059.
- La peau Frontier est donc **plus jaune de ~15°** et **moins rouge** (a divisé par ~4 face à la scène), avec
  15 à 25 % de chroma en moins. C'est « urine plutôt que doré ».
- Mesuré directement sur le rendu Base de la machine [M] : peau claire 46° → 97°, peau sombre 44° → 92°
  (absolu, dominante de la charte comprise). Même en retirant la dominante du gris moyen, encore +16 et +26°.
  La machine elle-même a rendu jaunes les peaux de ce négatif.
- **La dominante vert-jaune** (a −0,022, b +0,021) **n'explique pas** la peau jaune [M] :
  - elle est propre au réglage Base sur la charte ;
  - sur l'axe neutre de l'app (`neutral + t`), le cube est neutre au gris moyen par construction ;
  - aux clartés de la peau (L 0,55 à 0,86), il est légèrement chaud : a 0 à +0,010, b 0 à +0,007, C ≤ 0,011,
    vers 24–64°, pas vert.
- **La perte de chroma de l'entrée** joue, mais peu : diaphotie −0,010, dose 0,6 +0,005, net −0,005, soit
  ~8 % de la chroma de peau et +2° vers le jaune [I].
- Ce qui pèse est la machine : +18° et −0,006 à −0,017 de chroma contre Pro à luminosité égale.
- Dans le modèle Frontier [M], la peau dépend surtout de l'équilibre papier par canal. Rendre décalages et gains
  communs la ramène de −29° (trop loin), et la matrice identité de −9,6°. Les réglages de teinte par secteur n'y
  font que ±2°. **Rangée « peau » du tableau : « Décalage R, B relatif au vert ».**

**Pourquoi la machine fait ça à ce négatif** [I, non vérifiable ici] : le négatif synthétique n'a ni diaphotie,
ni masque coloré réel, ni les gammas d'un vrai film (0,60 / 0,64 / 0,70, sensibilités Rec.2020). Une machine
réglée pour de vrais films Fuji et Kodak corrige des bavures qui n'y sont pas : elle retire du rouge à la peau
et pousse les bleus vers le violet. Notre cube apprend fidèlement cette réponse, puis l'applique à de vrais
scans. Mon modèle de diaphotie caméra, seul, ne la défait pas : il pousse dans le même sens (+4 à +5°). La
correction se règle donc à l'œil, par la matrice et l'équilibre papier, jusqu'à ce qu'une paire de scans réels
(même vue, Frontier et caméra) permette de la mesurer.

## 5. Ce que la mesure ne peut pas dire

- Ce que le vrai SP-3000 fait à un **vrai** film : aucun rendu mesuré n'en contient un. Le « doré » des labos
  n'est donc ni reproduit ni réfuté ici, seulement la dérive du cube.
- Les rouleaux du propriétaire : fenêtre verte réelle, diaphotie de sa caméra et de sa lumière. J'ai pris un
  gris 18 % comme médiane et le gain 1,0 ou 1,3.
