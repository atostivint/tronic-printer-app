# Inbox — Tronic Printer

File de travail réelle, remise à plat le 2026-11-08 à 15:35. Une ligne par item ; `done` = mergé sur
`main` et vérifié, `ready` = prêt à partir, `blocked` = attend une info de l'utilisateur.
Les décisions qui expliquent ces items sont dans `decisions.md`.

## Bilan de la nuit du 2026-11-08 (release v0.1.0 publiée)
`main` = `36eccd4`, **deux releases publiées** : `v0.1.0` puis **`v1.1.0`** (installateur + zip
portable, construits et testés par le workflow). Copie QA : `v1.1.0 #89 36eccd4`.

`v1.1.0` ajoute par rapport à `v0.1.0` : presets par thème, espace après l'impression (moteur + MCP,
pas encore de contrôle dans la fenêtre), et le CERFA devenu **demande d'autorisation d'utilisation
d'une intelligence artificielle**. Les échantillons du faux terminal et du thème Terminal prennent
désormais la version depuis la crate au lieu de l'écrire à la main.

Chantier enregistré pour la suite (page Hindsight `kp-842b22d4d31f4545b1b982db3da05b84`) : **sortir
les textes des thèmes du code** — un JSON par thème pour tous les libellés, les littéraux actuels en
repli, sur le modèle de `render/theme/custom.rs`. Objectif : rendre tous les textes éditables sans
recompiler, puis n'écrire les nouveaux thèmes qu'en données.


Mergé dans la soirée : fantôme déplaçable, bannière « file dialog », densité du tiroir, réglages sous
le papier, `qa.ps1` (version calculée avant le build), TK explique les onglets, ombre de TK, balancement
au glisser, Print/pastille sur le boîtier + jauge de batterie + barres de réception, frappe gelée,
rendu du thème pendant la frappe, rythme 60/120 fps, CERFA, presets par thème, espace après
l'impression, README, CI réparée (et rendue rare : plus de run à chaque push).

Encore ouvert, avec le code déjà écrit mais **non mergé** :
- ~~`tkerror` (TK annonce l'erreur)~~ : **mergé** `5b56382` — TK dit l'erreur dans sa bulle (une phrase par
  raison dans `voice::error_talk`), verre et papier tièdes au lieu d'un flash rouge, un clic efface
  l'erreur. Preuve : 290 tests verts + snapshot `error` lu (« The printer ghosted me. Again. »).
  Reste `qa.ps1` + `QA.md`.
- `collections` (navigateur de thèmes/images) : abandonné, à refaire après le découpage.
- ~~`ci` : `a_long_train_field_keeps_the_layout`~~ : **corrigé** `5df97ac`. Les deux assertions ne
  dépendent plus de la police installée : le test de `travel` épingle tous les autres champs pour ne
  comparer que les valeurs longues (même face, même processus), et celui de `banner` sonde d'abord le
  résolveur avant de supposer deux visages — sinon il s'arrête en disant pourquoi. Vérifié avec DejaVu,
  avec une seule police, et sans aucune police. Il reste à voir la CI Linux pour de vrai.
- `space` : le réglage existe et s'applique, mais **aucun contrôle dans la fenêtre** (à poser dans la bande de réglages).


## En cours (un seul par fichier partagé)
| id | item | fichier | état |
|---|---|---|---|
| TP-56b | Fermer les trous du garde-fou étiquette (`can_print`, `print_once`, `flush_terminal`, `run_job`) | `app.rs`, `jobs.rs` | **fait/mergé** (A1c, feat/fx-revamp non commité) : `print`/`drain_gallery_queue`/`flush_terminal` via `on_a_label_printer`, `tick_terminal` feed-only gardé, `send_job` en défense, tests label ajoutés. Build+tests verts. |
| TP-56c | Dire à quelle imprimante on parle (nom + type dans l'infobulle, pastille « label », Print éteint, scène `label-online`) | `ui/mod.rs`, `voice.rs`, `main.rs` | **fait/mergé** (A2, feat/fx-revamp non commité) : `label_online` unifié, help label `device_line`+refus, sélecteur `discovered`→`prefer`/`prefer_auto`, scène `label-online` réaliste. Build+tests verts. Écran/snapshot/QA non vérifiés ; `discovered` non alimenté par BLE (picker inerte en réel). |
| TP-57a | Fenêtre de recadrage 14:30 (`picture::crop_window` / `cropped`), pure et testée, pour le futur mode Étiquettes | `render/picture.rs`, `render/mod.rs`, `app.rs` | **fait/mergé** (A3, feat/fx-revamp non commité) : API crop/export/tests + câblage `compose_image` (`prepare_with`, crop dans clé cache) présents, `render::picture` et suite verts. Câblage UI `pic_opts.crop` (éditeur label/TP-57) non fait. |
| TP-34 | Une feuille Plain vide doit exister et être cliquable au démarrage | `app.rs`, `theme.rs` | in-flight (worker `blank`) |
| TP-32 | Bulle BD dimensionnée au texte + fantôme déplaçable à la souris | `ui.rs`, `app.rs` | branche `bubble`, 223 tests verts, merge en cours |
| BC250 | Analyses read-only relancées (audit/unwrap/BLE/IPC) | — | in-flight |

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
| TP-55 | Découper `src/ui/mod.rs` et `src/app.rs` pour travailler à plusieurs | `src/ui/*`, `src/app/*` | queued | Demande utilisateur : le programme doit être découpé pour que plusieurs agents travaillent en parallèle. Cible : `ui/{top_bar,stage,drawer,settings_strip}.rs` + `ui/mod.rs` réduit à l'assemblage et aux helpers partagés ; `app/` en blocs `impl App` séparés (ghost, print, gallery, themes). Socles A1c/A2 mergés dans main (non commité) sur `ui/mod.rs`/`app.rs` ; découpage toujours à faire, ordre inchangé (après TP-52, TP-53, TP-54). |
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
| TP-56 | Deuxième imprimante : découverte par type + choix de l'appareil | `ble.rs`, `protocol.rs`, `app.rs`, `voice.rs` | in-flight | Mergé : `a2f9790` (`Kind::{Pocket,Label}`, tous les appareils Tronic vus, tri par signal) puis `f982552` (`preferred`/`preferred_kind`, le worker se connecte à l'appareil choisi sans repli, `App::print` refuse une étiquette et TK le dit). **Trou restant réduit par A1c** (feat/fx-revamp non commité) : `flush_terminal` passe maintenant par `on_a_label_printer` ; restent `print_once` (CLI/MCP) et `can_print` (bouton Print non éteint). À fermer avant que le mode Étiquettes existe. |
| TP-57 | Mode Étiquettes : 112 × 240 points, coins arrondis, une étiquette par impression | `ui/*`, `render/*`, `app.rs` | queued | Décisions utilisateur : une impression = **une** étiquette (pas de série) ; la surface est un rectangle **à coins arrondis** donc le contenu est masqué dans une zone utile arrondie ; une image se recadre dans une fenêtre au format 14:30, déplaçable derrière la fenêtre dans l'aperçu, tramée en direct. Le boîtier a son propre dessin (galet, bouton rond à anneau LED, TRONIC vertical, tiroir au dos, barre crantée en haut). Socle A3 (crop 14:30) mergé dans main (non commité) ; câblage UI `pic_opts.crop` non fait ; mode complet (B/C/D) pas démarré. Dépend de TP-58. |
| TP-58 | Le profil papier remplace les 384 points codés en dur | `protocol.rs`, `render/*`, `app.rs` | queued | 13 références dans 8 fichiers. Objectif : une largeur par appareil (384 pour la 5890, 112 pour les étiquettes), un `Bitmap` qui porte sa largeur, et l'étiqueteuse en continu vs étiquette + entre-deux. Socles A1c (print-path) et A3 (render) mergés dans main (non commité) ; profil papier (largeur par appareil, Bitmap portant sa largeur) pas encore implémenté. Nécessaire avant TP-57. |
| TP-59 | Étiqueteuse : protocole de la DP-L13 | `protocol.rs`, sonde `examples/labelprobe.rs` (worktree `bleprobe`) | **protocole d'impression établi** ; calage encore ouvert ; socles A1c/A2/A3 mergés (non commité), B/C/D pas démarrés | Vérifié sur l'appareil, à l'impression : activation `10 FF F1 03` + 12 zéros, bandes de 24 lignes en `1D 76 30 00 0E 00 18 00` + 336 octets (112 points de large), un crédit par paquet de 180 B, stop `10 FF F1 45`. Mesure du motif `--ladder` (photo + règle 1:100, 14,2 px/mm) : blocs de 3,0 mm, blancs de 3,0 mm, pas de 6,0 mm → **8 points/mm confirmé**, contenu sur **une** étiquette, 1,3 mm de marge en haut, 3 mm en bas. Un seul défaut observé : l'entraînement a glissé de ~1,2 mm (10 lignes) sur le 5e bloc. **Calage : négatif, testé.** `1D 0C` et `1F 11 51`+`50` n'alignent pas ; `10 0C`+`1B 4A 28` en fin de job ramène le départ de 13,7 à 5,9 mm du bord ; `1F 11 11 nn` (recul) est accepté mais ne ferme pas l'écart. **`10 FF 40` ne voit pas l'interstice** : balayage `activated_gapscan --step 8 --count 60` (60 mm) → `00` sur les 60 pas, aucun basculement. Conclusion : pas de détection de bord possible par le capteur. Décision produit à prendre : calage manuel (bouton « avancer à l'étiquette ») + fin de job `10 0C`/`1B 4A 28` en best effort, ou mode Étiquettes assumant un offset. Voir `run\research_dpl13.md`. |

## Fait et mergé (ne pas refaire)
TP-08, TP-11, TP-13, TP-14, TP-15, TP-17, TP-18, TP-19, TP-22, TP-27, et les lots
`ticket-formulaire` (31b50d5), `scène d'abord` (b145feb), `effets par famille` (ac55615),
`fantôme volant` (7f32f20), `version` (8e3c7f6 + 61928c0), `build verrouillé` (8bd6b06),
`veilleur` (0a6c8d7), `boarding` (19e14da), `papier qui monte` (fc48547).

## Avant de commencer un item
1. `git worktree list` : une branche porte peut-être déjà le travail.
2. Un seul agent par fichier partagé (`ui.rs`, `app.rs`, `main.rs`, `theme.rs`).
3. Brief court, code d'abord, preuve = `git diff` + commandes, jamais un résumé.
