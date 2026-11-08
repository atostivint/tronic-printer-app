# Inbox — Tronic Printer

File de travail réelle, remise à plat le 2026-11-08 à 15:35. Une ligne par item ; `done` = mergé sur
`main` et vérifié, `ready` = prêt à partir, `blocked` = attend une info de l'utilisateur.
Les décisions qui expliquent ces items sont dans `decisions.md`.

## En cours (un seul par fichier partagé)
| id | item | fichier | état |
|---|---|---|---|
| TP-34 | Une feuille Plain vide doit exister et être cliquable au démarrage | `app.rs`, `theme.rs` | in-flight (worker `blank`) |
| TP-32 | Bulle BD dimensionnée au texte + fantôme déplaçable à la souris | `ui.rs`, `app.rs` | branche `bubble`, 223 tests verts, merge en cours |

## Prêt, dans l'ordre où je le prends
| id | item | fichier | notes |
|---|---|---|---|
| TP-30 | La boîte d'édition du montant `fine` se pose en haut à gauche du papier | `ui.rs` + région | la zone du champ est fausse, pas le dessin |
| TP-28 | Une valeur générée par défaut ne doit pas s'afficher comme un hint ; l'encadré d'édition doit se poser **autour** de la zone, pas sur les glyphes | `theme.rs`, `ui.rs` | demandé deux fois |
| TP-33 | Barre haute allégée : la pastille de connexion descend sur la scène, Print passe sur le boîtier, les 7 pastilles de palette entrent dans Options | `ui.rs` | inclut TP-26 |
| TP-35 | Nom du thème en haut à gauche + bouton « thème au hasard » près de l'imprimante | `ui.rs` | dépend de TP-33 |
| TP-36 | Aperçu sur papier couleur : crème (défaut), bleu, rose, jaune | `ui.rs`, `settings.rs` | l'utilisateur a les trois rouleaux pastel ; c'est un réglage d'**affichage** : on teinte la feuille dans l'aperçu, l'encre reste noire, aucun changement de rendu ni d'impression. Garder les teintes claires (le contraste du texte doit tenir). |
| TP-25 | La tagline sous TRONIC doit **monter du bas** (révélation verticale), pas se taper lettre à lettre | `ui.rs`, `app.rs` | jamais fait, deux échecs de workers |
| TP-29 | Typographie WYSIWYG par partie (police, taille, effet imprimable), **tous les thèmes**, par familles, poignées autour de la page | `theme.rs`, `theme/*`, `ui.rs` | le plus gros chantier |
| TP-21 | Boîtes d'écriture **tournées** pour `flight`, `train-old` (corps) et `banner` | `travel.rs`, `ui.rs` | décision utilisateur |
| TP-16 | États explicites restants : `TerminalSession`, `Link::Online { health }`, `McpEndpoint` | `term.rs`, `app.rs`, `mcp*.rs` | batches 3 à 5 de l'audit |
| TP-01 | XBackBone : galerie d'URLs de partage (lecture seule) | `xbackbone.rs`, galerie | PR #1 ouverte, jamais mergée |
| TP-02 | Linux/macOS : la CI a tourné, il manque un **vrai run** sur les deux systèmes | — | rien n'a jamais été lancé là-bas |
| TP-03 | Icône dans la zone de notification : statut de l'imprimante au survol, clic = rouvrir la fenêtre | plateforme (Windows) | ready | Décision utilisateur : pas de mini-fenêtre de dépôt pour l'instant. Icône de notification (Shell_NotifyIcon, le crate `windows` est déjà là pour `chrome.rs`) sur un thread dédié avec fenêtre cachée : infobulle = connecté/batterie, clic gauche = montrer et mettre au premier plan la fenêtre existante (`find_window`), clic droit = menu Ouvrir / Quitter. Si le X peut cacher au lieu de quitter sans casser Rune, on le fait ; sinon on le dit et on garde le X actuel. Windows uniquement. |
| TP-05 | Le petit espace résiduel entre deux lots du terminal live | `app.rs`, `term.rs` | branche `orch-live-gap`, à reprendre |
| TP-06 | Thèmes utilisateur : zones d'écriture déclarées (`"region": "champ"`) | `theme/custom.rs` | branche `orch-custom-regions` |
| TP-07 | Textures de ticket (trame, tache de café, tampon, perforation, coin corné) | `theme/*`, `fx.rs` | branche `orch-ticket-effects` |
| TP-10 | Docs : plus aucune mention des anciens scripts `tools/make_*` | `README.md`, `themes/README.md` | à vérifier, rien trouvé pour l'instant |
| TP-09 | Trouvailles des relecteurs (bugs, UI/UX, perf) | — | à répartir dans les items ci-dessus |
| TP-04 | Format de la réponse papier `10 FF 40` | `protocol.rs` | **bloqué** : il faut l'imprimante avec et sans papier, et l'utilisateur |

## Fait et mergé (ne pas refaire)
TP-08, TP-11, TP-13, TP-14, TP-15, TP-17, TP-18, TP-19, TP-22, TP-27, et les lots
`ticket-formulaire` (31b50d5), `scène d'abord` (b145feb), `effets par famille` (ac55615),
`fantôme volant` (7f32f20), `version` (8e3c7f6 + 61928c0), `build verrouillé` (8bd6b06),
`veilleur` (0a6c8d7), `boarding` (19e14da), `papier qui monte` (fc48547).

## Avant de commencer un item
1. `git worktree list` : une branche porte peut-être déjà le travail.
2. Un seul agent par fichier partagé (`ui.rs`, `app.rs`, `main.rs`, `theme.rs`).
3. Brief court, code d'abord, preuve = `git diff` + commandes, jamais un résumé.
