VIctory S {VERSION} : démarrage rapide
======================================

VIctory S lance VOTRE copie d'un jeu Xbox Series sur PC, avec le profil Series S réglé pour les
petites configurations. Aucun fichier du jeu n'est fourni ici : il faut votre propre copie.

0. Il faut la copie COMPLÈTE du jeu : game_scarlett_master.exe avec toutes ses archives
   à côté (shaders.rpf, levels.rpf et les autres .rpf, ~130 Go pour la v1.0.2.1), dans son
   propre dossier (par exemple C:\Jeux\GTA6), pas en vrac dans Téléchargements.
1. Décompressez ce dossier où vous voulez (pas dans le dossier du jeu).
2. Double-cliquez sur « VIctory S.exe ».
3. Onglet Préparation, bouton « Tout préparer » : le lanceur trouve le jeu
   (game_scarlett_master.exe de votre copie), traduit les shaders pour votre carte, puis
   construit d'avance les pipelines de la liste partagée (25 à 40 minutes, une seule fois).
   VIctory lui-même est déjà inclus : rien à compiler.
4. Onglet Jouer, bouton JOUER : la première partie est déjà fluide jusqu'après la mission
   du bateau. Plus loin, Préparation, étape 4, « Construire » ajoute les pipelines notées.

Le profil (fils du processeur, mémoire de la carte, mémoire vive) est choisi tout seul d'après
votre PC : voyez « Profil détecté » dans l'onglet Réglages.

Conseils
- Carte NVIDIA : Panneau de configuration NVIDIA > Gérer les paramètres 3D > Taille du cache
  des shaders > Illimitée.
- 16 Go de mémoire vive : fermez le navigateur, Discord et les autres programmes avant de jouer.
- Portable : branchez le chargeur.
- Processeur avec AVX2 nécessaire (Intel 4e génération / AMD Ryzen ou plus récents) et Windows 10
  ou 11 64 bits avec WebView2 (déjà présent sur un Windows à jour).

Donner mon avis : onglet Aide ou bas de la fenêtre, « Donner mon avis ». Le formulaire GitHub
s'ouvre déjà rempli avec votre configuration ; vous le relisez et l'envoyez vous-même.

Contenu
- VIctory S.exe              le lanceur
- engine\victory.exe         le moteur VIctory (profil Series S)
- series-s\reglages.json     les réglages mesurés, les profils par matériel
- series-s\pipelines\series-s-rtx.txt
                             la liste de pipelines partagée (aucun code du jeu)
- scripts\rt_off_hashes.txt  les passes du ray tracing sautées
- LICENSE                    GPL-3.0-or-later

Vos réglages, journaux et captures : %LOCALAPPDATA%\Sagouindupm
Sources, aide et mesures : https://github.com/sagouindupm/Mandy
Projet sans lien avec l'éditeur ou le développeur du jeu.
