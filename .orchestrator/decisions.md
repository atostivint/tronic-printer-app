# Décisions de l'utilisateur — Tronic Printer

Une ligne par arbitrage. C'est la mémoire qui survit aux compactions, aux redémarrages et aux agents
qui relisent le dépôt. Quand une décision change, on ne l'efface pas : on la barre et on ajoute la
nouvelle, avec la date. Format : `date · décision · pourquoi · où c'est appliqué`.

## 2026-11-07 — publication et premiers correctifs
- Publier le projet sur un dépôt GitHub **public** `atostivint/tronic-printer-app`, branche par défaut `main`
  (le nom du dépôt garde celui du dossier). → `README.md`, `LICENSE`, `origin`.
- `opencode.json` **non suivi** (ids d'agent et de modèle propres à la machine) et dans `.gitignore`.
- Licence **MIT**, cohérente avec `Cargo.toml`.
- Les branches sont poussées et deux PR ouvertes (#1 XBackBone, #2 Linux/macOS) ; #2 rebasée puis mergée.
- La **barre de titre native** doit suivre la palette (elle restait en couleurs de Night).
- Le **libellé du bouton Print** doit être centré dans la capsule (il partait au bord).
- Le **fantôme** passe à côté de l'imprimante quand la fenêtre est assez large, au-dessus sinon.

## 2026-11-08 — refonte de l'interface
- **Scène d'abord** : plus de colonne ; une barre haute unique, le papier au centre, un **tiroir à gauche
  fermé par défaut** qui contient les galeries. (Trois candidats rendus, celui-ci retenu.)
- Le **ticket est le formulaire** : on écrit en cliquant sur le papier ; les valeurs sont rangées **par clé
  de champ** ; le formulaire dupliqué de la colonne est supprimé. Shuffle / Blank form / Closing line restent.
- **Thèmes et réglages séparés** : le tiroir ne garde que les thèmes et les galeries, les réglages vont
  dans un panneau distinct. *(Remplacé plus bas par « tout autour de la page ».)*
- **Options autour de la page** : chaque partie modifiable porte sa poignée ; le choix s'ouvre près d'elle.
  Plus de panneau latéral de réglages.
- **Typographie WYSIWYG** par partie (police, taille, effet imprimable : contour, ombre, double trait,
  inversé), sur **tous les thèmes**, par familles successives, avec une poignée « Aa ».
- Le **bouton Print devient un bouton d'appareil sur le boîtier**, à côté de la LED.
- La **pastille de connexion/batterie** descend de la barre haute vers l'imprimante.
- Le **fantôme vole** autour de la feuille (dérive ~26 s, au clic) et parle dans une **bulle BD** crème.
- La **tagline sous TRONIC monte du bas** au lieu de se taper lettre à lettre.
- **Effets** : l'**aurora** revient partout, simplement teintée par famille de thèmes ; les trames qui
  ressemblaient à un scanner sont supprimées.
- **Tampon PAID/UNPAID** : activable/désactivable, et il ne doit **jamais** masquer un montant.
- Un **champ vidé** ne fait pas disparaître le ticket : le refus « nothing to print » n'arrive qu'à l'impression.
- Les **billets** exposent leurs attributs générés (opérateur, numéro, prix, classe) comme champs éditables,
  avec la valeur générée en défaut ; `MAX_FIELDS` passe à 8. Idem pour boarding/flight (en cours).
- Un **numéro de version** est visible en bas à droite (`v0.1.0 · #N · hash`, `*` si arbre sale).

## 2026-11-08 — machine et outillage
- **Un seul build cargo à la fois** sur la machine : `tools/build.ps1` (mutex), `jobs = 8`, priorité
  `BelowNormal`, refus de démarrer sous 1,5 Go de RAM libre. Jamais deux cargo, même avec des `target` séparés.
  Mesure : six builds parallèles sur 32 Go laissaient 2,3 Go libres et figeaient le bureau — c'est le
  commit charge, pas le CPU.
- **Veilleur de commits** : `tools/watch.ps1` reconstruit et **relance** la copie QA quand `main` bouge.
- ~~**Modèles locaux (BC250)** : prouvés fonctionnels côté passerelle, mais **inutilisables comme agents**
  (Qwen 7B a inventé un appel d'outil ; Mellum2 a inventé un test et sa sortie verte). Aucun travail de code
  ne leur est confié ; tout résultat qu'ils rendraient doit être vérifié par un `git diff`.~~
  *(Corrigé le 2026-11-08 après mesures — voir la section « modèles locaux » en fin de fichier.)*
- **Impression physique** : jamais sans demande explicite de l'utilisateur.

## 2026-11-07 — produits et fonctions
- **XBackBone** : lecture seule — galerie d'URLs de partage, clic → téléchargement → impression.
  Pas d'upload, pas de navigation dans le serveur (lister les uploads avec un token est impossible).
- **Thème greentext** (fil 4chan) : en-tête Anonyme, barres de citation, `>>` plus pâle, pied monospace.
- **Sens de l'impression** : le **bas** du ticket apparaît en premier près de la fente (contrairement au
  comportement actuel, qui montre le haut d'abord). À corriger.

## 2026-11-08 (soir) — modèles locaux : ce qui marche vraiment
Mesuré sur `bc250-gateway` (llama.cpp `b12020`, `--jinja`), deux tests par modèle : capacité d'appel
d'outil, puis deux tâches d'édition bornées sur un dépôt jetable, vérifiées par `git diff` plus une
assertion **hors du modèle**. Détail : `atostivint/bc250-gateway`, `docs/model-tool-calling.md`.
- **Mellum2 a bien le tool-calling natif** ; son échec venait d'une copie infidèle du fichier
  (`Reluire` pour `Relire`). Il se rattrape seul **si on lui renvoie l'erreur de l'outil** dans le tour
  suivant. Erreur avalée = boucle puis résumé d'un travail non fait.
- **Qwen3-14B et Qwen3-8B fonctionnent** à condition d'envoyer `chat_template_kwargs:
  {"enable_thinking": false}`. Sans ce paramètre, le raisonnement mange le budget de tokens : zéro appel
  d'outil, `finish_reason: length`. Ce n'était pas une incompétence du modèle.
- **`gpt-oss-20b-MXFP4` reste le worker par défaut** : le seul propre sur les deux tâches à chaque essai.
  `Qwen2.5-Coder-7B`, lui, n'a aucun tool-calling structuré : utilisable seulement via un protocole texte
  (bloc SEARCH/REPLACE appliqué et vérifié par l'orchestrateur), et jamais pour lire ou rapporter un état.
- **La limite de contexte n'est pas à 32k** : `/v1/models` annonce 131072 pour gpt-oss et Mellum2. Une
  limite plus basse vient du client, pas du nœud.
- Règle inchangée et non négociable : **le modèle local ne lance ni build ni test**. Il rend un diff,
  l'orchestrateur vérifie.
