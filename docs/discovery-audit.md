# Inventaire matériel — audit du premier jalon

Base : `67ff099`. Charge visée : une application graphique locale, plusieurs transports, un effet actif. Travail isolé du prototype de tray. Aucun essai sur bus physique.

| Priorité | Constat et impact | Décision et preuve attendue |
|---|---|---|
| P0 | `corsair::open` envoie deux sélecteurs d'information avant de vérifier le fabricant. Le DMI d'une B550-A ne dit rien sur les barrettes installées. | Ne jamais ouvrir le SMBus pendant l'inventaire. Exiger une signature en lecture sur l'adresse explicitement sélectionnée avant les sélecteurs ; conserver VID/PID, protocole et CRC avant la couleur. Tester le refus au premier registre inconnu. |
| P0 | `board::open_hid` sélectionne par sysfs puis ouvre un numéro `hidrawN` recyclable. Un débranchement entre les deux opérations invalide le choix. | Vérifier le descripteur de fichier ouvert avant tout rapport ; refuser les correspondances ambiguës. Tester les types et identifiants en simulation. |
| P1 | `main.rs` construit des cases Crosshair/ENE sur tous les PC et propose la B550 sur le seul DMI. Deux listes distinctes alimentent recherche, sélection et Apply. Les parcours de sysfs se font sur GTK. | Une seule source de cibles, instantané passif et racines injectables. Exiger le HID correspondant pour Aura ; les adresses DIMM sont des **candidats**, pas des barrettes détectées. Tester l'absence de matériel, les fausses correspondances et le retrait. |
| P1 | `lamp_array` valide déjà le descripteur exact, mais le chemin du périphérique HID incorpore un compteur de connexion. Le catalogue PCI n'est pas un pilote. | Séparer identité de sélection et chemin d'ouverture ; garder les contrôles au moment de l'ouverture. Conserver l'inventaire sans pilote. Ne pas étendre les permissions. |
| P2 | Le moteur partage déjà son horloge, saute les trames en retard et les couleurs identiques ; une erreur de trame retire ce writer. Les opérations initiales restent séquentielles par groupe. | Ne pas modifier la cadence sans traces par transport. Mesurer latences et conflits avec les interfaces LED du noyau sur matériel réel ; les groupes sérialisent mais ne prouvent pas l'exclusivité physique. |
| P2 | Pas de vue complète des ACL, ni de suivi complet de reprise. Des noms matériels ou chemins libres ne sont pas un bon format de diagnostic à publier. | Établir un rapport local à champs autorisés ; ne pas exporter numéros de série, DMI brut, profils, chemins personnels. Tester révocation d'accès et veille avant toute restauration automatique. |
| P3 | L'ARGB B550 suppose 100 positions ; des GPU ENE partagent des registres mais pas forcément le même câblage. Les identifiants de marque ne prouvent pas un protocole. | Avant extension : collecter des fixtures Aura (version, configuration, zones), spécifier bornes et variantes, tester les refus, puis tester physiquement. Pas de pilote générique par marque. |

## Choix d'intégration

- L'ABI sysfs donne la topologie, le DMI, les identifiants HID et les canaux LED sans ouvrir de nœud de contrôle. Ne pas lire les EEPROM SPD pour remplir une liste.
- Le modèle du noyau ne fournit pas une fonction universelle de détection sûre sur I²C. Les règles `uaccess` ciblent un adaptateur, pas un registre ou une adresse. Une ACL sur le bus donne accès au bus entier. Conserver les garde-fous dans les pilotes.
- Les événements udev sont préférables pour couvrir tous les sous-systèmes. Un moniteur GIO sur `/dev` permet déjà de suivre hidraw/i2c-dev sans boucle d'inventaire ni nouvelle dépendance. La reprise est traitée séparément par `PrepareForSleep(false)` sur le bus système logind. Les événements LED sans changement de nœud `/dev` restent couverts par l'actualisation manuelle.
- Le chemin générique intéressant à court terme pour d'autres marques (MSI, Gigabyte, G.Skill, Kingston) est une interface multicolore Linux **quand le noyau l'expose**, pas un accès propriétaire supposé compatible. Conserver les autres appareils en inventaire seul.
- Conserver les formats de profils et thèmes, GTK natif, le moteur d'effets, les versions épinglées, le module NixOS et les permissions. Pas de nouvelle promesse de distribution : vérifier la licence et la provenance du catalogue avant publication.

## Sources et limites

[Énumération I²C Linux](https://docs.kernel.org/i2c/instantiating-devices.html), [hidraw et HIDIOCGRAWINFO](https://docs.kernel.org/hid/hidraw.html), [classe LED multicolore](https://docs.kernel.org/leds/leds-class-multicolor.html). Référence de protocole : [détection Corsair DRAM dans OpenRGB](https://github.com/CalcProgrammer1/OpenRGB/blob/5bcfa9ae01536d2dd45b0c6e5b15677513f7a6d5/Controllers/CorsairDRAMController/CorsairDRAMControllerDetect.cpp), registre et contrôleur Aura au même commit. Pas d'import de pilote tiers.

Baseline propre : format et Clippy OK, 32 tests, flake check OK. Dans la copie locale avec le tray non publié : 34 tests et Clippy OK ; le flake Git échoue car `src/tray-icon.argb` n'est pas suivi. Ne pas inclure cet autre chantier dans le jalon.

## Résultat du jalon et recette physique proposée

`src/discovery.rs` construit un instantané sans écrire ; `src/device_list.rs` le présente dans GTK et regroupe les demandes concurrentes. La présence du bus et du DMI ne suffit plus à sélectionner une DIMM via « Tout sélectionner ». Les pilotes vérifient encore leurs préconditions à l'ouverture. Un parent natif accessible a priorité sur un alias LED situé sous le même périphérique sysfs ; une relation physique invisible dans sysfs reste indécidable. Pas de réapplication automatique après retrait.

Tests automatisés : format, Clippy strict, 39 tests Rust et `nix flake check --no-update-lock-file`. Un diagnostic du binaire Nix sans serveur graphique a produit du JSON valide, sans lancer d'effet. Les profils et les recettes ne sont pas migrés. Aucun résultat de débit ni contrôle RGB physique revendiqué pour cette branche.

1. Sur Crosshair puis B550-A : comparer `--diagnostics` et la liste affichée. Hors effet, débrancher et rebrancher le hub ; vérifier une seule ligne WDL et l'absence d'écriture spontanée.
2. Sur les deux cartes : sélectionner une seule barrette connue, tester deux couleurs successives, puis l'effet et son annulation. Sur B550-A, vérifier séparément la zone fixe et le header de la XFX.
3. Suspendre puis reprendre ; tester la liste et les permissions en session active. Vérifier les cas sans logind et sans moniteur `/dev` avec le bouton d'actualisation.
4. Avant extension à d'autres familles : collecter des descripteurs de zones Aura, établir quelles variantes partagent réellement le mode direct, puis tester l'identification négative. Ne pas modifier les cadences ni ouvrir d'autres bus sur hypothèse.
