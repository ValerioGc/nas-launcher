# Installazione sul NAS

Script per eseguire il launcher su una macchina Debian 13 reale. **Non sono necessari per lo
sviluppo.** Sul NAS li esegue il wizard, un progetto a sé (`../nas-wizard`): prima configura il sistema
operativo e installa il gateway, poi scarica questo pacchetto ed esegue `install.sh` e
`../ui_configuration/install.sh`. L'aggiornamento del launcher riesegue `install.sh` dal pacchetto
nuovo.

Questi script servono quando i comandi di alimentazione devono agire davvero.

## Contenuto

Il pacchetto si costruisce con `npm run release:package` (lo stesso comando usato dalla pipeline) e
si trova in `release-assets/`. Accanto ai file di questa cartella contiene il binario, l'interfaccia
compilata e gli script che il backend fa eseguire alla macchina, che nel repository stanno accanto
al modulo Rust che li usa.

| File                               | Scopo                                                                                   |
| ---------------------------------- | --------------------------------------------------------------------------------------- |
| `install.sh`                       | Installa o aggiorna utente, binario, interfaccia, permessi e unità systemd              |
| `scripts/check.sh`                 | Verifica che tutto sia in ordine, senza riavviare o spegnere                            |
| `scripts/uninstall.sh`             | Rimuove tutto; i dati restano, salvo con `--purge`                                      |
| `files/nas-os.service`             | L'unità systemd, con le modalità reali e l'hardening                                    |
| `files/10-nas-os-power.rules`      | Le regole polkit: riavvio, spegnimento e allarme di accensione, solo per il launcher    |
| `files/20-nas-os-network.rules`    | La regola polkit di rete: scansione, connessione e salvataggio del profilo              |
| `files/30-nas-os-firewall.rules`   | La regola polkit della lista di accesso: solo `start`, solo per quell'unità             |
| `files/40-nas-os-update.rules`     | La regola polkit dell'aggiornamento del launcher: solo `start` di quell'unità           |
| `files/50-nas-os-apps.rules`       | La regola polkit delle applicazioni: `start`, `stop`, `restart` di `nas-app@<id>`       |
| `files/nas-os-wake.service`        | L'unità che imposta l'allarme dell'orologio, eseguita come root                         |
| `files/nas-os-firewall.service`    | L'unità che applica come root la lista degli indirizzi ammessi                          |
| `files/nas-os-update.service`      | L'unità che sostituisce il binario del launcher con quello scaricato                    |
| `files/nas-app@.service`           | Il template da cui gira ogni applicazione installata                                    |
| `files/nas-os-app-grants@.service` | L'unità che scrive come root i percorsi concessi a un'applicazione                      |
| `files/55-nas-os-app-grants.rules` | La regola polkit dei permessi delle app: solo `start` di `nas-os-app-grants@<id>`       |
| `files/60-nas-os-apt.rules`        | La regola polkit dei pacchetti di sistema: solo `start` di refresh e upgrade            |
| `files/nas-os-apt@.service`        | Il template che aggiorna l'indice o installa gli aggiornamenti Debian, come root        |
| `files/65-nas-os-processes.rules`  | La regola polkit dei processi: solo `start` di `nas-os-process@<azione>`                |
| `files/nas-os-process@.service`    | Il template che termina un processo o riavvia il suo servizio, come root                |
| `files/70-nas-os-token.rules`      | La regola polkit del token delle release: solo `start` di `nas-os-token.service`        |
| `files/nas-os-token.service`       | L'unità che cifra il token ricevuto dal wizard o dall'App Manager, come root            |
| `files/75-nas-os-reset.rules`      | La regola polkit del ripristino: solo `start` di `nas-os-reset.service`                 |
| `files/nas-os-reset.service`       | L'unità che cancella i dati di launcher, gateway e app e riavvia il wizard, come root   |
| `files/catalogue.json`             | Il catalogo iniziale delle applicazioni installabili                                    |
| `files/logos/*`                    | I loghi predefiniti delle applicazioni note, copiati in `/usr/local/share/nas-os/logos` |
| `apps/*.tar.gz`                    | Pacchetti opzionali copiati da `system/apps` durante la build                           |
| `bin/nas-os-axum`                  | Il binario del launcher, aggiunto dal pacchetto                                         |
| `ui/`                              | L'interfaccia compilata, aggiunta dal pacchetto                                         |
| `helpers/*.sh`                     | Gli script eseguiti dalle unità, copiati da `backend/axum/src/`                         |

Gli helper, nel repository:

| Script                       | Modulo                                                               |
| ---------------------------- | -------------------------------------------------------------------- |
| `wake-alarm.sh`              | `backend/axum/src/apps/services/nas_system_settings/power/`          |
| `apply-access-rules.sh`      | `backend/axum/src/apps/services/nas_system_settings/access/`         |
| `run-app.sh`                 | `backend/axum/src/services/lifecycle/`                               |
| `apply-app-grants.sh`        | `backend/axum/src/services/grants/`                                  |
| `install-launcher-update.sh` | `backend/axum/src/apps/services/nas_system_settings/updates/`        |
| `apt-updates.sh`             | `backend/axum/src/apps/services/nas_system_settings/system_updates/` |
| `launcher-token.sh`          | `backend/axum/src/services/packages/`                                |
| `terminal-user.sh`           | `backend/axum/src/services/terminal/`                                |
| `apply-hostname.sh`          | `backend/axum/src/apps/services/nas_system_settings/`                |
| `factory-reset.sh`           | `backend/axum/src/apps/services/nas_system_settings/reset/`          |

## Utilizzo

Di solito non serve: lo fa il wizard. A mano, su una macchina già configurata dal wizard:

```bash
# 1. Costruisci il pacchetto (binario, interfaccia, helper)
npm run release:package

# 2. Sul NAS: verifica, scompatta e installa (può essere eseguito di nuovo a ogni aggiornamento)
sha256sum -c nas-os-<versione>-linux-x64.tar.gz.sha256
mkdir nas-os && tar -xzf nas-os-<versione>-linux-x64.tar.gz -C nas-os
cd nas-os/packages/ui_installation
sudo ./install.sh

# 3. Verifica, anche più avanti: lo script resta sulla macchina
sudo /usr/local/lib/nas-os/check.sh
```

Prima della build, collocare eventuali archivi in `system/apps/<id>.tar.gz`. L'archivio deve
contenere `nas-app.json` alla radice e dichiarare lo stesso `<id>`. Il pacchetto di release li
include; `install.sh` li copia temporaneamente nella directory di stato e li estrae come utente
`nas-os` solo se l'app non è già presente. Al primo
avvio il launcher registra i manifest trovati in SQLite e AppManager li mostra come installati.
Il catalogo comprende `nas-picture-manager` come app privata scaricabile dal repository
`ValerioGc/nas-picture-manager`; non occorre preinstallarla. Il suo selettore NAS richiede
Explorer (`nas-file-manager`) installato con accesso ai volumi dati. Per usare il selettore su un
NAS esistente aggiornare launcher, Explorer e Picture Manager. Il catalogo esistente viene
conservato: sostituire un'eventuale voce `picture-manager` con `nas-picture-manager`, repository
`ValerioGc/nas-picture-manager` e asset `nas-picture-manager-{version}-linux-x64.tar.gz`.
Il nuovo pacchetto va installato con il nuovo ID; gli aggiornamenti non rinominano le app.
Verificare che il nome dell'asset della release corrisponda alla voce `asset` del catalogo.

`install.sh` accetta anche il percorso di un binario già compilato:
`sudo ./install.sh /tmp/nas-os-axum`.

## Cosa fa concretamente

1. **Utente di servizio** `nas-os`, di sistema, senza shell né password: il launcher non viene
   eseguito come root.
2. **Binario** in `/usr/local/bin/nas-os-axum` e **interfaccia** in `/usr/local/share/nas-os/ui`.
   La serve il gateway, un progetto a sé con la sua installazione: è l'ingresso dal browser sulla
   porta 80, gestisce l'accesso e inoltra al launcher, che resta su `127.0.0.1`.
3. **Una regola polkit** che consente a quell'utente **solo** `reboot` e `power-off`. Non concede
   le varianti `ignore-inhibit`: se un backup sta trattenendo lo spegnimento, il launcher lo rispetta.
4. **Un'unità systemd** con `StateDirectory=nas-os` (database, permessi `0710`: il gruppo può solo
   attraversarla, per raggiungere i pacchetti delle applicazioni; i file del launcher restano suoi),
   le impostazioni lette da `/etc/nas-os/nas-os.env` (fra cui `NAS_POWER_MODE=real`) e le protezioni systemd (`ProtectSystem=strict`, `NoNewPrivileges`,
   filtro delle chiamate di sistema).
5. **L'allarme dell'orologio**: `nas-os-wake.service` e il relativo script in
   `/usr/local/lib/nas-os/`. Non è abilitato all'avvio, perché non deve partire autonomamente.
6. **Permessi di rete** per cercare reti wireless e collegarsi a una di esse, e
   `NAS_NETWORK_MODE=real` nell'unità, che rende reale la modifica della rete.
7. **La lista degli indirizzi ammessi**: `nas-os-firewall.service` e il relativo script, più
   `NAS_FIREWALL_MODE=real` nell'unità. Come l'allarme, non è abilitato all'avvio: il launcher la
   avvia ogni volta che la lista cambia. Lo script accetta dalla lista solo caratteri di indirizzi
   e sostituisce la tabella in una transazione sola.
8. **Pacchetti delle applicazioni**: il catalogo in `/etc/nas-os/catalogue.json` (mai
   sovrascritto), i loghi predefiniti in `/usr/local/share/nas-os/logos`, la cartella
   `/var/lib/nas-os/apps` e le applicazioni preinstallate da `apps/`. I pacchetti già presenti
   vengono resi leggibili al gruppo `nas-os` (`chmod -R g=u-w`): il processo di ogni applicazione
   gira come un altro utente di quel gruppo, e un pacchetto scompattato da un launcher precedente
   restava leggibile solo al launcher. `scripts/check.sh` segnala i file che non lo sono.
9. **Le applicazioni**: il template `nas-app@.service` e `run-app.sh`, vedi sotto.
10. **Gli aggiornamenti**: `nas-os-update.service` per il launcher e `nas-os-apt@.service` per i
    pacchetti Debian, entrambi avviati solo dal launcher.
11. **Il token delle release**: `nas-os-token.service` e `launcher-token.sh`, avviati dal launcher
    quando il token arriva dall'App Manager. Il wizard lo salva da root con `ui_configuration`.
12. **Il terminale**: l'app esterna `nas-system-terminal` gestisce WebSocket e PTY; `NAS_TERMINAL_MODE=real`
    e il gruppo `systemd-journal` consentono al launcher di fornire il journal. Il wizard esegue
    `terminal-user.sh` da root: crea l'utente con il nome dell'account, senza password e senza sudo,
    e abilita `nas-os-terminal.socket`. Ogni connessione avvia il binario dell'app con `--shell`
    tramite `nas-os-terminal@.service`, come quell'utente. Il gruppo supplementare `nas-os` della
    sessione consente di leggere il pacchetto installato; non modifica i gruppi dell'account.
    La sessione permette `su - nas` con la password Linux di `nas` e successivamente `sudo` secondo
    i permessi già assegnati a quell'account. L'unità dedicata usa `NoNewPrivileges=no`; le
    protezioni dei servizi HTTP del launcher e delle app restano attive. `KillMode=control-group`
    termina anche i processi rimasti alla chiusura. Il launcher conserva soltanto impostazioni,
    journal e configurazione dell'utente, e rimuove le vecchie unità polkit del terminale.
13. **Gli strumenti che restano**: `check.sh`, `uninstall.sh` e `configure.sh` (la copia di
    `ui_configuration/install.sh`) in `/usr/local/lib/nas-os/`, perché il pacchetto non resta
    sulla macchina.

Le unità del launcher e delle applicazioni non hanno capability e limitano orologio, log del
kernel, hostname e famiglie di socket: `systemd-analyze security --offline=yes files/nas-os.service`
le valuta anche prima di installarle.

## Apertura delle applicazioni

Un'applicazione installata è un processo autonomo su un socket unix. Il launcher la serve su
`/apps/<id>/`, sullo stesso indirizzo e sulla stessa porta dell'interfaccia, dietro l'accesso del
gateway: nessuna configurazione di rete né dei computer da cui si naviga, anche con una sola porta
inoltrata. L'installazione prepara una directory e
due righe nell'unità:

- `RuntimeDirectory=nas-os/apps` crea `/run/nas-os/apps` e
  `RuntimeDirectoryPreserve=yes` la conserva durante un riavvio del launcher, perché le
  applicazioni non vengono riavviate insieme a esso.
- `NAS_APP_SOCKETS` indica dove sono i socket.

L'avvio delle applicazioni usa un template e una regola:

- `nas-app@.service` esegue un'applicazione per istanza e `run-app.sh` legge dal manifest
  dell'applicazione quale programma eseguire. Il template non viene mai abilitato: il launcher
  avvia un'istanza quando un pacchetto viene installato e quando qualcuno preme il pulsante.
- `50-nas-os-apps.rules` consente al launcher di eseguire `start`, `stop` e `restart` su un'unità
  con nome nella forma `nas-app@<id>.service`, e nient'altro. Non `enable`.
- `NAS_APPS_MODE=real` nell'unità è ciò che abilita tutto questo.

Un'applicazione viene eseguita con `DynamicUser=yes`: systemd crea un utente che non possiede
niente e che ha il gruppo `nas-os` tra i gruppi supplementari. La directory dei socket è setgid,
quindi il socket creato eredita quel gruppo, e `UMask=0007` permette al launcher di aprirlo. È
anche sticky (`3770`): un'applicazione non può rimuovere il socket di un'altra. Gira con due
worker di tokio e due arene di glibc (`TOKIO_WORKER_THREADS=2`, `MALLOC_ARENA_MAX=2`) e senza
memoria scrivibile ed eseguibile insieme (`MemoryDenyWriteExecute`); il launcher e il gateway con
quattro worker: i valori predefiniti crescono con i core della macchina e tengono memoria che non
serve.

Il template nasconde a ogni applicazione i dischi dati (`/srv`, `/mnt`, `/media`), i log, lo stato
del launcher tranne il pacchetto dell'app, `/sys`, `/run/udev` e i processi degli altri. Quello che
il manifest chiede in `permissions.paths` il launcher lo concede da un elenco fisso (`/srv/storage`,
e in sola lettura `/proc`, `/sys`, `/run/udev`) e rifiuta di installare un pacchetto che chiede
altro; `nas-os-app-grants@<id>.service` (regola polkit 55, solo `start`) lo scrive come
`/etc/systemd/system/nas-app@<id>.service.d/grants.conf` con `apply-app-grants.sh`. Chi ottiene
`/srv/storage` (Explorer e Backup) riceve il gruppo `nas-storage`, `BindPaths=/srv/storage`,
`NAS_STORAGE_ROOT=/srv/storage` e `NAS_STORAGE_REGISTRY=/etc/nas-os/storage-volumes.tsv`.
`/srv/storage` è attraversabile dal gruppo `nas-storage`; il mount grezzo sotto `/srv/nas-volumes` è accessibile solo a root.
Dal widget dei processi il launcher termina un processo (SIGTERM) o riavvia il servizio in cui gira
avviando `nas-os-process@<terminate|restart>-<pid>-<avvio>.service` (regola polkit 65, solo
`start`). `act-on-process.sh` ricontrolla tutto come root: l'istante di avvio deve essere quello
mostrato, i thread del kernel e il processo 1 restano intoccabili, e i processi delle unità che
tengono in piedi il NAS (launcher, gateway, D-Bus, polkit, `systemd-*`, NetworkManager, ssh) si
possono solo riavviare. Richiede `NAS_PROCESSES_MODE=real`.

Il servizio root `nas-os-storage.service` rileva i filesystem dati tramite UUID, esclude i
filesystem di sistema, monta ogni volume sotto `/srv/nas-volumes/v-<uuid>` e pubblica in
Explorer soltanto la directory `.nas-users/<utente>` del wizard, tramite bind mount sotto
`/srv/storage/v-<uuid>`. L'utente lo legge dal drop-in che `terminal-user.sh` scrive in
`/etc/systemd/system/nas-os-terminal@.service.d/user.conf`, e lo accetta solo se l'account ha il
commento `nas-os terminal`: il database del launcher lo apre soltanto il launcher. Il timer e la regola udev ripetono la riconciliazione dopo boot o
hotplug. Dopo aver creato l'utente, `ui_configuration` avvia una riconciliazione, perché quella fatta
all'installazione non aveva ancora un'area utente in cui pubblicare i volumi. I file già presenti fuori
dall'area utente non vengono toccati o mostrati; partizioni non formattate e filesystem non
supportati non compaiono. Il registro contiene solo
i volumi pronti; gli errori dei mount sono in `journalctl -u nas-os-storage.service`. Le cartelle
sotto `/srv/nas-volumes` e `/srv/storage` senza un filesystem montato restano con permessi `000`:
un volume scollegato non diventa una cartella del disco di sistema per chi ci scrive. Quando il
registro cambia la riconciliazione riapplica le cartelle condivise, e `apply-smb-shares.sh` lascia
fuori quelle di un disco non montato.
La classificazione dei dischi è una sola (`services/storage/inventory.rs`): la usano la
riconciliazione, le unità root e `GET /api/storage/disks`, che nas-disk-manager legge attraverso
la shell (`disk-inventory`). Un disco che contiene il sistema non si cancella in nessuna sua parte,
anche quando ha un volume dati montabile; ogni voce porta l'identità del disco intero (WWN, seriale
e dimensione), che una richiesta distruttiva ripete e l'unità root confronta.
I dischi interi vuoti possono essere inizializzati esplicitamente da nas-disk-manager. Il launcher
accetta la richiesta solo da quell'app e avvia l'unità root fissa
`nas-os-storage-initialize.service`; l'unità ricontrolla che il dispositivo sia scrivibile, senza
partizioni, mount, filesystem o firme, e che sia ancora il disco scelto, crea una partizione GPT
ext4 e richiama la riconciliazione.
`parted` ed `e2fsprogs` li installa il wizard con la configurazione del sistema; nessun disco viene inizializzato
automaticamente durante l'installazione.
La cifratura dei dischi dati (LUKS2) passa da `nas-os-storage-crypt.socket`: solo il gruppo
`nas-os-crypt`, che ha soltanto il launcher, può aprirlo (le applicazioni hanno il gruppo `nas-os`,
non questo), e ogni connessione avvia `nas-os-storage-crypt@.service`, che esegue come root
`nas-os-axum --storage-crypt`. La richiesta, password comprese, arriva sulla connessione; le chiavi
restano nella cartella runtime dell'unità (tmpfs, 0700) per il tempo di un comando, mai in un
argomento o nel journal. Formatta un disco dati interno (identità ripetuta, mai il disco di sistema,
mai un disco con LVM o RAID), sblocca, blocca, imposta password, sblocco automatico e chiave di
recupero, esporta l'intestazione. Gli slot: 0 password, 1 chiave automatica, 2 chiave di recupero.
La chiave automatica è una credenziale cifrata di systemd (`/etc/credstore.encrypted/nas-os.luks-<uuid>`,
chiave della macchina e TPM se presente); la riconciliazione apre con essa i volumi dopo l'avvio,
senza crypttab, così un errore non blocca il boot. Un volume bloccato dall'utente resta in
`/var/lib/nas-os/storage-held` e non viene riaperto, neanche dopo un riavvio; le chiavi presenti
le scrive la riconciliazione in `/var/lib/nas-os/storage-crypt.json`. È accesa di predefinito:
`NAS_ENCRYPTION_MODE=off` in `/etc/nas-os/nas-os.local.env` la spegne. La
disinstallazione lascia le chiavi automatiche al loro posto e stampa come aprire un volume a mano.
Lo stesso socket porta le richieste dei dischi di rete (SMB): server, condivisione, utente e dominio
stanno in `/etc/nas-os/network-disks.json`, la password nella credenziale cifrata
`nas-os.smb-<id>`. `mount.cifs` (pacchetto `cifs-utils`, chiesto dall'app Dischi) la legge da un
file della cartella runtime, tolto dopo il montaggio. La riconciliazione monta su
`/srv/storage/n-<id>` le condivisioni che si collegano all'avvio, se l'utente non le ha scollegate
(`storage-held`), ognuna entro 20 secondi e tutte entro 30, dopo aver pubblicato i dischi locali,
e le pubblica a Explorer, Backup e Sync come volumi `network`. Il kernel raggiunge il server sulla
445 solo verso gli indirizzi della rete locale messi nei set `nas_smb_servers` e
`nas_smb_servers6` del firewall di base; Samba non ricondivide un disco di rete. Accesa di
predefinito, `NAS_NETWORK_DISKS_MODE=off` la spegne. La disinstallazione completa toglie le
password dei dischi di rete.
nas-disk-manager può anche rinominare i volumi ext2/ext3/ext4 scrivibili tramite l'unità root fissa
`nas-os-storage-rename.service`; il launcher accetta la richiesta soltanto da nas-disk-manager e il
backend ricontrolla dispositivo, filesystem e appartenenza all'area dati prima di usare `e2label`, e
rifiuta un volume che fstab monta per etichetta: la riga resterebbe senza filesystem al riavvio.
La lettura S.M.A.R.T. usa allo stesso modo `nas-os-storage-smart.service`: il browser non accede ai
device, il backend valida che si tratti di un disco dati e l'unità esegue `smartctl` in sola lettura.
Un disco esterno (rimovibile, USB, lettore di schede o FireWire) si espelle da nas-disk-manager con
`nas-os-storage-eject.service`: l'unità ricontrolla che il disco sia esterno e senza filesystem del
sistema, smonta i suoi volumi (prima le viste di Explorer, poi i mount grezzi, poi gli altri), lo
stacca dal kernel con `/sys/block/<disco>/device/delete` e pubblica i volumi rimasti. Il launcher
rifiuta un disco segnato come non removibile: il segno sta nella tabella `storage_locks`, per UUID
del filesystem, e resta quando il disco viene ricollegato.
La disinstallazione di un'app toglie il suo drop-in e lascia mount, registro e dati utente al loro
posto.

## Accensione programmata

Una regola di accensione non può essere eseguita: con la macchina spenta non gira nulla. Viene
impostata **in anticipo** nell'orologio hardware, che resta alimentato dalla batteria sulla scheda:

1. Arriva l'orario di una regola di spegnimento; lo scheduler del launcher verifica se sta per
   arrivare anche una regola di accensione.
2. Se presente, scrive quell'orario in `/var/lib/nas-os/wakealarm` e avvia
   `nas-os-wake.service`.
3. Quell'unità, eseguita come root, lo copia in `/sys/class/rtc/rtc0/wakealarm`.
4. Solo allora il launcher spegne la macchina.

Il launcher non scrive mai direttamente nell'orologio: non è root e non deve diventarlo. La regola
polkit gli consente di avviare **quell'unità** e nient'altro, né di arrestarla né di abilitarla.

Se qualcosa non va, l'allarme non viene impostato, ma **lo spegnimento avviene comunque**, con una
notifica che indica cosa manca: meglio un NAS spento da accendere a mano che uno che resta acceso
silenziosamente.

> **Due cose non dipendono da questi script.** Il BIOS deve avere abilitato il risveglio RTC (la
> voce può chiamarsi _Wake on RTC_, _RTC Alarm_ o _Resume by Alarm_) e la macchina deve restare
> collegata all'alimentazione. Senza questi requisiti il file viene scritto, ma la macchina non si
> accende. La prima accensione programmata è l'unica verifica che conta: `check.sh` può dire che
> tutto è installato, non che il BIOS eseguirà l'operazione.

Il launcher resta in ascolto **solo su `127.0.0.1`**: dall'esterno verrà raggiunto attraverso il
gateway di autenticazione, non aprendo una porta.

## Rete

La lettura della rete (interfaccia, indirizzo, SSID, segnale) funziona ovunque e non richiede
permessi: `ip`, sysfs e `nmcli` vengono usati in sola lettura. **Modificarla** è diverso e richiede
due elementi:

1. `NAS_NETWORK_MODE=real` nell'unità, altrimenti il backend risponde 503 dichiarando che in
   quell'installazione la rete non viene toccata. Questo evita che chi sviluppa sposti il Wi-Fi
   del proprio portatile premendo un pulsante nell'interfaccia.
2. La regola polkit, che concede all'utente di servizio **tre** azioni NetworkManager: cercare
   reti, attivare una connessione e salvarne il profilo. Non concede le azioni che spengono la
   rete: un NAS che si toglie la rete da solo non sarebbe raggiungibile per ripristinarla.

Il launcher non scrive a mano file di configurazione e non costruisce comandi da stringhe: usa
`nmcli` con ogni argomento separato, così un SSID con spazi, virgolette o punti e virgola resta un
SSID.

## Indirizzi ammessi

La lista viene scritta dall'interfaccia, nelle impostazioni di accesso, ed è applicata in due punti
diversi perché nessuno dei due basterebbe da solo.

1. **Nel launcher**: una richiesta che arriva da un indirizzo non presente nella lista riceve un
   `403`. Copre ciò che comunica direttamente con il launcher.
2. **Nel firewall**: `nftables` scarta il pacchetto prima che diventi una richiesta e così copre
   **il resto del NAS**, non solo il launcher. Richiede `NAS_FIREWALL_MODE=real`, la regola polkit
   e `nft` installato; il launcher scrive la lista in `/var/lib/nas-os/access-rules` e avvia
   l'unità, che è l'unica azione che può compiere.

Tre elementi non fanno mai parte del filtro, evitando che la prima regola salvata isoli la macchina
da sé stessa: **loopback**, connessioni **già stabilite** e la coppia **DHCP più ICMP**, senza cui
il NAS perderebbe il proprio indirizzo al successivo rinnovo del lease e IPv6 smetterebbe del tutto
di funzionare.

**Una lista vuota non chiude nulla.** La restrizione entra in vigore aggiungendo la prima regola,
non installando lo script: uno script che bloccasse la macchina appena copiato sarebbe una trappola.

## Le impostazioni

Le variabili del launcher stanno in `/etc/nas-os/nas-os.env`, copia di `config/production.env` del
repository, che ogni installazione e ogni aggiornamento sostituiscono. Per cambiarne una su questa
macchina senza perderla al prossimo aggiornamento si scrive `/etc/nas-os/nas-os.local.env`, che ha
la precedenza, e si riavviano i servizi:

```bash
echo 'NAS_APT_MODE=simulated' | sudo tee -a /etc/nas-os/nas-os.local.env
sudo systemctl restart nas-os nas-os-gateway
```

Una riga `CHIAVE=valore` per variabile, senza virgolette. `scripts/check.sh` legge i valori dai due
file.

## Senza questi script

Il backend si avvia comunque e risponde alle azioni con `"simulated": true`: registra la notifica
e non tocca la macchina. È il comportamento predefinito dove manca `NAS_POWER_MODE=real` ed esiste
proprio per evitare che qualcuno spenga il computer su cui sta sviluppando.

## Quando qualcosa non funziona

`check.sh` indica quale elemento manca. I casi più comuni:

- **`org.freedesktop.login1.reboot refused`**: manca la regola polkit oppure polkit non è stato
  riavviato. Il launcher risponde `503` con la ragione, invece di fingere di aver spento la macchina.
- **`power mode: simulated`**: in `/etc/nas-os/nas-os.env` manca `NAS_POWER_MODE=real`, oppure
  `/etc/nas-os/nas-os.local.env` lo cambia.
- **Le reti sono elencate ma la connessione non parte**: manca `NAS_NETWORK_MODE=real` (il backend
  risponde 503 e lo dichiara), oppure manca la regola polkit di rete: in quel caso l'errore mostrato
  nella scheda è quello di `nmcli`, di solito una richiesta di autorizzazione.
- **`/health non risponde`**: il servizio non è partito: `journalctl -u nas-os -n 50`.
- **La macchina si spegne ma non torna accesa**: controllare cosa ha detto l'unità wake con
  `journalctl -u nas-os-wake -n 20`. Se dice "alarm set", manca la configurazione del BIOS. Se non
  esiste traccia dell'unità, manca la regola polkit: il launcher non ha potuto avviarla e nelle sue
  notifiche compare "Wake alarm not set" con la ragione.
- **La macchina si accende da sola quando non dovrebbe**: nell'orologio è ancora presente un
  allarme di una pianificazione precedente: `echo 0 | sudo tee /sys/class/rtc/rtc0/wakealarm`.
- **Un indirizzo nella lista non raggiunge comunque il NAS**: con `nft list table inet nas_launcher`
  controllare che il set lo contenga davvero.

## Aggiornamento del launcher

Dall'interfaccia (Impostazioni → Aggiornamenti) il launcher scarica il pacchetto della release, lo
verifica e avvia `nas-os-update.service`, che salva un backup, installa, controlla che il launcher
nuovo risponda e riavvia la macchina. Se qualcosa va storto rimette il backup e riavvia il launcher
di prima; l'esito compare come notifica al riavvio.

I backup sono in `/var/lib/nas-os/backups/` (gli ultimi tre, leggibili solo da root). Per tornare
indietro a mano a un backup:

```bash
sudo systemctl stop nas-os
sudo rm -f /var/lib/nas-os/launcher.db-wal /var/lib/nas-os/launcher.db-shm
sudo tar -xpzf /var/lib/nas-os/backups/<data>.tar.gz -C /
sudo systemctl daemon-reload && sudo systemctl start nas-os
```

## Rimozione

```bash
sudo /usr/local/lib/nas-os/uninstall.sh            # servizi, binario e permessi, conservando i dati
sudo /usr/local/lib/nas-os/uninstall.sh --purge    # anche database, dati delle app e utente, con conferma
```

La rimozione completa del NAS (token, gateway, launcher, firewall di base e pacchetti Debian della
configurazione) è l'unico file che il wizard lascia nella cartella del suo pacchetto:
`sudo ./uninstall.sh`.

## Antivirus opzionale

L'installer copia `nas-os-antivirus.service` e `57-nas-os-antivirus.rules`, senza abilitare
la unit né installare ClamAV. La pagina Antivirus del Security Center richiede i pacchetti
Debian `clamav` e `clamav-freshclam` tramite il gestore dei pacchetti delle app, solo quando
l'utente la abilita. La unit esegue il launcher con `--antivirus-job` e ricontrolla la richiesta;
le scansioni e le operazioni di quarantena non danno permessi di sistema all'app esterna.
FreshClam è abilitato per gli aggiornamenti delle firme mentre l'antivirus è acceso e viene
fermato e disabilitato quando l'utente lo spegne. FreshClam scarica le firme come utente
`clamav`, che il firewall di base non lascia uscire sul web: `nas-os-antivirus.service` e il
drop-in `clamav-freshclam.service.d/nas-os-firewall.conf`, copiato dall'installer, mettono il
cgroup della unit nel set `nas_apps_out` con `NFTSet=`, come le unit delle applicazioni.

La quarantena in `/var/lib/nas-os-antivirus` rimane privata di root e viene conservata anche
alla rimozione del launcher: non si perdono definitivamente file isolati in un'operazione di
manutenzione. Le eliminazioni definitive passano dalla conferma nella pagina Antivirus.

La unit opzionale `nas-os-antivirus-lifecycle.service` viene copiata dall'installer ma non
abilitata durante l'installazione. L'executor la abilita solo se l'utente configura scansioni
all'avvio o allo spegnimento. Il boot ID evita scansioni durante l'abilitazione e durante i
riavvii del launcher; la scansione finale si esegue solo mentre systemd è in stato `stopping`.
Avvio e spegnimento attendono al massimo 24 ore. Le due unit antivirus usano lo stesso flock
per non sovrapporre operazioni. I risultati restano disponibili per le notifiche dopo il
riavvio; quarantena e metadati restano privati di root. La unit è disabilitata anche durante
factory reset e disinstallazione. Le opzioni e l'ordinamento sono documentati nei manuali
ufficiali di [systemd](https://www.freedesktop.org/software/systemd/man/latest/systemd.service.html)
e [ClamAV](https://docs.clamav.net/manual/Usage/Scanning.html).
