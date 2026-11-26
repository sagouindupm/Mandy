# Device slots 0x1c8 and 0x218: pipelines the title deserializes

Rétro-ingénierie Sagouindupm | VIctory, 2026-11-26 (RTX 3050, Series S, titre 1.0.2.1).
Données relevées avec `VICTORY_DUMP_UNKNOWN_PIPELINES=<dossier>` (rien du jeu dans ce dépôt).

## Ce que c'est
- **0x218** = `DeserializeComputePipelineStateX` (comme Marvisak/VIctory#2396 l'a vu pour l'upscaler AMD).
  Description : `root, (paquet, 0x1c), (hash, 0x14), (code GFX10, n), (métadonnées, m), ...`.
- **0x1c8** = sa version graphique (`DeserializeGraphicsPipelineStateX`, nom supposé). Aucun fork ne l'implémente.
  Mesuré : les ~24 500 appels tombent tous en 2 s au chargement, environ 12 000 pipelines distinctes demandées
  18 fois chacune, aucune relance par image. C'est le cache de pipelines précompilées du jeu ; il se rabat ensuite
  sur la création classique (DXIL), que nous traduisons. Gain attendu à l'implémenter : temps de chargement et
  compilation de shaders, pas l'image.

## Description 0x1c8 (mots de 64 bits)
| mot | contenu |
|---|---|
| 0 | root signature (notre objet) |
| 1, 2 | paquet de registres, 0x184 à 0x200 octets |
| 3, 4 | format d'entrée des sommets (noms `POSITION`, `COLOR`, `TEXCOORD` en clair), 0x50 à 0xbc |
| 5-8 | étage 0 = **pixel shader** : code GFX10, taille ; métadonnées, taille |
| 9-12 | étage 1 = **vertex shader** (exports `pos0`, `param0+`) |
| 13-16 | étage 2 = étage de sommets quand l'étage 1 est vide (vu avec un pixel shader de 19 Ko) |

Les étages sont reconnus à leurs exports (`exp mrt0` pour le pixel shader, `exp pos0`/`param` pour les sommets).

## Métadonnées d'étage
Même format que la `kind 7` FSPX de `fspx_program` (Marvisak main, `native.rs`), version 2 au lieu de 0x403 :
`u32 2, u32 taille-0xc, u32 mots de code, u8 0x40, u8 premier dword user-data, u8 nombre d'enregistrements 0x41, u8 0`,
puis des enregistrements de 32 octets (0x41 ressource lue, 0x42 déclaration de vue, 0x43 hash).

## Ce qui manque pour les créer
1. Le format FSPX/ACCC dans notre traducteur (porter `fspx_program`).
2. Le paquet de registres : SPI_PS_INPUT_*, sorties du vertex shader, format des cibles couleur (le code exporte déjà au format de la cible, sans les correctifs d'export du FSSM).
3. Des signatures d'entrée/sortie fabriquées (le jeu ne passe pas de DXIL).

## Eau blanche (enquête du 2026-11-26)
Outils : `VICTORY_CAPTURE_AT=<secondes>` (capture à l'heure, les cinématiques suivent l'horloge),
`VICTORY_DUMP_TARGETS=1`, valeurs brutes des petites cibles dans le journal (build `diagnostics`).
- L'éclairage HDR est juste (coucher de soleil de la cour de prison correct avant post-traitement) ;
  l'exposition vaut 2^-12,9 et le ciel, les arbres et les personnages sont bien exposés.
- Seule l'eau est blanche, dès la cible HDR éclairée : on voit le sable plein soleil sous l'eau, sans absorption.
- Le calcul d'eau `cad2edef5ecea3ba` écrit la couleur turquoise (R11G11B10 1280x720), trouée de bandes noires,
  et **aucune passe ne la relit dans la même image**. La passe qui la compose manque ou lit une autre image.
- Seuls dessins sautés : les étapes du constructeur de ray tracing (`VICTORY_BVH_BUILDER` éteint).
