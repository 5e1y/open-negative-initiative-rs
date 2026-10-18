# Optimiser le portage Rust d'ONI à outrance

Recherche du 2026-10-17 pour le portage `open-negative-initiative-rs` (wgpu 30 + WGSL, LibRaw, Slint, rusqlite ; macOS et Windows). Jugé comme l'app Swift : 99e centile, pire trou, images au-delà de 8,3 ms. Les points sur Slint ont été vérifiés dans son code source (master 1.19, la dernière version publiée est la 1.18.1), parce que sa documentation ne dit presque rien des performances. Les lignes marquées **[déduit]** sont un calcul ou une inférence, pas un chiffre publié.

## La réponse courte

Oui. Rust va aussi loin que Swift, et plus loin sur trois points : aucun comptage de références implicite ni ramasse-miettes, la main sur chaque allocation, et des threads que le compilateur garantit sans course de données. Mais comme sur Swift, presque tout le gain vient de l'architecture et non du langage. Les deux grandes leçons de l'app Swift (le découpage de l'UI, et ne jamais rien recalculer ni réallouer pendant un geste) se transposent toutes les deux, et Slint en donne la première à moitié par construction.

## À trancher tôt : six limites de Slint pour une app photo

Elles se lisent dans le code de Slint et changent l'architecture ; mieux vaut les connaître avant d'écrire l'écran d'édition.

1. **Le chemin vers l'écran est en 8 bits.** Une texture wgpu importée dans Slint doit être `Rgba8Unorm` ou `Rgba8UnormSrgb`, et la swapchain est forcée en `Rgba8Unorm`/`Bgra8Unorm` (`internal/core/graphics/wgpu_30.rs`). Pas de `rgba16float`, pas d'EDR. La chaîne flottante doit donc finir par un noyau d'affichage qui écrit 8 bits avec un tramage, en `Rgba8Unorm` (la variante sRGB ne peut pas être une cible de stockage). **Ce n'est pas un recul** : le canevas de l'app Swift affiche déjà en 8 bits (`colorPixelFormat = .bgra8Unorm`, `MetalImageView.swift:42`).
2. **Aucun espace colorimétrique sur la `CAMetalLayer`.** Slint n'en pose pas, alors que l'app Swift étiquette la sienne avec l'espace de l'écran (`MetalImageView.displaySpace`). La conversion vers l'écran (P3 compris) est donc à écrire dans notre noyau d'affichage, d'après le profil de l'écran. **[à vérifier sur la machine]** Une couche sans espace n'est pas convertie par macOS, donc des valeurs déjà converties devraient s'afficher telles quelles.
3. **Sur GPU, chaque image redessine toute la fenêtre.** Le rendu partiel n'existe que pour le moteur logiciel. À 120 Hz, tout le panneau est repeint pendant un glissement : il faut le garder peu coûteux (voir §1).
4. **Sous Windows, la cadence vient d'une minuterie en millisecondes entières** (8 ms à 120 Hz, 6 ms à 144 Hz), non calée sur le balayage, avec un mode de présentation figé en `AutoVsync` et une latence de 2 images non réglable. Attendre de la gigue ; la mesurer avec PIX avant d'y croire. Sur macOS en revanche, la cadence suit ProMotion par un `CADisplayLink` (macOS 14+).
5. **Aucune grille virtualisée.** `ListView` ne crée que les lignes visibles d'une liste verticale ; un `for` dans une `GridLayout` crée tout. Un rapport non résolu : 1 000 lignes de tableau figent l'UI ([discussion #3601](https://github.com/slint-ui/slint/discussions/3601)).
6. **L'API d'interop wgpu est instable** (`unstable-wgpu-30`) : elle a suivi wgpu 24 → 26 → 27 → 28 → 29 → 30 en un an environ. Le graphe de rendu doit épingler la même version majeure de wgpu que Slint.

**La sortie qui lève 1, 2 et 4 d'un coup** (depuis Slint 1.18) : `SkiaWGPU30Renderer::render_to_texture` dessine l'UI Slint dans **notre** texture wgpu (elle-même en 8 bits, ce qui suffit pour l'interface). On possède alors la fenêtre, la swapchain, le mode de présentation, une latence de 1, une swapchain à attente sous Windows ou un `CAMetalDisplayLink` sur macOS, et la composition finale : la photo va à l'écran en flottant, l'UI se pose par-dessus. wgpu 30 sait le faire (lu dans `wgpu-hal` 30.0.1) : sur Metal, une surface `Rgba16Float` ou `Rgb10a2Unorm` en `ExtendedDisplayP3`, `DisplayP3`, sRGB étendu linéaire ou BT.2100, l'EDR activé et la marge EDR de l'écran lisible ; sous DX12, `Rgba16Float` en scRGB ou `Rgb10a2Unorm` en HDR10, appliqués par `SetColorSpace1` (DXGI n'a pas d'espace P3 : le P3 passe par le scRGB). Le prix : une plateforme Slint maison (événements, saisie IME, accessibilité). **[déduit]** Une voie intermédiaire, une vue native à nous pour le seul canevas posée dans la fenêtre Slint, coûterait moins mais cacherait tout ce que Slint dessine par-dessus le canevas (menus, bulles, poignées de recadrage). Garder le noyau d'affichage isolé permet de changer de voie plus tard sans réécrire la chaîne.

## Les optimisations, par impact attendu sur ONI

### 1. Le découpage de l'UI, version Slint

Slint suit les dépendances propriété par propriété : un changement ne marque sales que les liaisons qui lisent cette propriété, recalculées paresseusement, et écrire la valeur déjà en place ne fait rien (`Property::set` compare d'abord). Le coût que le découpage SwiftUI corrigeait (un cran qui réévaluait 4 900 lignes) n'existe donc pas par conception ([réactivité Slint](https://docs.slint.dev/latest/docs/slint/guide/language/concepts/reactivity/)). Il revient pourtant si on écrit mal :

- **`PipelineSettings` reste en Rust**, l'équivalent d'`EditorStore`. Slint ne reçoit que les scalaires qu'un contrôle affiche.
- **Une propriété par valeur affichée**, dans un `export global` par exemple. **Jamais tous les réglages dans une seule propriété `struct`** : lire `s.exposure` dépend de tout `s` **[déduit du code généré]**, et une écriture salirait toutes les liaisons, c'est-à-dire exactement le piège SwiftUI.
- **Une écriture par cran** : le rappel du curseur écrit le store Rust, puis ne repose que l'étiquette de valeur.
- **Ce qui bouge à son propre rythme est publié à part** (histogramme, compteurs de chargement, taille du canevas), par un `slint::Timer` à cadence fixe : l'équivalent de `Plotted`, `CanvasSize` et `LoadProgress`.
- **Les rappels `changed` avec parcimonie** : ils forcent l'évaluation d'une liaison sinon paresseuse et sont différés d'un tour ([billet Slint](https://slint.dev/blog/property-changed-callback)). Jamais comme colonne vertébrale du flux de données.
- **Les modèles se mettent à jour ligne par ligne** (`set_row_data` ne reconstruit que sa ligne ; `set_vec` reconstruit tout).
- **Garder la repeinte complète bon marché** (limite 3) : `cache-rendering-hint: true` sur les panneaux immobiles, peu d'appels de dessin, pas de dégradés (ils cassent le regroupement de Skia), des étiquettes de valeur de largeur fixe pour qu'un nouveau chiffre ne relance pas la mise en page **[déduit]**. Peu d'éléments au total, et Slint 1.18 ou plus, qui a corrigé deux lenteurs sur les grands arbres (#13043, #13049).
- **Mesurer** : `SLINT_DEBUG_PERFORMANCE=refresh_full_speed,console` donne le plafond, `refresh_lazy` vérifie que rien ne repeint quand rien ne change ([débogage Slint](https://docs.slint.dev/latest/docs/slint/guide/development/debugging_techniques/)).

### 2. Du curseur au rendu : le dernier gagne, un seul rendu en vol

Ce que `decodeLatest` faisait pour les décodages, appliqué à chaque cran :

1. **Le rappel du curseur écrit une case « dernière valeur »**, jamais une file : `triple_buffer` (0,9 ns en lecture, 6,9 ns en écriture, [docs](https://docs.rs/crate/triple_buffer/latest)) ou `arc-swap` pour un instantané lu souvent ([docs](https://docs.rs/arc-swap)).
2. **Un seul rendu en vol, un seul en attente.** Un thread prend la case, encode et soumet sur la file **partagée** avec Slint (une seule file ordonne le travail, pas de barrière nécessaire), sur 2 ou 3 textures de sortie en rotation.
3. **Un numéro de génération** (`AtomicU64`) : un travail plus vieux que le dernier est abandonné à l'encodage, et un travail CPU vérifie entre deux bandes.
4. **Ne poser le résultat qu'au rythme de l'écran** : dans `BeforeRendering`, si une texture plus récente est finie, l'image est remplacée, et `request_redraw` n'est appelé **que** s'il y a du neuf (l'exemple officiel l'appelle à chaque image et tourne pour rien).
5. **Pendant le geste, la prévisualisation ; au lâcher, la pleine résolution**, comme `fullIfReady` sur Swift.
6. **`slint::Image` n'est pas `Send`** : on revient au thread de l'UI par `invoke_from_event_loop`, avec un rappel trivial, puisque les événements et la peinture partagent ce thread.

### 3. La bande passante décide, donc la pleine résolution se rend en région

**[déduit]** Une image 47 Mpx en `rgba32float` pèse 752 Mo ; une passe qui la lit et l'écrit déplace 1,5 Go. Avec 120 Go/s sur un M4 de base (273 sur un M4 Pro, 410 à 546 sur un M4 Max, [Apple M4](https://en.wikipedia.org/wiki/Apple_M4)), une seule passe pleine coûte 12,5 ms, au-delà des 8,3 ms d'une image à 120 Hz. **Rejouer la chaîne sur toute l'image à pleine résolution ne tiendra jamais 120 Hz** ; l'auteur de vkdt dit la même chose : « je suis surtout limité par la bande passante » ([journal vkdt](https://discuss.pixls.us/t/vkdt-devel-diary/19505)). La prévisualisation de 2000 × 1333 (43 Mo par texture) coûte environ 0,7 ms par passe : une chaîne fusionnée y tient largement.

- **À pleine résolution, ne rendre que la région visible plus une marge** égale à la somme des rayons des filtres de voisinage (les tuiles chevauchantes de Halide). C'est le chemin de darktable ([pixelpipe](https://docs.darktable.org/usermanual/5.6/en/darkroom/pixelpipe/the-pixelpipe-and-module-order/)), et ce que `Viewport.needsFullResolution` faisait déjà sur Swift.
- **Les statistiques globales** (histogramme, touches automatiques) se lisent sur l'image entière ou la pyramide, jamais sur la région. Le flat-field reste dans le repère de son masque.
- **Une pyramide** : la tête pleine résolution plus des niveaux 1/2 et 1/4, comme les `lod` de vkdt. On glisse sur le niveau d'écran, on affine ensuite.
- **Repère** : vkdt rend un RAW de 24 Mpx en entier, dématriçage compris, en 17 ms, dont environ 6 ms de préparation CPU du graphe ([journal](https://discuss.pixls.us/t/vkdt-devel-diary/19505)). **[déduit]** Environ 30 ms pour 47 Mpx sur un GPU comparable.

### 4. Fusionner tout ce qui est pixel par pixel

Le noyau 4–7 l'est déjà. Les étapes pixel par pixel restantes (courbes, saturation, vignette, découpe de sortie) peuvent le rejoindre, et une texture intermédiaire ne s'écrit qu'avant un filtre de voisinage (débruitage chroma, texture, netteté) ou à une frontière de cache. Halide mesure qu'en évitant le parcours une-passe-par-étape, on gagne environ 10× pour un surcoût de 6 % de calcul, et sa chaîne d'appareil photo tourne en 14 ms contre 49 ms écrite à la main ([Halide PLDI'13](https://people.csail.mit.edu/jrk/halide-pldi13.pdf)). Moins de passes veut aussi dire moins de barrières : retirer 15 barrières redondantes sur 16 passes de calcul a gagné environ 30 % de temps GPU ([arXiv 2607.26506](https://huggingface.co/papers/2607.26506)).

### 5. Le cache du graphe : relancer seulement l'aval

Un drapeau sale par nœud, et seul l'aval du réglage changé repart. Deux têtes à garder : après décodage, flat-field, recadrage et poussières (coûteux, rarement touchés), et l'entrée des filtres de voisinage. Le reste est recalculé par le noyau fusionné. vkdt rejoue tout le graphe, ce qui coûte peu sur NVIDIA mais des centaines de millisecondes sur un vieil Intel ([pixls](https://discuss.pixls.us/t/processing-that-sucks-less/13016/171)) ; **[déduit]** avec des GPU intégrés sous Windows dans la cible, le cache s'impose.

### 6. Ne jamais allouer pendant un geste

- **Demander les limites de l'adaptateur** : par défaut `max_texture_dimension_2d` vaut 8192, alors qu'une image du S1R fait 8368 px de large ; Metal et DX12 vont à 16384. Un tampon de stockage de 752 Mo dépasse aussi tous les défauts ([Limits](https://wgpu.rs/doc/wgpu/struct.Limits.html)). Au-delà de 16384 px (le pixel-shift), on tuile, comme `RawDecode.textureSide` sur Swift.
- **Un réservoir de textures** indexé par (largeur, hauteur, format, usage). wgpu efface d'office toute texture neuve qu'un shader écrit avant de l'avoir remplie ([init_tracker](https://wgpu.rs/doc/wgpu_core/init_tracker/index.html)), et un `create_buffer` a été mesuré à environ 4 ms ([discussion #5525](https://github.com/gfx-rs/wgpu/discussions/5525)).
- **Libérer tout de suite** : `Texture::destroy()` rend la mémoire native sans attendre la dernière référence. C'est l'équivalent de `Slot.retire`, qui sur Swift évitait 6,6 Go après douze tours de zoom.
- **`MemoryHints::Manual`** avec de grands blocs, pour que les textures de 750 Mo soient des allocations dédiées et que les petits tampons ne fragmentent pas ([MemoryHints](https://wgpu.rs/doc/wgpu/enum.MemoryHints.html)). Aucune API wgpu ne donne le budget mémoire sur Metal : on reprend la règle de `MemoryBudget`, une part de la mémoire unifiée.
- **Côté CPU, le même principe** : un réservoir de tampons pleine taille gardés toute la session. **[déduit]** Une première écriture sur 752 Mo coûte environ 46 000 défauts de page sur Apple Silicon (pages de 16 Kio) et 184 000 sous Windows (4 Kio), chacun remis à zéro par le noyau ; et `vec![0.0; n]` paraît gratuit parce qu'il repousse ce coût à la première écriture ([darkcoding](https://darkcoding.net/software/rust-zeroed-vector-allocation/)).
- **Montée de l'image** : un tampon `mapped_at_creation` où LibRaw écrit directement, puis une copie vers une texture privée, une fois par image et jamais par cran.

### 7. Les paramètres d'un cran en « immediates », les bind groups en cache

- Les paramètres du curseur (≤ 128 octets) passent en immediates (`var<immediate>`, `set_immediates`, l'ancien push constants renommé en wgpu 28, [notes de version](https://github.com/gfx-rs/wgpu/releases/tag/v28.0.0)) plutôt qu'en `write_buffer` plus un changement de bind group. Il faut la fonctionnalité `IMMEDIATES` et l'envelopper dans une struct (un défaut connu sous DX12).
- Les bind groups se créent une fois par (pipeline, jeu de textures) et se gardent tant que la texture n'est pas réallouée ([notes wgpu](https://hackmd.io/@cwfitzgerald/wgpu-bindless)).
- `PipelineCache` ne fonctionne que sous Vulkan. Les pipelines se créent donc tous au démarrage, sur un thread d'arrière-plan, jamais dans la boucle d'un geste.
- Les écritures `write_*` faites depuis un autre thread attendent le prochain `submit` (un `submit` vide les envoie).

### 8. Les filtres de voisinage : groupes, localité, mémoire partagée

- **Groupes de 8 × 8** pour les noyaux pixel par pixel (AMD écrit ses images par blocs de 8 × 8, [guide RDNA](https://gpuopen.com/download/RDNA_Architecture_public.pdf) ; Apple a des SIMD de 32), **16 × 16** pour un filtre avec marge, qui l'amortit mieux **[déduit]**.
- **Réordonner les groupes en bandes** d'environ 16 groupes : +47 % mesurés par NVIDIA sur un débruiteur à large noyau, avec un taux de succès du cache L2 passé de 63 % à 86 % ([NVIDIA](https://developer.nvidia.com/blog/optimizing-compute-shaders-for-l2-locality-using-thread-group-id-swizzling)). Cela vaut pour le débruitage chroma, la texture et la netteté.
- **La mémoire partagée n'aide un flou séparable qu'à partir d'un rayon de 9 environ**, et nuit en dessous ([mesures Sabourin](https://github.com/AlexSabourinDev/cranberry_blog/blob/master/ATaleOfTooManyBlurs_Part2_SeparableKernels.md)). Sur Apple, le cache de texture absorbe déjà la réutilisation ([forum Apple](https://developer.apple.com/forums/thread/690208)). Il n'y a pas de gagnant universel : on mesure par backend.
- `rgba32float` n'est pas lisible-inscriptible en stockage sans fonctionnalité propre à l'adaptateur : ping-pong entre deux textures, `textureLoad` d'un côté, écriture seule de l'autre.

### 9. L'histogramme sur le GPU, en retard d'une image

Un histogramme privé par groupe de travail (`var<workgroup>` et atomiques), fusionné ensuite : environ 2× plus rapide que les atomiques globaux ([NVIDIA](https://developer.nvidia.com/blog/parallelforall/gpu-pro-tip-fast-histograms-using-shared-atomics-maxwell)). Des cases `u32`, car les atomiques flottants n'existent pas sous DX12. La relecture passe par `map_async` sans jamais attendre, l'histogramme affiché a une image de retard, et sa cadence reste limitée et non différée, comme `HistogramThrottle`.

### 10. La galerie : dessiner la grille soi-même

L'équivalent des tuiles-calques et de `GalleryLayout` : un noyau wgpu copie les vignettes, depuis un atlas ou un tableau de textures, dans **une seule** texture affichée par **une seule** `Image`. Une seule `TouchArea` convertit la position en numéro de cellule par calcul, et le défilement est un décalage dans un tampon d'uniformes. Coût : un appel de dessin et zéro élément Slint par tuile ; la sélection, le survol et le clavier deviennent notre travail. Le repli plus simple, une `ListView` de rangées dont chacune porte N colonnes, ne virtualise que par rangée.

### 11. Les threads, et leur priorité

- **Des pools séparés, jamais le thread de l'UI** : un pool rayon pour le CPU interactif, un second pool rayon à basse priorité pour les vignettes et l'indexation, des `std::thread` simples pour les entrées-sorties bloquantes et SQLite. tokio n'apporte rien sans réseau.
- **Des bandes de lignes** (`par_chunks_mut`), avec `with_min_len` ; viser des tâches de 50 à 500 µs **[déduit]**.
- **macOS** : la classe QoS par thread (`pthread_set_qos_class_self_np`, dans le `start_handler` du pool). `USER_INITIATED` pour le rendu, `UTILITY` pour les vignettes et l'indexation. **Jamais `BACKGROUND` pour une passe de bibliothèque** : il reste sur les cœurs E à fréquence quasi nulle ([Eclectic Light](https://eclecticlight.co/2026/02/17/in-the-background-putting-threads-to-work/)).
- **Windows** : `SetThreadInformation(ThreadPowerThrottling)` marque l'indexation en EcoQoS et les threads de rendu en haute priorité de façon explicite ; sans marque, Windows baisse la priorité d'une fenêtre masquée ([Microsoft](https://learn.microsoft.com/windows/win32/procthread/quality-of-service)).

### 12. La compilation

- **Profil release** : `lto = "fat"` (10 à 20 % selon le [Rust Performance Book](https://nnethercote.github.io/perf-book/build-configuration.html)), `codegen-units = 1`, `debug = "line-tables-only"` (le dépôt a déjà `debug = 1`). `panic = "abort"` est un peu plus rapide, mais une panique dans un thread de travail tue l'app et ce qui n'était pas enregistré : à discuter.
- **PGO** (`cargo pgo`) : environ 10 % selon le Performance Book, 19,8 % sur la chaîne d'outils Rust d'Android ([Android](https://android-developers.googleblog.com/2023/12/faster-rust-toolchains-for-android.html)). À entraîner sur de vrais 47 Mpx, un défilement de galerie et un export. **BOLT est hors jeu** : il ne traite que les binaires ELF, ni Mach-O ni PE.
- **CPU cible** : macOS vise déjà `apple-m1`, rien à ajouter. Sous Windows, la base est SSE2 ; plutôt que relever tout le binaire en AVX2, on multiversionne les boucles chaudes (crate [`multiversion`](https://docs.rs/multiversion)).
- **SIMD** : `std::simd` reste réservé à nightly ; `fearless_simd`, `pulp` ou `wide` selon le [panorama 2026](https://shnatsel.github.io/state-of-simd-rust-2026/). Peu de noyaux restent sur CPU, donc peu d'enjeu.
- **Builds de développement** : `opt-level` 1 à 3 pour les crates d'image même en dev (une boucle de pixels non optimisée est inutilisable), `split-debuginfo = "unpacked"` sur macOS. **Mesurer uniquement sur le profil final**, la leçon de `make hand`.

### 13. Entrées-sorties, index, vignettes

- **SQLite** : `journal_mode=WAL`, `synchronous=NORMAL` (l'index est un cache jetable, une coupure perd au pire les dernières transactions), `mmap_size` de 256 Mo, `prepare_cached`, une transaction par lot, un rédacteur et plusieurs lecteurs, et les checkpoints faits sur le thread d'entrées-sorties plutôt qu'au commit ([WAL](https://www.sqlite.org/wal.html)). Par lots, rusqlite passe de 3 min à 34 s sur 100 millions de lignes ([avi.im](https://avi.im/blag/2021/fast-sqlite-inserts/)).
- **Vignettes** : l'aperçu JPEG embarqué du RAW (`unpack_thumb`) plutôt qu'un dématriçage, et un décodage JPEG réduit dans le domaine DCT. `zune-jpeg` est à mesurer sur Apple Silicon : sa documentation ne cite pas NEON.
- **Empreintes** : `blake3` monte à 14 Go/s avec SIMD et rayon sur un M1 ([iroh](https://iroh.computer/blog/hashing-multiple-blobs-with-BLAKE3)) ; une empreinte tête, queue et taille évite de lire 60 Mo par fichier.
- **Surveillance de dossiers** : `notify` avec `notify-debouncer-full`, chaque événement voulant dire « relire ce dossier », jamais une vérité (FSEvents fusionne et désordonne).
- **Sidecars** : `serde_json::from_slice` après un seul `fs::read` suffit probablement ; sonic-rs est environ 2,7× plus rapide sur ses propres mesures, à ne prendre que si un profil le réclame.

### 14. fp16 : seulement pour l'affichage, et à argumenter

`rgba16float` divise la bande passante par deux, et `SHADER_F16` existe sur Metal et DX12 (DX12 demande SM 6.2 et DXC). Sur M1 le calcul f16 n'est pas plus rapide que f32 ([metal-benchmarks](https://github.com/philipturner/metal-benchmarks)) : tout le gain est en bande passante. Cela heurte la règle 2 (`rgba32float` entre chaque étape), donc **à réserver aux copies d'affichage** (la pyramide, la copie épreuvée), ce que l'app Swift faisait déjà avec son cache de canevas en `rgba16Float`. C'est une décision à argumenter, pas un changement discret.

### 15. Mesurer

- **Tracy** (`tracing-tracy`, environ 2 ns par zone) avec une `FrameMark` après la présentation, et **wgpu-profiler** pour les portées GPU exportées vers Tracy ([wgpu-profiler](https://github.com/Wumpf/wgpu-profiler)). Sur Apple, les timestamps dans une passe n'existent pas : on mesure aux frontières de passe, donc une passe par étape le temps du profilage.
- **Dans l'app** : un `Instant` par image et par latence curseur-vers-pixels dans un `hdrhistogram`, pour sortir le 99e centile, le pire trou et le nombre d'images au-delà de 8,3 ms, comme sur Swift.
- **macOS** : Instruments (Metal System Trace pour la chronologie des drawables, Animation Hitches). **Windows** : PIX Timing Captures, où se verra la minuterie de 8 ms ([PIX](https://devblogs.microsoft.com/pix/timing-captures)), et Superluminal.
- **CPU** : `samply` sur les deux systèmes ; `dhat-rs` pour compter les allocations dans les tests.

## Les skills Rust

Rien qui mérite d'être installé. Aucun skill ni plugin ne couvre wgpu, WGSL ou Slint. Les deux plus visibles sont généralistes : `actionbook/rust-skills` (communautaire, environ 1 000 à 1 400 étoiles, idiomes et propriété, rien sur le GPU ni le SIMD ni le profilage) et « Rust CLI Skills » de l'annuaire Anthropic (communautaire, pensé pour des outils en ligne de commande). Les sources citées ici, le Rust Performance Book en tête, valent mieux. **[déduit]** Le plus utile serait un skill propre au projet, écrit depuis cette note et les mesures réelles, une fois que le portage en aura.

## Les pièges de l'app Swift qui se transposent tels quels

- Un seul dématriçage à la fois, le dernier demandé gagne (`decodeLatest`).
- Une prévisualisation matérialisée une fois, la copie de mesure taillée dedans.
- La pleine résolution seulement quand la main se pose ; un panoramique erratique en reconstruisait une dix fois par seconde (11 Go).
- Le budget de cache lu sur la machine (`MemoryBudget`), épinglé pendant les contrôles pour que les chiffres restent reproductibles.
- Aucune liste de dossier sur le chemin d'écriture, aucune lecture EXIF sur le thread de l'UI.
