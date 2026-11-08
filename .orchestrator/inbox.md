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
| TP-37 | Un thème `codes` : plusieurs QR/barcodes dans le texte | render-themes | done | Merged `bd77cdc` : thème `codes`, lignes `qr:`, `code128:`, `code39:`, `ean13:` rendues en symboles avec légende, le reste en texte ; 44 thèmes. |
| TP-38 | Le fantôme ne se déplace pas malgré la main | `ui/ghost.rs`, `ui/mod.rs` | done | Mergé `63d5e33`. Deux causes empilées : `Drag::x/y` sont locaux à l'élément (le fantôme suivait le pointeur, delta nul) → on prend `Drag::dx/dy` ; puis `memo::layout("stage")` répond `None` hors d'un build de vue (`rune/src/memo.rs:438,475`) et le `if let` avalait chaque Move → la taille de scène arrive maintenant en paramètre depuis `paper()`. Tests : `dragged_to` + `a_press_a_move_and_a_release_carry_the_ghost_in_the_room_it_was_given`. Vérification souris par l'utilisateur en attente. |
| TP-39 | « a file dialog is already open » après avoir rouvert le sélecteur | `ui/mod.rs` | done | Mergé `e95ac48` : le refus vient de Rune (un seul dialogue), notre callback l'affichait comme une erreur ; `is_busy_dialog` n'affiche plus rien pour ce message, les vraies erreurs restent. Test `a_second_dialog_request_is_not_an_error_worth_a_banner`. |
| TP-40 | Densité et typographie du tiroir | `ui/mod.rs` | done | Mergé `aa68f8e` : légendes en taille corps, `choice_fit` (plus de débordement des cartes), « Medium » entier, interrupteurs avec libellé à gauche, noms de vignettes complets sur deux lignes, air entre sections + pied de scroll. |
| TP-41 | Sortir les réglages du tiroir et les poser sous le papier | `ui/mod.rs` | done | Mergé `a0f6a95` : le tiroir ne garde que les galeries ; Look / luminosité / contraste / espace / rotation / miroir / reset / inversion sont sur une bande sous la scène, mêmes widgets et mêmes handlers. |
| TP-42 | `qa.ps1` annonçait un commit que le binaire ne contenait pas | `tools/qa.ps1` | done | Mergé `1d7f1bd` + `74d91e2`. Le label est calculé **une fois, avant** le build, et réutilisé après ; `-NoBuild` n'invente plus de commit (heure du fichier + renvoi à la fenêtre) ; après `Stop-Process`, on attend la fin du processus et on réessaie la copie 3 fois (sinon l'app restait fermée). |
| TP-43 | TK : le rendre animé quand on le déplace | `ui/ghost.rs`, `app.rs`, `main.rs` | in-flight | Demande utilisateur : en glissant le petit bonhomme, il doit y avoir du mouvement (balancement selon le geste, retour souple au lâcher, yeux qui suivent). Pure fonction de swing + décroissance amortie, `tick` dédié pour redessiner le temps de l'amorti. |
| TP-44 | Bouton Print **à côté** de l'imprimante (elle rétrécit sur un long ticket) | `ui/mod.rs` | queued | Demande utilisateur : quand le ticket est long l'imprimante rétrécit et le bouton sur le boîtier devient minuscule. Le sortir du boîtier, posé sur le côté de la scène, à taille fixe. |
| TP-45 | Voir le **rendu** pendant la frappe, et arrêter le saut | `ui/mod.rs`, `ui/write.rs`, `app.rs` | queued | Bug + demande utilisateur : chaque touche fait sauter la page (re-fit + régions recalculées) et la boîte d'édition affiche une écriture d'interface au lieu du rendu du thème. Geler la géométrie pendant l'édition (re-fit au `stop_editing`), ancrer la boîte, et montrer l'encre du thème. |
| TP-46 | TK explique chaque onglet quand on y va | `voice.rs`, `app.rs`, `ui/mod.rs` | in-flight | Demande utilisateur : arriver sur Terminal ou Images doit faire dire à TK quoi faire là. `voice::mode_hint`, `App::switch_mode`, 12 s de conseil puis les vannes habituelles. |
| TP-47 | Une petite ombre sous TK | `ui/ghost.rs` | in-flight | Demande utilisateur (le mascotte s'appelle T-Kay / TK) : la bulle a une ombre, le visage en papier non. Ombre douce et discrète, sans rectangle visible derrière les bords déchirés. |
| TP-48 | README à jour | `README.md` | done | Mergé `f6e966a` : 44→45 thèmes, tiroir/galeries, bande de réglages sous le papier, TK, thème `codes`, `tools/*`, licence MIT, scripts `make_*` disparus. |
| TP-49 | Faire tourner les animations à 60/120 fps | `app.rs` | in-flight (preuve) | Cause : `tick_interval` ne redessinait qu'à 30 fps, et **12,5 fps** pour le simple flottement du fantôme. Paliers `frame_step` : 8 ms quand une main tient quelque chose, 16 ms pour le reste qui bouge, rien au repos. |
| TP-50 | CI rouge sur les trois systèmes | `.github/workflows/ci.yml`, `theme.rs` | in-flight | Trois bugs réels : `libxkbcommon` manquant sous Linux, un test de typographie qui dépend d'une police absente du runner Windows, panics en cascade sous macOS. Plus : la CI ne tourne plus à chaque push (PR + `workflow_dispatch` seulement, `concurrency`). |
| TP-51 | Release GitHub pour cette version | `gh`, tag `v0.1.0` | queued | Après CI verte : tag `v0.1.0`, `release.yml` fabrique l'installateur Inno + le zip portable et publie la release. `release.yml` et `installer/tronic-printer.iss` existent déjà et n'ont jamais tourné. |
| TP-52 | Un vrai navigateur de thèmes et d'images | `ui/mod.rs`, `app.rs`, `gallery.rs` | in-flight | Demande utilisateur : recherche, filtres par famille, tri (favoris / récents / A→Z), grandes vignettes, panneau au-dessus de la scène, états vides. |
| TP-53 | Print sur le boîtier, jauge de batterie, qualité de réception | `ui/mod.rs` | in-flight | Régression signalée : Print et la pastille sont toujours au pied de la scène. Ils retournent **sur le boîtier** par défaut, passent **à côté** quand l'imprimante devient petite, et glissent entre les deux (même clé de layout). Batterie dessinée (jauge colorée), 4 barres de signal depuis `s.rssi`. |
| TP-54 | Espace avant **et** après l'impression, pour tous les types | `settings.rs`, `app.rs`, `jobs.rs`, `mcp.rs` | in-flight | `space_before_mm` n'existe que pour les images ; on généralise : réglage persistant, rangées blanches en bas du bitmap composé (aperçu = impression), paramètre de job `space_after_mm` 0-50. Le contrôle dans la bande de réglages viendra après (dépend de `ui/mod.rs`). |
| TP-55 | Découper `src/ui/mod.rs` et `src/app.rs` pour travailler à plusieurs | `src/ui/*`, `src/app/*` | queued | Demande utilisateur : le programme doit être découpé pour que plusieurs agents travaillent en parallèle. Cible : `ui/{top_bar,stage,drawer,settings_strip}.rs` + `ui/mod.rs` réduit à l'assemblage et aux helpers partagés ; `app/` en blocs `impl App` séparés (ghost, print, gallery, themes). À faire **après** les trois branches qui tiennent `ui/mod.rs`/`app.rs` (TP-52, TP-53, TP-54). |
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
