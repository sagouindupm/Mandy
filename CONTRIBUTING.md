# Contribuer à Sagouindupm | VIctory

Le but du projet est unique : **faire tourner le jeu au mieux sur une petite configuration, profil Series S**
(carte de 8 Go, processeur milieu de gamme). Une contribution est la bienvenue si elle va dans ce sens.

## Règles

1. **Pas de poussée directe sur `main`.** Ouvrez une pull request depuis une branche ou un fork. Elle est
   relue et fusionnée par le mainteneur uniquement.
2. **Une pull request = un sujet.** Un correctif ou une optimisation par PR, avec un titre qui dit ce que
   ça change.
3. **Des mesures.** Pour une optimisation : fps ou ms par image avant et après, sur quelle carte, à quel
   endroit du jeu. Pour un correctif de rendu : une capture avant et après.
4. **Rien du jeu.** Ni fichiers, ni dumps, ni journaux bruts, ni captures de code du jeu.
5. **Licence.** Tout le code est GPL-3.0-or-later. Aucun code GPL-2.0-only (XCL, Mandy d'origine) ni sous
   licence incompatible. Pas de patch qui modifie l'exécutable du jeu.
6. **Crédits.** Si vous reprenez le travail de quelqu'un, gardez son commit et son nom
   (`git cherry-pick -x`) ; ne réattribuez jamais le travail d'un autre.
7. **Aide d'une IA** : autorisée, mais à signaler dans la description de la PR.

## Avant d'ouvrir la PR

- `cargo build --release` passe.
- `cargo test -p victory-gpu` et les tests du crate touché passent.
- Le jeu démarre en `--console series-s` et atteint au moins le monde.
