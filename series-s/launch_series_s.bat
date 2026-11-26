@echo off
rem Sagouindupm | VIctory : profil Series S pour carte de 8 Go.
rem Renseignez les trois chemins ci-dessous, puis lancez ce fichier.
setlocal
set "GAME_EXE=C:\chemin\vers\game_scarlett_master.exe"
set "VICTORY_EXE=%~dp0..\target\release\victory.exe"
set "VICTORY_CONFIG=%~dp0..\victory.toml"
rem Optionnel : DLSS (Streamline) si vous avez ses DLL
rem set "VICTORY_DLSS_DIR=C:\chemin\vers\streamline\bin"
rem Avec DLSS, mode Performance : le jeu calcule en 960x540 et la carte NVIDIA remet en 1080p
rem (RTX 3050 : 292 ms par image au lieu de 351, image plus nette). Change en memoire l'echelle
rem de rendu du jeu (2/3 vers 1/2) apres avoir verifie son code ; le fichier du jeu n'est pas touche.
rem set "VICTORY_DLSS_MODE=performance"

rem Memoire de la carte : place pour les textures sur 8 Go
set "VICTORY_CARD_ROOM_FOR_TARGETS_MB=1536"
set "VICTORY_CARD_BUFFERS_MIN_MB=1"
set "VICTORY_CARD_BUFFERS_MB=512"
set "VICTORY_TEXTURE_DROP_MIPS=1"
set "VICTORY_TEXTURE_SPILL=1"
set "VICTORY_HEAP_USAGE=1"
set "VICTORY_LEVELS_ON_DEMAND=1"
rem Chargement des textures
set "VICTORY_TEXTURE_STAGE_MS=180"
set "VICTORY_TEXTURE_CATCHUP=1"
rem Correctifs de rendu
set "VICTORY_HEAP_SYNC_TABLES=1"
set "VICTORY_SHADER_LOOP_LIMIT=256"
set "VICTORY_AMBIENT=0.6"
rem Soumission et copies
set "VICTORY_ASYNC_SUBMIT=1"
set "VICTORY_QUEUE2=1"
set "VICTORY_COPY_PLAIN=1"
set "VICTORY_FAST_CORES=1"
rem L'eau : les bateaux et le joueur flottent
set "VICTORY_TEXTURE_BACK=1"
rem Memoire non surveillee recopiee une image sur deux (-17 %% par image)
set "VICTORY_UNWATCHED_FRAMES=2"
rem Eau turquoise : les deux calculs de l eau sans la forme "sans sauts profonds" (bandes vides sur NVIDIA)
set "VICTORY_WAVE_DEEP_SKIPS_NOT=cad2edef5ecea3ba,731a39171b568298"

"%VICTORY_EXE%" run "%GAME_EXE%" --console series-s --execute --unimplemented zero
pause
