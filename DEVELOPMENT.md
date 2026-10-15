# Sviluppo di nas-os

## Confini e struttura

```text
src/
  api/client.ts                  # HTTP, errori e segnale di mutazione riuscita
  apps/
    registry.ts                  # Identità UI e caricamento delle app interne
    AppHost.vue                  # Monta tutte le viste aperte e mostra quella attiva
    AppTabs.vue                  # Navigazione, desktop e chiusura delle viste
    AppView.vue                  # Superficie comune, senza scheda sovrapposta
    components/                  # Primitive comuni soltanto alle app
    styles/_apps.scss            # Stile comune delle app, sotto .app_surface
    <app>/                       # Componenti, tipi e store del singolo dominio
  components/                    # Launcher e controlli UI riutilizzabili (commons/: campo,
                                 #   password con l'occhio, segmented, spinner, modali…)
  components/setup/              # Il riquadro della pagina di preparazione
  views/                         # Pagine: login, console, preparazione del NAS
  composables/                   # HTTP lifecycle, debounce, polling e collegamento del tema
  utils/                         # Funzioni pure: ordinamento, contrasto, validazione dell'account,
                                 #   messaggio di una richiesta fallita
  i18n/launcher/, i18n/apps/      # Cataloghi IT/EN divisi per shell e applicazioni
  stores/                        # apps, shell, widgets, auth, setup: uno per mestiere
  styles/                        # Token, temi, placeholder, mixin e reset
  styles/themes/<tipo>/          # Un file per tema (`data-palette`), elencati in themes/_index.scss
  widgets/WidgetDrawer.vue       # Guscio comune dei widget: entrata, Esc, focus
  widgets/WidgetPanel.vue        # Intestazione delle operazioni: titolo, impostazioni del polling, chiusura
  widgets/monitor/               # Cassetto dei processi: la cornice del widget di nas-system-monitor
  widgets/tasks/                 # Widget delle operazioni in corso
  tests/                         # Stessi percorsi di src, suite component/unit/e2e
backend/
  axum/src/
    main.rs                      # Apre il database, poi avvia il server
    utils.rs                     # Helper comuni: is_real(Mode), now_ms, systemctl
    http/                        # Impalcatura HTTP: rotte, stato, errori, estrattori, confine
    controllers/                 # Shell: health, desktop, setup
    services/                    # Shell: database, settings, session, notifications, registry,
                                 #   desktop, manifest, lifecycle, pacchetti, fine del wizard e app
                                 #   preinstallate
    apps/controllers/<app>.rs    # Rotte e handler di un'applicazione
    apps/services/<app>.rs       # Logica, tipi e validazione della stessa applicazione; quelle
                                 #   grandi hanno una cartella (nas_system_settings: access, network,
                                 #   power, system_updates, updates)
    widgets/controllers/<w>.rs   # Rotte dei widget dell'header: operazioni, impostazioni
    widgets/services/<w>.rs      # Logica e tipi degli stessi widget
    **/*.sh                      # Helper eseguiti come root dalle unit, accanto al modulo che li
                                 #   avvia
  axum/migrations/               # Migrazioni SQL numerate, incluse nel binario
  data/                          # Database di sviluppo e degli e2e, ignorato da git
  tests/                         # Test Rust, stessi percorsi di axum/src
scripts/                         # Solo script locali e di pipeline: build, formattazione, rilascio
system/apps/                     # Archivi di applicazioni da preinstallare, inclusi nel pacchetto
system/packages/ui_installation/ # Installazione sul NAS: utente, permessi, unit systemd, catalogo
system/packages/ui_configuration/  # Token, utente del terminale e volumi: li esegue il wizard
docs/user_manual/                # Manuale utente, in PDF alla radice del pacchetto
public/wallpapers/               # Immagini SVG locali, senza richieste a terzi
```

Gateway e wizard sono progetti a sé, nelle cartelle accanto (`../nas-gateway`, `../nas-wizard`), ognuno con
il suo repository, la sua versione e la sua pipeline. Il wizard configura il sistema operativo,
scarica e installa gateway e launcher, e tiene il manuale di installazione. Il manuale utente,
che descrive l'interfaccia, è di questo progetto (`docs/user_manual/`).

I tipi specifici di un'app rimangono in `src/apps/<app>/types.ts`; quelli dei widget nella propria
cartella. Non creare un file globale per tipi di dominio. La shell conosce solo il contratto per
aprire le app. Il menu notifiche appartiene a `nas-notification-manager`, anche se viene montato
nell'header. Una nuova app non deve duplicare client HTTP, cornice o controlli.

## Interne ed esterne

Il contratto per chi scrive un'applicazione esterna (requisiti, pacchetto, release e API esposte)
sta in [APPS.md](docs/development/external_apps/APPS.md).

Un'app **interna** è compilata dentro il launcher, in entrambe le metà: i componenti nel bundle,
gli endpoint nel binario, e cambiarla significa rilasciare il launcher. Resta interna l'app senza cui la shell non funziona (gestore applicazioni, impostazioni, notifiche)
e la ricerca, che mostra i risultati dei fornitori invece di esserne uno.

La ricerca trova da sola applicazioni e voci delle impostazioni. I file li cercano le app che
dichiarano la capability `search`: il launcher ne dà l'elenco (`GET /api/capabilities/search`) e il
browser le chiama direttamente sul loro percorso. Il contratto sta in
[APPS.md](docs/development/external_apps/APPS.md).

### Il registro delle applicazioni

Le app integrate sono nel codice, in `services/registry.rs`, che è anche l'unico punto in cui si
interroga `installed_apps`: ci sono sempre, non si installano e non si rimuovono, perché sono la
shell. Tutto il resto è una riga di `installed_apps` nel database,
quindi un'app installata resta installata dopo un riavvio e dopo un aggiornamento del binario.
Quello che si può installare è il catalogo in
`/etc/nas-os/catalogue.json`, e ogni voce punta a una release da scaricare.

Il campo `app.system` del manifest e `system` del catalogo classificano anche le app esterne
come app di sistema, senza modificarne la categoria o trasformarle in app integrate.
Nel catalogo standard tutte le app esterne sono di sistema tranne Backup e Picture Manager.
Le altre hanno `preinstall` attivo e ricevono un collegamento durante la configurazione iniziale.
Il Terminale è l’app esterna `nas-system-terminal`, preinstallata dal catalogo standard.
Il launcher conserva in `services/terminal.rs` le sessioni, il journal e le impostazioni già salvate;
il backend dell’app raggiunge queste funzioni sul canale Unix delle applicazioni.

**Installata vuol dire due cose insieme**, e l'elenco è l'unione delle due. Il registro sa cosa ha
scritto un'installazione (nome e descrizione della voce di catalogo, versione della release) e la
cartella sa cosa c'è davvero da aprire. Un pacchetto scompattato a mano non ha riga e compare lo
stesso, ed è quello che permette di provare una build prima di pubblicarla; una riga la cui
cartella è sparita resta visibile, perché rimuoverla è l'unica cosa rimasta da fare. Dove le due
non concordano sulla versione decide la cartella: è quella che verrà aperta, ed è quella che il
controllo aggiornamenti confronta.

L'icona segue la stessa strada: sta nel pacchetto, e il launcher la serve da `/api/apps/{id}/icon`.
È un file scritto da qualcun altro servito sulla propria origine, quindi il percorso deve restare
dentro il pacchetto, l'estensione dev'essere di un formato immagine, la dimensione sotto il mezzo
mega, e la risposta porta `nosniff` più una policy che non lascia eseguire niente: un SVG dentro
un `img` non esegue nulla, uno aperto direttamente sì.

Nel backend vale la stessa separazione, e le cartelle sono quattro.

- **`http/`** è l'impalcatura: tabella delle rotte, stato condiviso, errore tipizzato, estrattore
  JSON e confine delle API. Nessuna logica di dominio.
- **`controllers/` e `services/`** sono la shell, ciò che non appartiene a nessuna applicazione:
  salute, registro delle app, collegamenti del desktop, database, impostazioni, notifiche,
  pacchetti e processi delle applicazioni.
- **`utils.rs`** raccoglie ciò che si ripete ovunque: `is_real(Mode::Power)` per gli interruttori
  `NAS_*_MODE`, `now_ms` e `systemctl`.
- **`apps/`** contiene un'applicazione per file, con le rotte in `controllers/<app>.rs` e la logica
  in `services/<app>.rs`, con lo stesso nome dell'app nel frontend in snake_case. Le impostazioni
  (`nas_system_settings`) tengono anche accesso, alimentazione e rete. L'accesso non è più una
  loro scheda: lo gestisce l'app esterna `nas-security-center`, che usa queste stesse rotte
  tramite la shell (`src/api/access.ts`), mentre il filtro resta nel launcher.
- **`widgets/`** contiene operazioni (il registro che usano anche pacchetti e applicazioni)
  e impostazioni comuni del polling. Il widget dei processi e la raccolta hardware appartengono
  a nas-system-monitor; il launcher ospita soltanto il suo iframe.

Ogni gruppo tiene il proprio stato in memoria: `SessionData` ha un campo `apps` (la release del launcher trovata e il lavoro
di apt in corso). Un'app usa i servizi del launcher, per esempio `notifications` per registrare un evento,
ma non importa mai i moduli di un'altra app. Una nuova app aggiunge il proprio router in
`apps::controllers::routes`, un nuovo widget in `widgets::controllers::routes`.

I file Rust si chiamano in `snake_case`: un modulo `activeTasksWidget.rs` fa fallire
`clippy -D warnings`.

## Azioni sulla macchina

`apps/services/nas_system_settings/power.rs` esegue riavvio e spegnimento con `systemctl`, quindi con logind. Tre regole:

- **Nessuna shell e nessun argomento costruito con una stringa:** programma fisso, verbo da un enum.
- **Nessun privilegio nel processo:** il launcher gira come utente di servizio e i permessi arrivano
  dalla regola polkit installata da [system/packages/ui_installation/](system/packages/ui_installation/), limitata a riavvio e
  spegnimento.
- **Di norma non spegne niente:** l'azione vera richiede `NAS_POWER_MODE=real`; altrove resta
  simulata, e la risposta dice quale dei due casi è avvenuto.

### Il ripristino

`apps/services/nas_system_settings/reset.rs` riporta il NAS a dove l'aveva trovato il wizard. Il
launcher chiede soltanto: avvia `nas-os-reset.service` (regola polkit 75, solo il verbo `start` di
quella unit) con `--no-block`, perché la unit ferma il launcher stesso, e lo script
`reset/factory-reset.sh` fa il lavoro da root:

- disattiva e ferma il gateway, ferma le applicazioni e il launcher;
- cancella l'utente del terminale con la sua home, solo se è quello che ha creato
  `ui_configuration` (il commento `nas-os terminal`);
- cancella gli stati del launcher, del gateway (account e sessioni), delle applicazioni e del
  wizard, e il token delle release;
- rimuove entrambe le tabelle firewall del NAS e il controllo Wi-Fi, ripristina nome del computer,
  rete, DNS, Samba e stato dei servizi dalla copia iniziale protetta del wizard; gli aggiornamenti
  dei pacchetti Debian restano installati;
- avvia `nas-wizard.service`, che lo script di avvio del wizard ha lasciato sulla macchina.

Smonta soltanto i mount introdotti dal NAS; i file sui dischi dati restano intatti. Senza wizard
eseguibile e relativa unit non cancella niente (`noWizard`). Senza la copia iniziale restituisce
`noOriginalState`: non registra lo stato di un NAS già configurato come se fosse quello originale.
`GET /api/system/reset` restituisce anche `disabled` dove `NAS_RESET_MODE` non è `real`. Lo script
root verifica l'integrità della copia prima delle modifiche. Il reset rifiuta aggiornamenti,
installazioni di pacchetti e operazioni sui dischi in corso. Il browser può disconnettersi quando
tornano nome e indirizzo originali; il wizard si raggiunge allora all'indirizzo ripristinato.

La password dell'account la controlla il gateway: la pagina la manda a `POST /auth/confirm`, che
risponde con un codice valido per una sola richiesta entro un minuto, e `POST /api/system/reset` lo
porta in `X-NAS-Confirmation`. Il gateway lo toglie e inoltra la richiesta con
`X-NAS-Reauthenticated`; il launcher rifiuta con 403 la richiesta che arriva senza (l'estrattore
`http::extract::Reauthenticated`), e il gateway toglie quell'intestazione da ogni richiesta del
browser.

### Esportare e importare la configurazione

`apps/services/nas_system_settings/config.rs` dà e riceve la configurazione in chiaro
(`GET` e `POST /api/system/config`), entrambe con la stessa conferma della password del ripristino.
Contiene le impostazioni generali senza il nome del NAS, l'accesso e gli indirizzi ammessi, la
pianificazione dell'alimentazione, l'aggiornamento dei widget e il desktop; mai rete, nome, account
o token. In più, sotto il loro identificatore, le impostazioni delle applicazioni scelte nella
finestra dell'export (`GET /api/system/config?apps=<id>,<id>`): quelle che ne hanno una sul NAS
sono in `config::EXPORTABLE` e `GET /api/system/config/apps` le elenca. Oggi sono il terminale, le
notifiche e l'App Manager, che porta anche il catalogo salvato e l'elenco delle applicazioni
installate. Le altre applicazioni non hanno ancora impostazioni salvate sul NAS (le regole del
Backup non vengono salvate, le preferenze di Explorer stanno nel browser) e nella finestra restano
disattivate.

L'import controlla tutto come i salvataggi delle singole sezioni prima di scrivere, poi sostituisce
tutto in una transazione e applica indirizzi ammessi e pianificazione. Del desktop tiene le cartelle
e le applicazioni presenti o in arrivo. Con l'App Manager nel file installa le applicazioni che
mancano (dal catalogo; una che non c'è lo dice con una notifica) e rimuove, lasciandone i dati,
quelle che il file non ha, mai le applicazioni di sistema né quelle con `preinstall` nel catalogo:
solo con `NAS_PACKAGES_MODE=real`. Un file di un formato più nuovo è rifiutato; uno della versione
1, senza desktop e applicazioni, lascia entrambi come sono.

Il file lo scrive il browser (`src/apps/nas-system-settings/configFile.ts`), con la regola degli
accessi del gateway (`/auth/policy`) accanto alla configurazione. La password del file è
facoltativa e diversa da quella dell'account, che serve solo alla conferma: senza, il file è JSON
in chiaro; con, è cifrato con WebCrypto (PBKDF2 SHA-256 e AES-GCM, nessuna dipendenza) e si apre
solo con quella. All'import la pagina riconosce il file cifrato e chiede la sua password. Il
launcher non vede mai nessuna delle due.

### La pianificazione

`power/schedule.rs` è un compito avviato da `main.rs` accanto al server. Non è un timer di
systemd e non ne installa: le regole stanno nel database, e cambiarle non deve significare scrivere
file di sistema.

Il prossimo orario non lo calcola: trasforma la regola in un'espressione di calendario
(`Mon..Fri *-*-* 07:30:00`) e chiede a `systemd-analyze calendar` quando scatta. Il giro è: leggi le
regole, guarda il primo evento; se cade entro il minuto aspetta il secondo giusto, altrimenti torna
a dormire. Il salvataggio delle regole interrompe l'attesa (`schedule::rules_changed()`), così una
regola salvata a meno di un minuto dal proprio orario non viene persa.

L'**accensione non si esegue**: a macchina spenta non gira niente. Si programma prima di spegnere,
in `power/wake.rs`, che scrive l'orario accanto al database e avvia
`nas-os-wake.service`. Quella unit gira come root perché `/sys/class/rtc/rtc0/wakealarm` lo
scrive solo root, e il launcher non è root né deve diventarlo: la regola polkit gli consente di
avviare quella unit e nient'altro. Se manca, lo spegnimento avviene lo stesso e una notifica dice
cosa è mancato.

## La rete

`nas_system_settings/network/machine.rs` legge come la macchina è collegata adesso, elenca le reti attorno e si
collega a una di esse. Leggere non chiede permessi; **cambiare** rete accade solo con
`NAS_NETWORK_MODE=real`.

Gli strumenti sono quelli che Debian ha già (`ip -json`, sysfs, `nmcli` dove c'è NetworkManager,
`/proc/net/wireless` dove non c'è) e ognuno è facoltativo: se manca, il campo resta vuoto invece
di far fallire la lettura. Il punto di partenza è la rotta predefinita: è quella che porta il NAS
in rete, e senza di lei non c'è niente da mostrare.

L'elenco delle reti viene da `nmcli`, che in modalità `-t` separa con `:` ed esce con `\\` i due
punti dentro un valore: un SSID può contenerli, quindi le righe si dividono a mano e non con uno
`split`. La stessa rete si vede spesso due volte (due bande, due punti di accesso) e resta una
voce sola, con la lettura più forte. Dove la rete non è leggibile la rotta risponde 503 e la scheda
mostra l'errore con il suo bottone di riprova.

## Impostazioni che fanno qualcosa

Non tutte le preferenze hanno un effetto, e chi legge il codice deve poterlo sapere senza provarle:

- **tema, lingua, colore di rilievo e sfondo** li applica il launcher stesso (`useAppearance`,
  `@/i18n`). Il colore di rilievo finisce su `--accent`, quindi lo usano anche le app;
- **«crea collegamento»** lo legge il backend **dentro l'installazione**
  (`packages::run_install` → `desktop::shortcut_on_install` → `desktop::add_app`), dopo la
  registrazione di una nuova applicazione. Gli aggiornamenti mantengono i collegamenti esistenti
  e non ricreano quelli rimossi; il gestore applicazioni rilegge l'elenco al termine;
- **i secondi di aggiornamento dei widget** guidano davvero `usePolling`;
- **le regole di alimentazione** le esegue il pianificatore del backend. Prima di agire
  `power::busy` legge i task attivi e le unità di root in esecuzione (`nas-os-update`,
  `nas-os-apt@`, `nas-os-storage-*`, `nas-os-app-packages@`): `schedule::wait_for_work` aspetta i
  task al massimo un'ora, i lavori di root sempre, e `power::run` risponde 409 finché uno gira.
  `GET /system/power/busy` dà lo stesso elenco alla conferma di Riavvia e Spegni;
- **la rete** è tutta vera, dalla lettura al collegamento;
- **gli indirizzi ammessi** valgono in due punti: una richiesta fuori lista riceve 403 dal
  middleware del launcher, e con `NAS_FIREWALL_MODE=real` la lista diventa un set `nftables`
  davanti a tutto il NAS;
- **`notifyFailures`** lo legge `access::record_failure`: un accesso rifiutato lascia una notifica
  con indirizzo, campo sbagliato e orario;
- **la consegna delle notifiche** non esiste: il webhook si può ancora configurare, ma il pannello
  che lo modifica è disattivato invece di lasciar credere che spedisca;
- **gli aggiornamenti di sistema** sono quelli di apt, veri, e con `NAS_APT_MODE=real` si
  installano: vedi [Gli aggiornamenti di sistema](#gli-aggiornamenti-di-sistema).

## Fuori dai motori di ricerca

Niente di quello che il NAS serve deve finire in un indice, pagina di accesso compresa:

- `public/robots.txt` chiede a tutti di non visitare niente (`Disallow: /`);
- `index.html` (l'unica pagina: login e launcher sono rotte della stessa) porta
  `<meta name="robots" content="noindex, nofollow, noarchive, nosnippet, noimageindex, notranslate">`
  e `<meta name="referrer" content="no-referrer">`, perché l'indirizzo del NAS non esca nel Referer
  verso un sito esterno, che è il modo in cui un motore di ricerca lo scoprirebbe;
- il backend manda `X-Robots-Tag` con le stesse direttive su ogni risposta, applicazioni esterne
  comprese (`http/boundary.rs`, `no_index`), e il gateway anche sui file statici (`../nas-gateway/src/ui.rs`).

## Chi può raggiungere il NAS

`nas_system_settings/access.rs` tiene l'elenco degli indirizzi ammessi: indirizzo esatto o intervallo CIDR.
**Un elenco vuoto non aggiunge restrizioni** (la lista si accende con la prima regola) e l'anello
locale passa sempre. Il firewall di base, che installa il wizard, continua a bloccare le porte non
previste anche quando la lista è vuota.

Vale in due punti. Il middleware in `http/guard.rs` copre
ciò che parla direttamente con il launcher. Il set `nftables` copre **tutto il
resto del NAS**, e ci arriva come la sveglia dell'orologio: il launcher scrive la lista accanto al
database e avvia `nas-os-firewall.service`, che gira come root perché scrivere un firewall
richiede root e il launcher non lo è. Lo script di root rilegge la lista come se l'avesse scritta
chiunque (nel ruleset arrivano solo caratteri di un indirizzo) e sostituisce la tabella in una
transazione sola, così un ruleset rifiutato lascia in piedi quello di prima invece di nessun filtro.
Il dettaglio sta in `system/packages/ui_installation/README.md`.

Il launcher ascolta solo su `127.0.0.1`, quindi il suo middleware vede soltanto l'anello locale, che
passa sempre: davanti alla rete agiscono il firewall di base e la lista `nftables`.

### Rilevamento in rete (NAS Finder)

NAS Finder trova il NAS con una richiesta UDP sulla porta 39877, a cui rispondono il wizard e poi il
gateway. Il firewall di base (nas-wizard) la lascia entrare solo dalla rete locale IPv4 e solo
mentre l'insieme `nas_discovery` contiene la porta: il wizard ce la mette finché risponde e lo
svuota al passaggio al gateway. Da lì decide l'opzione di Security Center, spenta di default
(`GET`/`PUT /settings/discovery`, sezione `discovery`): `nas_system_settings/discovery.rs` scrive
`on` in `discovery` accanto al database, o toglie il file, e avvia `nas-os-discovery.service`
(regola polkit 63), che con `apply-discovery.sh` mette la porta fissa nell'insieme o lo svuota.
All'avvio il launcher la riapre se l'opzione è accesa, perché dopo un riavvio l'insieme è vuoto;
se è spenta non tocca nulla, così non chiude la porta mentre il wizard risponde ancora. La fine
della prima configurazione (`setup/complete`) registra la notifica `discoveryOff`, che dice dove
riaccenderla. Con `NAS_FIREWALL_MODE` diverso da `real` l'opzione viene solo salvata.

## I pacchetti delle applicazioni

Installare un'applicazione è l'unica cosa per cui il NAS esce su internet, e la forma è la stessa
di tutto il resto che tocca la macchina: **non fa niente senza `NAS_PACKAGES_MODE=real`**, non
compone comandi con le stringhe, e ciò che dura minuti finisce nel registro delle operazioni invece
di tenere aperta una richiesta HTTP.

**Il catalogo è un file**: `/etc/nas-os/catalogue.json` dice id, nome, forge, repository e nome
dell'asset. Lo scrive l'installazione, lo modifica solo root, e un file assente è un catalogo
vuoto: non un errore. Si rilegge a ogni richiesta, quindi una modifica vale senza riavvii.

Il giro di `services/packages.rs` è: chiedi al forge qual è l'ultima release, scarica l'asset,
**confrontalo con il `.sha256` pubblicato accanto**, scompattalo con `tar` in una cartella a parte,
e solo allora rinominala sopra quella che c'era. Un download che si interrompe a metà lascia la
copia funzionante esattamente dov'era, e la precedente resta da parte finché il rename non è
riuscito. Una release senza checksum si installa con un avviso nel registro, non in silenzio.

Il **token** per le release private è unico e condiviso con l'aggiornamento del launcher:
accompagna ogni richiesta a GitHub o GitLab, e mai un indirizzo di download fuori dalle loro origini
(`forge::for_forge`). `system/packages/ui_configuration/install.sh` lo cifra nel credential store di
systemd; la UI mostra solo se è configurato.

Un pacchetto è l'archivio di qualcun altro. `tar` rifiuta nomi assoluti e `..`, ma un collegamento
simbolico dentro il pacchetto resta possibile: per questo l'icona si risolve nel suo percorso reale
e si rifiuta se esce dal pacchetto (`nas_app_manager::inside`), altrimenti `icon.png -> ../../launcher.db`
servirebbe il database.

Il **launcher aggiorna il NAS**, sé stesso, il **gateway** e il **wizard**, con la stessa macchina
e insieme (`apps/services/nas_system_settings/updates.rs`). La verifica chiede l'ultima release
di ogni componente installato: il launcher a `NAS_LAUNCHER_REPOSITORY`, gateway e wizard a
`NAS_GATEWAY_REPOSITORY` e `NAS_WIZARD_REPOSITORY`, o in mancanza ai repository `nas-gateway` e
`nas-wizard` dello stesso proprietario. La versione installata la dicono `nas-os-gateway --version`
e `/usr/local/lib/nas-wizard/wizard/bin/nas-wizard --version`; un gateway che non conosce
`--version` ha versione sconosciuta e accetta qualunque release. Un solo pulsante installa tutti
gli aggiornamenti trovati: il launcher scarica ogni pacchetto, lo verifica (e qui un checksum
mancante è un rifiuto, non un avviso), lo lascia nella cartella di stato come
`<componente>-update.tar` e avvia `nas-os-update.service`: un servizio in esecuzione non può
sostituire sé stesso.

`install-launcher-update.sh`, come root, fa il resto in quest'ordine: scompatta i pacchetti, ferma
il launcher (e il gateway, se si aggiorna) perché i database su disco siano coerenti, salva in
`/var/lib/nas-os/backups/` un unico backup di database, `/etc/nas-os`, binari, interfaccia, helper,
unit e regole polkit, più gateway e wizard quando si aggiornano. Poi installa ogni componente con
l'**installer del suo pacchetto**: il wizard con `install.sh --update` (riconfigura il sistema e
mette al suo posto il wizard senza avviarlo), il gateway con `gateway_installation/install.sh` (che
lo riavvia, e si aspetta che risponda su `/health`), il launcher con `ui_installation/install.sh`.
Aspetta che il launcher risponda e **riavvia la macchina** se il launcher è cambiato, altrimenti
riavvia solo il launcher. Se un passo fallisce rimette tutto il backup (togliendo prima il WAL del
database nuovo, che non deve finire in quello vecchio) e riavvia launcher e gateway precedenti,
senza riavviare la macchina: l'aggiornamento è tutto o niente. Restano gli ultimi tre backup.

Il **ripristino di fabbrica** scarica prima il wizard più recente, se ce n'è uno
(`reset::prepare_wizard`), e `factory-reset.sh` lo installa con `install.sh --update` prima di
cancellare qualunque cosa; se l'aggiornamento non riesce, il ripristino continua con il wizard
installato.

Com'è finita lo scrive in `launcher-update.status`, accanto al database (`done` seguito dai
componenti installati), e il launcher lo trasforma in una notifica e cancella il file
(`updates::report_previous`): aggiornato, annullato con il motivo, rifiutato, o interrotto a metà
da uno spegnimento. Il nuovo launcher parte già
durante la fase di installazione, avviato dal suo installer: finché `nas-os-update.service` è
attivo aspetta la fine invece di prendere quella fase per un'interruzione, che segnala solo quando
non c'è più nessun aggiornamento in corso. Una verifica che non trova versioni nuove non lascia
notifiche.

Nell'interfaccia l'aggiornamento parte da una finestra che prima chiede conferma (il NAS si
riavvierà) e poi non si chiude più finché lavora (`LauncherUpdateModal`): segue l'operazione nel
registro (scaricamento, verifica, installazione), poi aspetta che il launcher sparisca e torni. Se
torna con la versione nuova ricarica la pagina; se torna con quella di prima dice che l'aggiornamento
è stato annullato.

Il repository del launcher, `ValerioGc/nas-launcher`, è privato, e il **token** è una **credenziale
systemd cifrata**, in `/etc/credstore.encrypted/nas-os.launcher-token`, scritta da
`system/packages/ui_configuration/`.
systemd la decifra a ogni avvio con la chiave della macchina (e il TPM, dove c'è) e la consegna
solo al servizio (`ImportCredential=nas-os.*`), che la legge da `$CREDENTIALS_DIRECTORY`
(`credentials::launcher`). L'interfaccia dice soltanto se c'è. In sviluppo, senza systemd
(`NAS_TOKEN_MODE` spento), `NAS_LAUNCHER_TOKEN_PATH` indica un file.

## Gli aggiornamenti di sistema

`apps/services/nas_system_settings/system_updates.rs` legge gli aggiornamenti Debian con
`apt list --upgradable`, che non chiede privilegi: la lista è vera anche su una copia di sviluppo, ed
è quella del computer su cui gira.

Aggiornare l'indice e installare chiedono root, e prendono la strada di tutto il resto che tocca la
macchina: il launcher lascia la selezione accanto al database (`apt-upgrade`, un nome per riga) e
avvia `nas-os-apt@refresh.service` o `nas-os-apt@upgrade.service`, che eseguono `apt-updates.sh`
come root. La regola polkit concede quelle due istanze e il solo `start`; senza
`NAS_APT_MODE=real` il launcher non avvia niente e la scheda dice perché.

Ogni nome passa due controlli: il launcher accetta solo pacchetti che apt offre in quel momento, e lo
script scarta tutto ciò che non ha la forma di un nome Debian, perché il file è comunque un file.
`--only-upgrade` impedisce di installare qualcosa di nuovo, `--force-confold` di sovrascrivere un file
di configurazione cambiato a mano.

Un lavoro alla volta: finché gira, la sessione lo ricorda (`apps.apt_job`), la lista risponde con
`running` e la scheda la rilegge ogni tre secondi, sbiadita e bloccata, finché non finisce. Il lavoro
sta anche nel registro delle operazioni: una riuscita lascia una notifica, un errore la lascia da sé.

## Come si apre un'applicazione esterna

Un'applicazione installata è un processo a sé che ascolta su un socket unix, e il launcher le sta
davanti come reverse proxy **per percorso**: `/apps/<id>/…` sull'indirizzo stesso del launcher.
`http/proxy.rs` toglie il prefisso, lo passa all'applicazione in `X-Forwarded-Prefix` e va su
`/run/nas-os/apps/<id>.sock`; un redirect verso un percorso dell'applicazione (`/login`) torna sotto
il prefisso, e `/apps/<id>` senza la barra finale è rediretto a `/apps/<id>/`.

Tutto quello che raggiunge il NAS raggiunge così le sue applicazioni, anche con una sola porta
inoltrata, senza DNS, nomi host o porte in più. Il prezzo è l'origine condivisa: il JavaScript di
un'applicazione gira sulla stessa origine della shell e potrebbe usarne la sessione, quindi si
installano solo applicazioni di cui ci si fida. Un'applicazione usa indirizzi relativi
(`base: './'` in Vite, `fetch('api/…')`): uno assoluto, `/api/…`, arriverebbe al launcher.

**Il percorso del socket si deriva dall'identificatore, non si legge dal pacchetto**, altrimenti un
pacchetto potrebbe puntare il proxy su `/run/dbus/system_bus_socket`; una differenza col manifest
finisce nel log. Allo stesso modo l'`id` del manifest deve essere quello della cartella.

Il connettore è scritto a mano su `tokio::net::UnixStream` e `hyper_util::rt::TokioIo`. Quello che
fa, oltre a inoltrare: toglie le intestazioni hop-by-hop nei due versi,
**scrive** `X-NAS-User` (il nome passato dal gateway se valido, altrimenti `local`), rifiuta con 403
tutto ciò che sta sotto `/internal/`, porta fino al socket un upgrade WebSocket (e il gateway fa lo
stesso verso il launcher) e risponde 501 a ogni altro upgrade. Sulla risposta toglie ogni
`Set-Cookie` con un `Domain`. Le applicazioni condividono i cookie dell'host con il launcher e fra
loro: il gateway toglie il cookie di sessione dalle richieste verso un'applicazione e scarta ogni
`Set-Cookie` che provi a sostituirlo.

In `http/routes.rs` il proxy sta **fuori** dal limite di corpo e dal confine dell'API, e dentro il
controllo degli indirizzi ammessi, che resta il più esterno di tutti.

Dal lato shell, `resolveApp` dà a un'applicazione sconosciuta un `AppEntry` esterno e `AppHost` apre
un iframe senza `sandbox` sull'indirizzo calcolato da `externalUrl`, `/apps/<id>/`: sulla stessa
origine della console un sandbox che esegue script e tiene l'origine non è una barriera e fa solo
avvisare il browser. Le sue regole (niente finestre proprie né dialoghi, niente navigazione della
pagina che lo contiene) le applica ogni applicazione con il suo `frameGuard.ts`, come chiede
[APPS.md](docs/development/external_apps/APPS.md). Una risposta che non è un successo (il processo non è avviato,
il launcher risponde 503 al suo posto) fa comparire un avviso con la via per avviarla.

In sviluppo Vite inoltra `/apps` al backend come `/api`, e l'applicazione va avviata a mano sul
socket giusto. La cartella dei socket, come quella dei pacchetti, in una build di debug sta accanto
al database (`backend/data/sockets`); `NAS_APP_SOCKETS` la sposta, tenendo presente che un percorso
di socket unix non può superare i 108 byte. Gli e2e avviano `src/tests/e2e/fixtures/app.mjs`, un'applicazione finta su un
socket.

### Chi fa partire il processo

Il launcher, ma non di persona: non è root e non scrive unit. Sulla macchina c'è **un template
solo**, `nas-app@.service`, e ogni applicazione è una sua istanza: `nas-app@fileExplorer.service`.
Il launcher chiama `systemctl start|stop|restart` su quel nome, e una regola polkit glielo concede
per quella forma di unit e per quei tre verbi soltanto, niente `enable`.

Il template non sa cosa eseguire, e nemmeno deve: `run-app.sh` legge `run.exec` dal manifest
dell'applicazione e lo esegue passandogli `--listen <socket>`. Un percorso assoluto o che esce dal
pacchetto viene rifiutato lì.

L'applicazione gira con `DynamicUser=yes` (un utente che systemd inventa e che non possiede niente
sulla macchina) con il gruppo del launcher fra i supplementari. Il socket lo crea lei: la cartella
è setgid, quindi il socket nasce con il gruppo `nas-os`, e `UMask=0007` lo rende apribile dal
launcher; è anche sticky (`3770`), quindi un'applicazione non può togliere il socket di un'altra e
rispondere al suo posto. Il launcher rimette quel modo a ogni avvio.

Il processo di un'applicazione **non ha l'anello locale** (`IPAddressDeny=localhost`, tranne il
resolver `127.0.0.53`): il launcher su `127.0.0.1:3000` risponde a chiunque lo raggiunga, perché
davanti c'è il gateway. Per notifiche e operazioni l'applicazione ha un socket suo,
`/run/nas-os/launcher/apps.sock` (`NAS_LAUNCHER_SOCKET`, `http/app_channel.rs`), in una cartella
che può aprire ma non scrivere, così nessuna applicazione ne occupa il nome mentre il launcher si
riavvia. Il launcher riconosce chi chiama dal cgroup del processo (`SO_PEERCRED`, poi
`/proc/<pid>/cgroup`): è quello di `nas-app@<id>.service`, che un processo non può cambiare da sé;
qualunque altro processo riceve 403. Dietro il socket ci sono solo le rotte del contratto
(`controllers/app_channel.rs`): le letture (tra cui `GET /api/system/inotify`, il limite dei watch
inotify per utente, che dalla sandbox un'app non legge), le notifiche e le operazioni, scritte soltanto con il
proprio `id` come `source`, e un'operazione si cambia o si toglie solo se l'ha registrata la stessa
applicazione.

Un'applicazione che lavora senza la sua finestra (i backup all'orario, le mail del NAS) dichiara
`run.background` nel manifest e dice da sé, con `PUT /api/apps/self/background {needed}` sullo
stesso socket, se ha lavoro da fare: il flag sta nella colonna `background` di `installed_apps`
(`services/background.rs`). All'avvio il launcher, dopo i grants, accende quelle con il flag; la
chiusura della finestra (`POST /apps/<id>/close`) le lascia accese, le altre le ferma come prima.
La dichiarazione diventa anche la riga `run:background` dei grants, che abbassa la priorità
dell'unità (`Nice=10`, `CPUWeight=50`, I/O `best-effort` 7).

Il template nasconde a ogni applicazione i dischi dati (`/srv`, `/mnt`, `/media`), i log, lo stato
del launcher (database compreso) tranne il pacchetto dell'app, `/sys`, `/run/udev` e i processi
degli altri. Il resto lo chiede il manifest in `permissions.paths`, e `services/grants.rs` lo
concede senza domandare solo da un elenco fisso: `/srv/storage`, e in sola lettura `/proc` intero,
`/sys` e `/run/udev`. Un pacchetto che chiede altro non si installa e non lascia niente. Il
concesso finisce in `app-grants/<id>` accanto al database, e `nas-os-app-grants@<id>.service`
(regola polkit 55, solo `start`) lo traduce come root in `nas-app@<id>.service.d/grants.conf` con
`apply-app-grants.sh`, che conosce lo stesso elenco e decide cosa diventa ogni percorso. Si applica
all'installazione, prima di mettere il pacchetto al suo posto, si toglie alla disinstallazione, e a
ogni avvio il launcher riallinea le app installate e riavvia quelle in esecuzione i cui permessi
sono cambiati.

`permissions.network` può chiedere `smtp` e `sync`: il grant diventa la riga `network:<tipo>` e il
drop-in aggiunge l'app all'insieme `nas_apps_smtp` (porte 25, 465 e 587) o `nas_apps_sync` (rete
locale sulle porte di Syncthing) del firewall di base (nas-wizard). Un'app con `sync` dice sul suo
socket se ha una sync attiva (`PUT /api/apps/self/sync`): `services/sync_ports.rs` scrive
`sync-ports/<id>` e avvia `nas-os-sync-ports.service` (regola polkit 62), che con
`apply-sync-ports.sh` mette le porte fisse nell'insieme `nas_sync` o lo svuota; all'avvio del
launcher e dopo l'installazione di un'app che sincronizza lo riallinea, alla disinstallazione lo
chiude. Le dichiarazioni sono serializzate e riapplicate anche se non cambiano, per ritentare un
helper fallito; la sync le rinnova ogni 30 secondi. In ingresso e uscita TCP è limitato a 22000,
UDP a 22000 e 21027, sempre sulla LAN. `permissions.packages` usa l'elenco fisso in
`services/system_packages.rs`, che comprende `msmtp`, `7zip`, `samba` e `syncthing`; il servizio
scrive `app-packages/<id>` e avvia `nas-os-app-packages@<id>.service` (regola polkit 56), che con
`app-packages.sh` installa i pacchetti prima che l'app prenda il suo posto e, alla disinstallazione,
toglie quelli che aveva installato lui e che nessun'altra app chiede più.

`services/mail_events.rs` consegna all'app che dichiara la capability `mail-events` (oggi
nas-mail-client) gli accessi rifiutati, gli aggiornamenti del launcher e gli aggiornamenti di
sistema, con un `POST` sul suo socket a un endpoint sotto `/internal/`, avviandola se è ferma.
Finché quell'app è installata, ogni sei ore controlla l'aggiornamento del launcher e quelli di apt,
catalogati per livello (`security`, `stable`, `other`) dall'archivio da cui arrivano.

`services/cleanup.rs` esegue le pulizie che un'app con `permissions.cleanup` chiede sul suo socket
(`POST /api/apps/self/cleanup`): valida la richiesta (azione, una voce dell'elenco fisso, giorni dei
temporanei e dimensione del journal), la scrive in `cleanup.request.json` accanto al database e
avvia `nas-os-cleanup.service` (regola polkit 59), che con `nas-os-axum --cleanup-job`
(`services/cleanup/executor.rs`) la riesegue come root su percorsi e comandi fissi e scrive la
risposta in `cleanup.result.json`. Una pulizia alla volta; fuori dal NAS (`NAS_APPS_MODE` non `real`)
risponde 503. Ogni voce ha un'anteprima che non cancella nulla, e la pulizia misura prima e dopo.
Non segue link, non attraversa altri filesystem, lascia sempre l'ultima copia degli aggiornamenti,
il kernel in uso, il precedente e quelli più recenti, e i file di `/tmp` dei programmi in esecuzione.

`services/caches.rs` raccoglie le cache delle app che dichiarano la capability `cache` (un
endpoint sotto `/internal/`, letto e svuotato sul socket dell'app, avviandola se è ferma) e ci
aggiunge l'archivio delle scansioni antivirus, di cui il job scrive file e byte nel report perché
la sua cartella è privata di root; svuotarlo è `forget_scanned`. `controllers/caches.rs` le espone
con `GET /api/caches` e `DELETE /api/caches/{app}/{id}`.

`controllers/mail.rs` aggiunge `GET /api/mail/status`, che la sezione **Account** delle
impostazioni usa per sapere se l'app Email può inviare, e `POST /internal/mail/code`, fuori da
`/api`: il gateway lo chiama sul loopback per mandare i codici dell'account (verifica
dell'indirizzo di recupero e reset della password) e non lo inoltra mai dal browser. Nome,
password ed email di recupero vivono nel gateway (`/auth/account`, `/auth/recovery`): la sezione
Account e il recupero della pagina di accesso (`composables/usePasswordRecovery.ts`) li chiamano
con `src/api/account.ts`. Il recupero via email è un interruttore dell'account, spento di
predefinito (`PUT /auth/account/recovery`); la pagina di accesso mostra "Password dimenticata?" solo
se `GET /auth/recovery` dice che è acceso con un indirizzo verificato.

Explorer e Backup chiedono `/srv/storage`: ricevono il gruppo `nas-storage`, `BindPaths` dei
dischi e la mappa `/etc/nas-os/storage-volumes.tsv` (`NAS_STORAGE_ROOT`,
`NAS_STORAGE_REGISTRY`). Il servizio root
`nas-os-storage.service`, attivato dal timer e da udev, esclude i filesystem di sistema,
monta le partizioni dati tramite UUID e collega soltanto `.nas-users/<utente del wizard>`
a `/srv/storage/v-<uuid>`. Il backend valida la mappa e verifica che i bind mount siano
ancora attivi prima di servire le API. Ricerca, selettore file e collegamenti usano gli
stessi percorsi logici. Il terminale non usa ancora questo spazio come root della sessione.

Le unit del launcher e delle applicazioni non hanno nessuna capability, né orologio, log del kernel
o hostname da cambiare, e solo le famiglie di socket che usano. `systemd-analyze security` le
misura senza installarle (`--offline=yes`): 1.7 su 10 il launcher, 1.5 le applicazioni e 1.1
l'unità che ne scrive i permessi.

Come tutto il resto che tocca la macchina, vale solo con `NAS_APPS_MODE=real`: senza, il launcher
non avvia niente, dice `503` con il nome della variabile, e l'interfaccia non disegna nemmeno il
bottone (`appsManaged` in `/apps/credentials`). Un'installazione fa `restart` alla fine e una
disinstallazione fa `stop` prima di togliere i file.

### Il dialogo dentro il frame

Un'applicazione sta sulla stessa origine della shell, quindi il browser non le impedirebbe di
chiamare l'API del launcher: non deve farlo, e usa i propri indirizzi relativi. Il canale previsto
fra le due è uno solo, `postMessage`, in `composables/useAppBridge.ts`.

Ogni messaggio porta `nas: 1`, perché nella finestra di una pagina finisce di tutto. In uscita il
launcher scrive sempre l'origine dell'applicazione, mai `*`. In entrata un messaggio conta solo se
arriva **dall'origine di un'applicazione aperta e dalla finestra di quel frame**: così
un'applicazione non parla per un'altra, e nemmeno lo fa qualcos'altro che posti in quella pagina.

Il launcher manda il contesto (tema risolto, colore di rilievo, lingua) al caricamento del frame,
quando l'applicazione dice `ready` e ogni volta che una delle tre cose cambia. L'applicazione può
chiedere una notifica, che il launcher **firma con il frame da cui è arrivata**, la descrizione
delle notifiche (`describe`) e l'apertura di un'altra applicazione installata, anche su una pagina
precisa (`?notification=12`). Solo nas-log-manager può chiedere le notifiche di avviso ed errore
(`diagnostics`), il log dietro una di esse (`diagnostic-log`) e la sua cancellazione
(`remove-diagnostic`): la shell le esegue con la sessione della console e risponde con
`<richiesta>-result`, così l'applicazione non ha bisogno dell'API del launcher.

## Le operazioni in corso

`widgets/services/tasks.rs` è un **registro, non un pianificatore**. Il launcher non esegue il lavoro:
un'applicazione che copia, indicizza o scarica registra la propria operazione, ne riporta
l'avanzamento mentre va e la chiude alla fine. Il widget legge il registro e lo mostra in due
schede, **In corso** (predefinita) e **Completate**, ognuna dalla più recente alla più vecchia per
data di creazione.

Chiedere una pausa **scrive lo stato**, e il proprietario, che rilegge le proprie righe, lo
rispetta. Chi non può essere messo in pausa lo dichiara quando registra, e l'interfaccia non disegna
quel bottone. Senza avanzamento la barra si disegna senza posizione. Un'operazione conclusa non si
riapre, e una finita male lascia anche una notifica. L'installatore dei pacchetti registra così ogni
installazione e ogni aggiornamento.

## Cosa sta facendo la macchina

Il backend di nas-system-monitor raccoglie processore, memoria, processi e sensori da `/proc`
e `/sys` e serve `api/telemetry` all’app e al widget. Il launcher non mantiene campioni né tipi
hardware: la rotta storica `/api/system/telemetry` inoltra al socket dell’app per compatibilità.
`services/process_control.rs` e il suo helper di root restano nel launcher come confine delle
operazioni privilegiate, con controllo di PID, istante di avvio e protezione dei servizi essenziali.
La terminazione di un processo di `nas-app@<id>.service` arresta l’intero servizio, compresi i
processi figli, e verifica che sia fermo. La risposta identifica l’app tramite il cgroup letto dal
backend. Il bridge condiviso da app e widget rimuove quindi la scheda e smonta l’iframe, usando
la stessa rimozione della chiusura normale senza inviare un secondo arresto. Un errore mantiene
la scheda aperta; un riavvio o la terminazione di un processo esterno alle app non la rimuovono.
`MonitorDrawer.vue` ospita l’iframe del widget senza disegnare dati o gestire preferenze dell’app.

## Stato e chiamate

Un errore della pagina stessa (l'elenco delle app o le impostazioni che non arrivano) non può
diventare una notifica del registro, perché è proprio il backend a non rispondere: `MainPage` lo
mostra come toast con il suo bottone di riprova, dallo stesso componente che mostra le notifiche
nuove (`NotificationToasts`), e lo toglie appena l'errore sparisce.

`request<T>()` usa `fetch`, ha un timeout di 10 secondi e genera `nas:mutation` dopo una scrittura
riuscita. Il launcher usa il segnale per aggiornare il riepilogo notifiche; non contiene le logiche
dell'app notifiche.

Quello che si rilegge da sé passa da `usePolling`: niente mentre la pagina è nascosta, e una lettura
subito quando torna visibile, invece di mostrare per un intervallo intero quello che c'era prima.
L'header rilegge ogni quindici secondi solo le notifiche **non lette**, che sono tutto quello che
mostra: il registro intero, fino a 500 righe, lo legge solo il gestore notifiche. Le app gestiscono caricamento, errore, tentativo successivo e azioni tramite
`useResource`. Le risposte obsolete vengono ignorate; i dati già caricati restano visibili in caso
di errore. Il frontend non deve inventare risultati positivi quando il backend non risponde.

Il menu contestuale è lo stesso in tre punti (menu applicazioni, gestore applicazioni e desktop)
e passa da `useContextMenu`, che tiene posizione e scelta. Offre sempre **l'azione che manca**:
aggiungi al desktop se l'icona non c'è, rimuovi il collegamento se c'è.

`appsStore` gestisce l'elenco delle app e gli ID delle schede aperte. `AppHost` conserva le istanze
con `v-show`: il cambio scheda non perde le bozze, la chiusura smonta la vista. Le modali sono
indipendenti dalla scheda attiva. La configurazione generale ha uno store nell'app settings;
il form usa una bozza e applica al sistema solo la risposta del salvataggio.

## Persistenza

Il launcher salva i propri dati in **un solo file SQLite**, e il gateway nel suo: solo il launcher
apre `launcher.db`, e quello che il sistema possiede già (SSH, log, dischi) non si copia.

### Dove va ogni dato

| Dato                                           | Dove                                            |
| ---------------------------------------------- | ----------------------------------------------- |
| Notifiche, con pulizia dei più vecchi          | SQLite                                          |
| Preferenze UI (`general`) e canali di notifica | SQLite, una riga JSON per utente e chiave       |
| Collegamenti del desktop, con posizione        | SQLite                                          |
| Regole di alimentazione                        | SQLite, nel documento della sezione             |
| `notify_failures`                              | SQLite                                          |
| Indirizzi ammessi all'accesso                  | SQLite, più un set nftables generato da lì      |
| SSH attivo e porta                             | Fuori dal launcher: si amministra dalla console |
| App installate                                 | Registro SQLite più il pacchetto in `apps/`     |
| Catalogo delle app installabili                | File JSON in `/etc/nas-os/catalogue.json`       |
| Token delle release                            | Credential cifrata di systemd                   |
| Log                                            | journald                                        |
| Processi, metriche, temperatura                | Letti da `/proc` e `/sys` al momento            |
| Dischi e aggiornamenti                         | Letti dal sistema al momento                    |
| Operazioni in corso                            | SQLite, 200 righe e un giorno di storico        |
| Account, hash Argon2id e sessioni              | SQLite del gateway, `gateway.db`                |

### Configurazione

`services/database.rs` apre il database all'avvio. Senza database il launcher non parte, e lo dice
nel journal. Il percorso si sceglie in quest'ordine:

1. `NAS_DB_PATH`, se impostato;
2. `$STATE_DIRECTORY/launcher.db`, la cartella che systemd crea con `StateDirectory=nas-os`
   (in produzione: `/var/lib/nas-os/launcher.db`);
3. solo in una build di debug, `backend/data/launcher.db`. Una build release senza percorso non
   parte.

Gli e2e usano `backend/data/e2e.db`, impostato in `playwright.config.ts`. Gli scenari condividono quel database e
girano in parallelo; quelli che cambiano ciò che gli altri leggono (la lingua, l'intera sezione
delle impostazioni generali, il catalogo, lo storico delle operazioni) sono in `SHARED` e girano dopo tutti gli altri, uno alla
volta. Uno scenario nuovo che scrive dati letti da altri va aggiunto lì.

La connessione usa `journal_mode=WAL` (le letture non aspettano le scritture),
`synchronous=FULL` (una transazione confermata sopravvive a un'interruzione di corrente),
`foreign_keys=ON` e `busy_timeout` di 5 secondi. Cartella e file nascono con permessi `0700` e
`0600`, prima che SQLite li apra: i file `-wal` e `-shm` ereditano i permessi del database. Allo
spegnimento, con Ctrl+C o con il SIGTERM di systemd, il pool viene chiuso e il WAL riversato nel
file principale. `/health` risponde `503` se il database non risponde.

Nella unit systemd: `StateDirectory=nas-os`, `StateDirectoryMode=0710` (il gruppo attraversa la
cartella per raggiungere i pacchetti delle applicazioni, senza elencarla), `ProtectSystem=strict` e
`UMask=0077`.

Le variabili del launcher (porta, interruttori `NAS_*_MODE`, repository delle release) stanno in
due file di [config/](config/):

- `production.env` diventa `/etc/nas-os/nas-os.env` sul NAS: il pacchetto lo copia in
  `files/nas-os.env` e `install.sh` lo installa a ogni installazione e aggiornamento. Le unità del
  launcher e del gateway lo leggono con `EnvironmentFile=`, insieme a `/etc/nas-os/nas-os.local.env`,
  facoltativo, che scrive l'amministratore, che nessuna installazione tocca e che ha la precedenza.
- `development.env` lo carica `pnpm run backend:dev` dalla shell: ogni interruttore è spento, e una
  variabile già impostata sulla riga di comando vince (`PORT=3001 pnpm run backend:dev`).

Una variabile nuova va in entrambi i file. Gli e2e non li usano: impostano le loro in
`playwright.config.ts`.

Ciò che agisce sulla macchina vale solo con il suo interruttore a `real`; spento, la funzione
risponde 503 o resta simulata e lo dice. In sviluppo se ne accende uno per un solo processo, per
esempio `NAS_APT_MODE=real pnpm run backend:dev`.

| Interruttore              | Cosa accende                                         | Cosa serve inoltre sul NAS                                           |
| ------------------------- | ---------------------------------------------------- | -------------------------------------------------------------------- |
| `NAS_POWER_MODE=real`     | riavvio, spegnimento, accensione programmata         | regola polkit 10, unit `nas-os-wake`, wake on RTC nel BIOS           |
| `NAS_NETWORK_MODE=real`   | scansione e connessione di rete                      | NetworkManager, regola polkit 20                                     |
| `NAS_FIREWALL_MODE=real`  | filtro `nftables` degli indirizzi ammessi            | nftables dal wizard, unit `nas-os-firewall`, polkit 30               |
| `NAS_PACKAGES_MODE=real`  | pacchetti delle app e aggiornamento del launcher     | rete verso la forge; per il launcher unit `nas-os-update`, polkit 40 |
| `NAS_APPS_MODE=real`      | avvio e arresto delle applicazioni                   | template `nas-app@.service`, regola polkit 50                        |
| `NAS_APT_MODE=real`       | installazione degli aggiornamenti Debian             | template `nas-os-apt@.service`, regola polkit 60                     |
| `NAS_TERMINAL_MODE=real`  | console e journal del terminale                      | -                                                                    |
| `NAS_TOKEN_MODE=real`     | token nelle credenziali cifrate di systemd           | `ui_configuration/install.sh`; in sviluppo `NAS_LAUNCHER_TOKEN_PATH` |
| `NAS_RESET_MODE=real`     | ripristino del NAS e nuovo avvio del wizard          | unit `nas-os-reset`, regola polkit 75, unit `nas-wizard`             |
| `NAS_PROCESSES_MODE=real` | termina e riavvia dai processi del widget            | template `nas-os-process@.service`, regola polkit 65                 |
| `NAS_LAUNCHER_REPOSITORY` | controllo delle versioni del launcher (`owner/nome`) | -                                                                    |

`launcher.db`, `launcher.db-wal` e `launcher.db-shm` sono un database solo: non si copia mai il
file `.db` da solo a servizio acceso. La copia coerente si fa con `VACUUM INTO '/percorso/copia.db'`,
ed è quella che il launcher tiene da sé (`database::keep_copy`): all'avvio e poi una volta al giorno
scrive `copies/launcher.db` accanto al database, leggibile solo da lui, scritta a parte e rinominata
così da non essere mai a metà. Una copia di sicurezza prende quella, non i file vivi.

### Le impostazioni, sezione per sezione

La tabella `settings` tiene **una riga per sezione**, non un unico documento: `general`,
`notifications`, `access`, `power`. Ogni riga ha la posizione della sezione nell'ordine
dell'interfaccia, il documento JSON e il momento dell'ultimo salvataggio. Una sezione si salva
tutta insieme o non si salva: un valore rifiutato non lascia né documento né notifica.

`services/settings.rs` è l'archivio del launcher e conosce **solo** i nomi delle sezioni e il loro
ordine, mai il contenuto di un documento. La forma di una sezione appartiene all'app che la
possiede, con i suoi tipi e la sua validazione: `nas-system-settings` per generali, accesso e
alimentazione, `nas-notification-manager` per i canali di consegna, `services/terminal.rs` per l'aspetto
della console esterna, `nas-app-manager` per il controllo automatico degli aggiornamenti e per il catalogo
modificato dall'interfaccia, che quando esiste prende il posto di `/etc/nas-os/catalogue.json`. Così una sezione cambia forma senza
toccare l'archivio, e un'app non legge i tipi di un'altra.

Quando il launcher ha bisogno di un singolo campo di una sezione altrui, ne legge solo quello con
un tipo parziale: `desktop.rs` fa così per `createShortcut`, che decide se un'app appena installata
finisce anche sul desktop, e ignora il resto del documento.

Una sezione mai salvata risponde con i suoi valori di default, quindi il NAS appena installato
funziona con la tabella vuota. Un documento illeggibile, per esempio dopo una migrazione sbagliata,
non blocca l'interfaccia: si torna ai default e l'errore finisce nel log.

Il launcher non legge ancora `X-NAS-User`: tutte le righe appartengono all'utente `local`.

La sezione `access` contiene solo `notifyFailures`.

### Migrazioni

Le regole sono in [backend/axum/migrations/README.md](backend/axum/migrations/README.md). In breve:
file numerati, inclusi nel binario, applicati all'avvio; una migrazione applicata non si modifica
più (il checksum non tornerebbe e il backend non partirebbe); solo in avanti, con una copia del
database prima di aggiornare; tabelle `STRICT` con `CHECK` e date in millisecondi `INTEGER`.

`build.rs` ricompila il crate quando cambia la cartella delle migrazioni.

### Regole per il codice

- **Tutto l'SQL sta in un livello repository.** I service chiamano metodi come
  `notifications.list(filter)`, non scrivono query.
- **Solo query parametrizzate** (`bind`), mai SQL composto con stringhe.
- `query_as` con controllo a runtime, per ora: `query!` richiede un database o una cache `.sqlx`
  anche in CI.
- Un handler chiede solo la parte di stato che usa: `State<SqlitePool>` o `State<Session>`, mai
  l'intero `AppState`.
- **Un corpo JSON si estrae con `ValidJson`**, non con `Json`: conserva la spiegazione di serde
  come errore tipizzato, per esempio `theme: unknown variant \`viola\`, expected one of \`light\`,
  \`dark\`, \`system\``. Con `Json` il confine delle API la ridurrebbe a "invalid request".
- **Valori a scelta fissa come enum Rust**, non stringhe validate a mano: tema, lingua, modalità e
  motivo dello sfondo, giorno e azione di una regola di alimentazione, tono di una notifica. La
  validazione a mano resta dove il tipo non basta: intervalli numerici, colore, ora del giorno, nomi
  degli sfondi presenti.
- **Preferenze salvate in JSON:** `deny_unknown_fields` protegge l'input dell'API, ma togliere o
  rinominare un campo renderebbe illeggibili le righe già salvate. Serve una migrazione che
  aggiorni i documenti con `json_remove`.
- **Ricerca testuale:** `LIKE` ignora maiuscole e minuscole solo per l'ASCII, quindi "Unità" non
  trova "unità". Si confronta in Rust oppure si usa FTS5.

### Ispezionare il database

```bash
sudo apt install sqlite3
sqlite3 backend/data/launcher.db
sqlite> .tables
sqlite> .schema
sqlite> SELECT version, description, success FROM _sqlx_migrations;
```

In sviluppo si riparte da zero cancellando `backend/data/`, a backend fermo.

## Stile

Vue Composition API, TypeScript e nomi CSS composti con `_`. Token in `_variables.scss`, colori
semantici in `theme.scss`, primitive in `_placeholders.scss` e `_mixins.scss`. `global.scss` è
iniettato tramite `vite.config.common.ts`, condiviso da sviluppo, build e test.

`src/apps/styles/_apps.scss` è importato una sola volta da `main.scss`. La classe `.app_surface`
limita lo stile a contenuti app/widget e modali. I componenti definiscono solo layout specifici.
Le primitive `%button`, `%control`, `%panel`, `%stack`, `%icon_button` estendono il design system
esistente e usano `focus-ring`. Nessun framework CSS aggiuntivo.

Il CSS passa da PostCSS (`vite.config.common.ts`): selettori duplicati uniti, classi inutilizzate
tolte con PurgeCSS, prefissi dei browser con `autoprefixer` e compressione con `cssnano`. I browser
supportati sono un elenco solo, `browserslist` in `package.json`, letto da `autoprefixer` e da
`cssnano`: è lo stesso obiettivo predefinito di Vite (`baseline-widely-available`, Chrome ed Edge
111, Firefox 114, Safari 16.4), così il minificatore della build non toglie i prefissi aggiunti.

Il tema salvato può essere `light`, `dark` o `system`. In quest'ultimo caso si segue anche il cambio
del sistema operativo a runtime. App e widget ereditano i colori da `data-theme` sulla radice. Il
login mantiene uno stile autonomo.

I loghi hanno una palette distinta per app; `CommonIcon` visualizza il colore SVG, mantenendo
nome accessibile, dimensioni e comportamento decorativo. Niente font o icone scaricati dalla rete.

### La formattazione degli SFC

I `.vue` non li formatta prettier dalla riga di comando: sono in `.prettierignore` e passano da
[scripts/format-sfc-spacing.mjs](scripts/format-sfc-spacing.mjs), che chiama prettier attraverso la
sua API e poi aggiunge una riga vuota subito dopo l'apertura e prima della chiusura di `<script>` e
`<style>`, e una riga vuota fra due regole CSS attaccate.

`pnpm run format` fa tutto nell'ordine giusto (prettier, `eslint --fix`, poi lo script, che ha
l'ultima parola); `pnpm run format:sfc` fa solo gli SFC e `pnpm run format:check` controlla entrambi
senza scrivere. **Nell'editor** l'estensione prettier di VS Code non interviene sui `.vue`: si
formatta con `pnpm run format`.

## La prima configurazione

La configurazione iniziale è del wizard, un progetto a sé (`../nas-wizard`) che gira come root prima
che il launcher serva chiunque. Raccoglie account, nome del NAS, lingua, tema, rete e token, poi:

1. installa gateway e launcher;
2. esegue `ui_configuration`, che salva il token, crea l'utente del terminale e riconcilia i volumi
   dati;
3. crea l'account nel database del gateway;
4. chiama `POST /api/setup/complete` sul loopback, senza passare dal gateway, con lingua, tema e
   nome del NAS (`nasName`, facoltativo).

Il launcher salva lingua e tema e, se c'è, il nome, controllato come dalle impostazioni generali e
applicato come nome host (`apply_hostname`, con `NAS_HOSTNAME_MODE=real`); poi scrive la riga della tabella `setup` (`services/setup.rs`) e
pianifica i passi che restano a lui: le applicazioni che il catalogo marca con `preinstall: true`
(`services/provisioning.rs`). I passi sono righe di `setup_steps`, e a ogni avvio il launcher
riprende quelli rimasti. Una seconda chiamata a `complete` riceve **409**.

Le installazioni iniziali usano `packages::install_for_setup`: mantengono operazioni, log e stato
dei passi, senza notifiche di installazione o errore. I problemi compaiono nella pagina di
preparazione. Le installazioni richieste dall'App Manager mantengono le proprie notifiche.

Finché la configurazione non è completa, o i passi non sono finiti, il router apre `/preparing`
(`views/PreparingView.vue`) al posto di qualunque altra pagina, login compreso. La pagina chiede
`GET /api/setup` ogni due secondi e va all'accesso quando tutti i passi sono riusciti; se qualcosa è
fallito lo mostra e lascia l'accesso a un bottone. Se il wizard non ha ancora configurato il NAS lo
dice e continua ad aspettare. `GET /api/setup` passa dal gateway senza sessione; `complete` no.

## Il gateway sul NAS

Sul NAS l'ingresso dal browser è il gateway (`nas-os-gateway.service`, porta 80), e il launcher
resta su `127.0.0.1:3000`. Il gateway, un progetto a sé in `../nas-gateway`, fa tre cose (`src/entry.rs`):

- `/apps/*` va al launcher solo con una sessione valida, altrimenti **401**, e senza il cookie di
  sessione, che l'applicazione non riceve e non può sostituire;
- `/api/*` va al launcher solo con una sessione valida, altrimenti **401**, che riporta al login;
- il resto è l'interfaccia compilata (`GATEWAY_UI_DIR`): un file della cartella oppure, per un
  percorso senza estensione, `index.html`, che disegna la rotta. I file sotto `assets/` hanno un
  hash nel nome e si tengono in cache un anno, `index.html` si richiede ogni volta, così un
  aggiornamento si vede al caricamento successivo. Nessuna pagina si può incorniciare
  (`frame-ancestors 'none'`).

All'arresto il gateway smette di accettare connessioni ma porta a termine quelle in corso, così la
risposta a un riavvio o a uno spegnimento arriva al browser. Il proxy è scritto a mano su `hyper`.
In sviluppo il gateway ascolta su `127.0.0.1:3003` senza cartella dell'interfaccia, e Vite gli
manda solo `/auth`.

**L'accesso** è del gateway, che tiene account e sessioni in un database suo
(`src/store.rs`, migrazioni in `migrations/`; in sviluppo `../nas-gateway/data/gateway.db`,
sul NAS nella cartella di stato della unit):

- la password è un hash Argon2id (`src/auth/accounts.rs`), e un nome inesistente costa
  quanto una password sbagliata perché viene confrontato con un hash fittizio;
- credenziali sbagliate sono un 401; dopo i rifiuti che la regola ammette lo stesso indirizzo
  riceve 429 per i minuti scelti (`src/auth/throttle.rs`). La regola (`auth/policy.rs`,
  attiva o no, tentativi, minuti) sta nel database del gateway e si cambia dalla scheda Accesso
  delle impostazioni con `/auth/policy`;
- la sessione (`src/auth/sessions.rs`) è una riga con lo SHA-256 di un token casuale, che il
  browser tiene nel cookie opaco HttpOnly `nas_session`: scade dopo 30 minuti senza richieste o 12
  ore dopo l'accesso, e sopravvive a un riavvio del gateway.

Il launcher non conosce account né password: l'account lo crea il wizard direttamente nel database
del gateway (`nas-os-gateway account create`), e senza account qualsiasi accesso riceve 401.

Il router verifica `/auth/session` prima di aprire la console, e un 401 delle API riporta a
`/login`. La disconnessione chiama la rotta del gateway (`src/api/gateway.ts`), costante di build
`NAS_GATEWAY_LOGOUT` in `vite.config.common.ts`, predefinita `/auth/logout`. La connessione è HTTP in
chiaro e il certificato del client un mock: la rete fidata, il firewall di base e l'elenco degli
indirizzi ammessi in `nftables` sono le protezioni disponibili.

## Lingua

Importare `t` da `@/i18n` e usare chiavi dei cataloghi per testo UI. Usare `formatDate` per le date.
Il cambio lingua aggiorna anche `html.lang`. Nessun file usa il trattino lungo: lo controlla
`src/tests/scripts/repository.spec.ts`.

I cataloghi sono divisi come il codice: `i18n/launcher/` per la shell (header, desktop, menu,
finestre, widget) e `i18n/apps/<app>/` per le applicazioni. Una chiave usata da più aree resta nel
catalogo del launcher, che fa da comune. `index.ts` li unisce in un catalogo piatto per lingua,
quindi `t('chiave')` non cambia e non sa da quale file arriva il testo.

Due regole, controllate da `src/tests/i18n/`: **ogni file ha il suo gemello nell'altra lingua con
le stesse chiavi**, e **una chiave sta in un file solo**.
I contenuti ricevuti dal backend (nomi dei processi, log, titoli delle notifiche, percorsi) sono dati,
non chiavi di traduzione. Il login usa gli stessi cataloghi italiano e inglese del launcher.

## Test e qualità

I file `src/tests/**/*.unit.spec.ts` provano moduli e store; `*.component.spec.ts` montano gli SFC.
`src/tests/scripts/` prova gli script di installazione e gli helper di root in un processo node
vero, contro comandi finti (`nft`, `systemctl`, l'orologio) messi per primi nel `PATH`: gira con
`pnpm run test:unit`. I percorsi rispecchiano il modulo provato. Fixture Vue sotto `src/tests/fixtures/`. Tutti i test sono
esclusi dalla build applicativa e dalla copertura dei sorgenti, e configurati come test in Sonar.

```bash
pnpm run test:unit
pnpm run test:component
pnpm run test:coverage
pnpm run build
pnpm run lint
pnpm run format:check
pnpm run test:e2e
```

La copertura Rust richiede due strumenti da installare una volta nell'ambiente di sviluppo:

```bash
cargo install cargo-llvm-cov --locked
rustup component add llvm-tools-preview
pnpm run coverage:rust
```

Il report viene scritto in `backend/coverage/lcov.info`. Anche `pnpm run test:sonar` usa questo
comando: `pnpm install` non installa gli strumenti Cargo.

Lo scanner usa Podman senza root: `--userns=keep-id:uid=1000,gid=1000` mappa l'utente locale
sull'utente dello scanner nel container. Può così leggere anche i file con permessi `0600` e
scrivere nel progetto senza cambiare proprietari o permessi dei file.

E2E: `src/tests/e2e/features` e `steps`, generati da `playwright-bdd`. Gli step comuni stanno in
`steps/base_page.ts` e nominano gli elementi per `data-testid`: `When I go to "/login"`,
`When I click on "login-submit"`, `When I fill "login-username" with "admin"`,
`Then "login-form" is visible` o `is not visible`, `Then "task_list" contains "Copia"`,
`Then the address ends with "/login"`. Uno step specifico serve solo per quello che questi non
dicono; gli attributi `data-testid`
restano in sviluppo e vengono rimossi in produzione. Gli scenari integrati avviano backend e Vite,
in modalità `e2e` senza watcher dei file, per non consumare il limite inotify del sistema,
con un database che parte vuoto (`backend/data/e2e.db`), e al posto del gateway
`src/tests/e2e/fixtures/gateway.mjs`, che ne ha il contratto (accesso, sessione, uscita e regola dei
tentativi) e conosce un solo account: il gateway vero si prova nel suo progetto.
Nessun test sta dentro un file sorgente Rust. Quelli di un modulo stanno in
`backend/tests/<stesso percorso>.rs` e il modulo li include con `#[cfg(test)] #[path = "…"] mod tests;`,
così vedono anche ciò che è privato; quelli che chiamano il router dall'esterno sono dichiarati nel
manifest come `[[test]]`.

`backend/tests/machine/` è un `[[test]]` a sé, `machine`: il launcher su una macchina tutta sua, in
una cartella temporanea, con ogni modalità `NAS_*_MODE` reale tranne l'alimentazione. Quello che il
launcher esegue come root (`systemctl`, `lsblk`, `smartctl`, `apt`, `nmcli` e gli altri di
`support.rs`) è un finto messo per primo nel `PATH`, che annota come è stato chiamato e risponde con
i file di `answers/`; all'avvio il test verifica che ogni comando sia davvero il finto. Le forge sono
un server locale che risponde come GitHub e GitLab, l'app Email un server sul suo socket. Così si
provano installazione dei pacchetti, aggiornamenti, unità di root, rotte e il binario stesso, senza
toccare la macchina: gli helper di root che montano, formattano o staccano un disco
(`reconcile`, `initialize_with_log`, `rename_reported`, `eject_reported`) non vengono mai eseguiti.
`NAS_STORAGE_STATE_DIR` sposta i file di richiesta di quelle unità, `/var/lib/nas-os` senza.

Il contratto delle API è [openapi.json](docs/development/openapi.json), generato con `utoipa`
dagli handler del launcher: path, metodo e corpi stanno nell'attributo `#[utoipa::path]` di ogni
handler, e il router li legge da lì. `pnpm run api:generate` riscrive lo schema e i tipi del
frontend (`src/types/api.ts`, con `openapi-typescript`); `cargo test`, `pnpm run test:unit` e la
pipeline (`pnpm run api:check`) falliscono finché non sono allineati.

Non aggiungere dipendenze per funzioni già coperte dal progetto. La revisione di sicurezza è in
[SECURITY_REVIEW.md](docs/development/SECURITY_REVIEW.md).

## Rilascio

Il rilascio si prova in locale e si pubblica in pipeline, e la pipeline di rilascio non esegue
test. I test girano invece su ogni pull request verso `main`, comprese quelle di Dependabot
(`.github/workflows/ci.yml`: unit, component, Rust ed e2e); un ruleset su `main` che li
richiede impedisce il merge finché non passano.

- `pnpm run release:test` è il cancello locale: riallinea `package-lock.json`, il lockfile con cui
  le pipeline installano (`npm ci`), rigenera lo schema OpenAPI e i suoi tipi, formatta, controlla formattazione e lint, esegue
  clippy e tutti i test (unit, component, e2e, Rust), poi compila interfaccia, binario di rilascio e
  pacchetto.
- `pnpm run release:merge -- --type=patch|minor|major --branch=main` sceglie la versione, esegue
  lo stesso cancello, scrive la versione in `package.json`, `Cargo.toml` e
  `sonar-project.properties`, fa commit, merge e tag, e spinge il tag. `--dry-run` dice solo quale
  versione uscirebbe.
- Il tag avvia `.github/workflows/build.yml`: verifica tag e versione, compila, costruisce il
  manuale utente e il pacchetto, pubblica la release (il pacchetto e il suo `.sha256`) e tiene solo le due più recenti. Le note della release sono la
  sezione della versione in `CHANGELOG.txt` o, se manca, i commit dal tag precedente.

Il binario di rilascio è compilato con `lto`, un'unità di codegen e senza simboli
(`[profile.release]` in `backend/Cargo.toml`): circa il 40% più piccolo, ed è quello che ogni
aggiornamento del launcher scarica.

Il pacchetto si chiama `nas-os-<versione>-linux-x64.tar.gz`, con il `.sha256` accanto: è il nome che
il launcher cerca quando aggiorna sé stesso e che il wizard scarica. Alla sua radice c'è il manuale
utente, `nas_os_user_manual_<versione>.pdf`, con la versione del launcher: il wizard lo copia accanto
al suo manuale di installazione.

Il manuale in PDF lo genera `md-to-pdf` (`pnpm run manual:build`) con un browser Chromium: quello
indicato da `PUPPETEER_EXECUTABLE_PATH`, altrimenti Brave, Chromium o Chrome se installati,
altrimenti quello che Puppeteer scarica da sé. Sui runner di GitHub la pipeline installa il Chrome
abbinato a Puppeteer, e con `CI` impostata il browser parte senza sandbox.

## Antivirus opzionale

`apps/services/antivirus/` contiene il client ClamAV del Security Center: impostazioni in SQLite,
operazioni serializzate, pianificazione e helper privilegiato. Le API sono `GET /api/antivirus`,
`PUT /api/antivirus/settings`, `POST /api/antivirus/enabled`, `POST /api/antivirus/scan` e
`POST /api/antivirus/quarantine/{quarantine|restore|delete}` e `POST /api/antivirus/scanned/forget`.
Le azioni rispondono 204 dopo la presa in carico; lo stato contiene fase, avanzamento di apt, ultimo
risultato e quarantena.

La scansione rapida, e quella personalizzata con `skipUnchanged`, è incrementale
(`antivirus/scan_cache.rs`): i file trovati puliti stanno in
`/var/lib/nas-os-antivirus/scan-cache.sqlite3` con percorso, device, inode, dimensione, mtime,
ctime e opzioni (archivi, PUA, dimensione massima). La visita non segue i link e applica le stesse
esclusioni di ClamAV; i file da controllare vanno a `clamscan --file-list` (i nomi con un a capo
come argomenti). Un "OK" vale solo se il ctime non è successivo all'inizio della scansione, un
"FOUND" toglie il file dall'elenco, e dopo 30 giorni un risultato non vale più. Un errore
dell'archivio non fa fallire la scansione: diventa intera. Il report ha `skipped` e `planned`, e
con `planned` lo stato dà la percentuale vera. `scanned/forget` cancella l'archivio.
Le richieste dell'iframe passano da `src/api/antivirus.ts` nel bridge del Security Center.

Il manifest della app resta senza pacchetti: solo l'abilitazione richiede `clamav` e
`clamav-freshclam` a `system_packages`. Apt espone lo stato tramite il proprio Status-Fd,
in `/var/lib/nas-os-app-packages/antivirus-install.progress`, senza percentuali inventate.
La unit `nas-os-antivirus.service` esegue `nas-os-axum --antivirus-job` su una richiesta fissa
in `/var/lib/nas-os/antivirus.request.json`; la regola polkit consente solo di avviarla.
Non si abilita la unit all'installazione e non si installano pacchetti all'avvio del launcher.
Il helper aggiorna le firme con FreshClam e controlla il suo servizio solo dopo l'abilitazione.

Il report pubblico è `/var/lib/nas-os/antivirus.result.json`; metadati e contenuti della
quarantena restano in `/var/lib/nas-os-antivirus`, privata di root. I nomi passati dall'utente
non diventano comandi o destinazioni di quarantena. I file sono individuati da ID del report
privato, verificati di nuovo da ClamAV e aperti senza seguire link. Le directory vengono
ancorate tramite fd per evitare sostituzioni dei percorsi durante le operazioni; il ripristino
usa create_new e non sovrascrive. La disabilitazione conserva firme e file isolati, ferma
FreshClam e impedisce l'avvio di nuovi job. Riavviare il launcher riconcilia il servizio con la
preferenza salvata e attende eventuali operazioni già in corso.

Il reset del NAS ferma anche la unit antivirus e FreshClam prima di cancellare lo stato del
launcher. I contenuti della quarantena vengono conservati, come durante la disinstallazione.

### Antivirus: lifecycle e coverage

Le nuove impostazioni `bootScan` e `shutdownScan` sono disabilitate di default e mantengono
compatibilità con i documenti precedenti. L'executor conserva una copia validata e privata
delle impostazioni, abilita la unit lifecycle solo se richiesta e usa il boot ID del kernel
per distinguere l'avvio dell'OS dall'abilitazione della funzione. Allo spegnimento controlla
`systemctl is-system-running`: fermare la unit durante un aggiornamento non esegue una scansione.
Il launcher viene fermato prima della scansione finale; filesystem e rete dopo. flock
serializza unit operative e lifecycle. I risultati lifecycle sono conservati e notificati
al successivo avvio del launcher. Factory reset e disinstallazione disabilitano la unit;
la quarantena viene conservata.

`pnpm run coverage:antivirus` esegue i test Rust, inclusa la macchina simulata, e impone
l'80% di righe e funzioni per ognuno dei cinque file del backend antivirus. Lo stesso controllo
è eseguito da `coverage:rust` sui report della pipeline, senza ripetere i test. La copertura UI
è nel repository Security Center, con l'80% per file anche su rami e istruzioni. La macchina
simulata richiede socket locali. Non viene installato né eseguito ClamAV reale nei test.
