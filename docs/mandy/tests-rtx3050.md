# Tests sur RTX 3050 (8 Go), 32 Go de RAM, Windows 11

Tests du 24/11/2026. Titre en version 1.0.2.1.

## Résumé

| | VIctory `ffe310e` | xcl `47efa2e` |
|---|---|---|
| À l'écran | générique d'intro sur fond noir | gris, sans son ni sous-titres |
| En interne | intro lancée, ~2 600 dessins/image | « Game Running », mission d'intro, 65 personnages |
| Vitesse | 0,3 à 0,9 fps dans la scène 3D | 30 fps, puis effondrement au chargement du monde |
| Arrêt | carte retirée par Windows (`DEVICE_HUNG`) après 2 min 30 à 3 min 45 | le jeu se tue lui-même (`+0x23cf683`) après 5 à 6 min 30 |

## VIctory

- 141 shaders de calcul refusés : ils demandent des vagues de 64 voies, Nvidia plafonne à 32. Ticket upstream #817, correctif en PR #189 (en conflit).
- 352 à 446 shaders ne se traduisent pas ; 116 à 120 pipelines refusés.
- Arrêt : une soumission de ~11 s, puis `DXGI_ERROR_DEVICE_HUNG`. Tickets upstream #822 (même carte), #264, #548.
- La couche de débogage signale un tampon de sommets laissé en état UAV. Ce n'est pas la cause : la PR #334 qui le corrige ne supprime pas l'arrêt (testée upstream sur RTX 4080).
- Cause identifiée upstream : un programme de calcul du constructeur de BVH (ray tracing) du jeu.

## xcl

- Les programmes 64 voies tournent en deux moitiés de 32 : 0 pipeline en échec.
- `+0x23cf683` est la routine d'erreur fatale du jeu, appelée par son propre chien de garde quand un thread reste bloqué. Tickets upstream #846, #675, #534 (touche aussi AMD).
- `XCL_PSO_WAIT_MS=50` repousse l'arrêt de 5 min 07 à 6 min 37, sans le supprimer.
- Mémoire au moment de l'arrêt : 29,8 Go engagés, 14,5 Go de mémoire vidéo demandés pour une carte de 8 Go.
- `device_type = "lockhart"` (profil Series S) : pire, assertion du jeu à 27 s pendant l'initialisation.

## Pistes

1. xcl : réduire la mémoire vidéo demandée (cause probable du blocage sur 8 Go).
2. VIctory : intégrer les PR Nvidia en attente (#189, #294, #334) dans `victory/nvidia`.
3. Non essayé : le script `patch.py` proposé dans xcl #846, qui modifie l'exécutable du jeu. L'auteur de xcl indique qu'il ne fait que masquer le problème.
