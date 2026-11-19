# Sessions matérielles — jalon 3

Base : `9173468`. Implémentation : `637af2d` sur `dev/passive-inventory`. Pas de nouveau modèle matériel revendiqué.

## Audit des pilotes

Avant ce jalon, `effects::Target::open` retournait une closure d'écriture. La vérification existait, mais le moteur ne recevait ni ses résultats ni les limites de la connexion. Un clic accepté par GTK pouvait attendre la fin d'un ancien worker, puis ouvrir du matériel alors que l'inventaire avait déjà changé. Une vérification WDL pouvait aussi changer le mode du hub avant la première couleur.

| Backend | Preuve disponible | Décision |
|---|---|---|
| LED multicolore du noyau | Attributs `multi_index`, `max_brightness` et identité locale du répertoire ; pas de modèle de contrôleur ni de nombre de LED physiques | Premier pilote complet. Mémoriser ordre des canaux et échelle dans la cible. Les relire par le fd du répertoire ouvert, refuser tout changement puis garder les deux fds d'attributs. |
| Fractal WDL/LampArray | Identité du fd hidraw, instance HID, descripteur exact ; les attributs LampArray donnent le nombre de lampes et l'intervalle minimal | Second pilote complet, transport différent. Revérifier le descripteur *sur le fd*, lire les attributs, puis entrer en contrôle hôte seulement lors de la première opération autorisée. |
| Aura USB (zones fixe/ARGB) | DMI et identité du fd ; dimensions provenant de la fiche de carte, pas d'une mesure | Conserver les limites et le dispatch existants. Différer la commande Gen1 jusqu'à la première couleur. Ne pas annoncer 100 LED physiques vérifiées. |
| ENE GPU et DIMM | Famille GPU/contrôleur hôte, adresse autorisée et signature firmware. Le nombre de LED vient du contrôleur GPU ou de la fiche DIMM | Conserver les vérifications. Entrer en mode direct lors de la première couleur, sans commande de sauvegarde persistante. |
| Corsair DIMM | Contrôleur hôte, adresse autorisée, signature en lecture puis identifiants, version de protocole et CRC | Conserver les contrôles et les sélecteurs nécessaires à l'identification explicite. Jamais de scan de plages d'adresses. |

## Contrat utilisé par l'application

1. **Observer** : `discovery::scan` lit les métadonnées. Son résultat ne contient ni fd d'écriture ni session.
2. **Identifier** : une disposition compatible produit un `drivers::Target`. Les candidats RAM restent sélectionnés individuellement. PCI/DMI seuls n'autorisent pas les couleurs.
3. **Connecter** : Apply fournit une sélection et un ticket de génération. `Target::connect` vérifie le ticket et l'annulation avant et après l'ouverture. Une panne d'identification n'expose pas de writer. Les deux pilotes migrés gardent le même fd HID ou la même paire de fichiers sysfs ; aucune réouverture au changement de couleur.
4. **Commander** : le moteur reçoit une `Session` non clonable. Sa seule opération est `set_rgb(&mut self, rgb)`. Le premier envoi vérifie à nouveau le ticket ; le drapeau d'arrêt est contrôlé à chaque appel. Les trois effets animés sont des séquences de couleurs calculées par LinRgb, pas des modes autonomes inférés. Pas de lecture de la couleur courante, d'édition individuelle des lampes ou de modification persistante.
5. **Révoquer** : annulation observée, ticket périmé avant le premier envoi ou erreur d'écriture : la session abandonne ses fds et refuse les appels suivants. Les générations empêchent de démarrer une commande en attente ; un simple rafraîchissement n'arrête pas une session déjà attachée si la réconciliation retrouve le même endpoint accessible. Après veille/remplacement, pas de reprise automatique.

Le modèle `Capabilities` conserve uniquement les faits vérifiés : disposition RGB/RGBW et échelle pour sysfs ; nombre de lampes et intervalle pour WDL. La session WDL ignore une trame trop tôt sans bloquer les autres sorties ni la marquer envoyée. Les couleurs identiques restent filtrées par le moteur. La description apparaît après une opération réussie ; le diagnostic passif n'ouvre pas de session pour compléter des champs. `Target::interface` décrit les permissions nécessaires. Les autres backends restent `LegacyDirect` ; aucun nombre de zones ou débit de fiche technique n'est présenté comme mesuré.

## Limites

- Une opération déjà engagée (trame, série d'écritures ou appel noyau) n'est pas annulable rétroactivement. La révocation d'une ACL ne ferme pas un fd Linux déjà ouvert ; la réconciliation ou une erreur I/O doit d'abord la signaler. LinRgb n'acquiert pas de verrou interprocessus contre d'autres logiciels RGB.
- L'identité sysfs est locale à une instance kernfs ; ce n'est pas un numéro de série. Un fd i2c-dev identifie un adaptateur, pas une puce immuable à une adresse. La signature au démarrage n'est pas une preuve permanente de présence. Ne jamais remplacer de RAM, de carte PCIe ou de câble ARGB interne à chaud.
- Le contrat ne promet pas que tous les pilotes connectés soient exempts de *transactions* : certaines identifications SMBus nécessitent un sélecteur suivi d'une lecture. Celles-ci restent limitées à l'ouverture explicite, pas à la découverte. Aucun nouveau registre utilisé.

## Vérifications et recette physique

Format, Clippy strict et `cargo test --all-targets --locked` : **51 tests**. Fixtures : changement de canaux à inode inchangé, remplacement de chemin, absence d'écriture lors de la vérification, attributs WDL erronés, annulation, `ENODEV`/`EACCES`. Une barrière entre deux threads force un rafraîchissement pendant la vérification sans minuterie aléatoire. Un test vérifie la limite d'intervalle avec des instants synthétiques. Ces tests ne prouvent pas la fluidité sur un vrai hub.

`nix flake check --no-update-lock-file` réussi ; paquet Nix construit et diagnostic lancé sans affichage. Paquets Arch, Debian 13 et Fedora 44 construits depuis l'archive du commit local, installés en conteneurs ; 51 tests par distribution et `linrgb --diagnostics` exécuté par un utilisateur ordinaire. Recettes épinglées et règles udev du dépôt inchangées. Les archives de validation ne constituent pas une publication.

Validation physique **en attente** : suivre la [recette du jalon 2](device-lifecycle.md#recette-physique-non-destructive-à-faire). D'abord, sans Apply, vérifier absence de changement de couleur au démarrage, au rafraîchissement et après reprise. Ensuite, si autorisé, sélectionner le hub connu et appliquer une couleur fixe ; relever nombre de lampes et intervalle indiqués. Répéter sur une LED sysfs connue ; vérifier la couleur et l'ordre des canaux. Fermer les autres outils RGB et rétablir la couleur choisie au départ. Ne pas tester une nouvelle adresse SMBus pour cette recette.

## Suite proposée

Migrer ensuite Aura USB : obtenir des fixtures de version/configuration pour les deux cartes existantes et préciser les zones réellement lisibles. Tester absence, ambiguïté et données tronquées avant d'exposer des capacités. Pour ENE/Corsair, traiter séparément l'identité de l'adaptateur et celle de la puce. Introduire un registre statique de pilotes uniquement si ces migrations répètent le même contrat ; aucune ABI de plugin dynamique nécessaire.
