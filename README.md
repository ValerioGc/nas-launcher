# nas-os

Desktop web per un NAS custom su Debian 13, con Vue 3, TypeScript, Pinia e backend Rust Axum.
La UI comprende menu applicazioni, schede multiple, collegamenti desktop, notifiche e due widget
per sistema, processi e operazioni in corso. Le app di sistema restano separate dal codice della
shell.

## Stato attuale

**Quello che agisce davvero sulla macchina**: ognuno dietro il proprio interruttore in
`/etc/nas-os/nas-os.env` (in sviluppo `config/development.env`, che li tiene spenti), così nessuno
spegne il computer su cui sta programmando:

- **Alimentazione**: riavvio e arresto con `systemctl`, regole pianificate eseguite da un
  pianificatore nel backend, e sveglia dell'orologio hardware prima di uno spegnimento programmato
  (`NAS_POWER_MODE=real`). Una regola aspetta al massimo un'ora i task delle app in corso, mai un
  lavoro di root (aggiornamento, apt, dischi, pacchetti di un'app), che blocca anche il comando
  manuale con un 409.
- **Rete**: lettura del collegamento, ricerca delle reti e connessione, da NetworkManager; un
  collegamento alla volta (con il cavo niente Wi-Fi) e schede disattivabili, mai quella in uso
  (`NAS_NETWORK_MODE=real`).
- **Indirizzi ammessi**: un elenco di indirizzi e intervalli CIDR, applicato
  dal launcher e, come set `nftables`, davanti a tutto il NAS (`NAS_FIREWALL_MODE=real`).
- **Pacchetti delle applicazioni**: catalogo su file, app opzionali incluse nella build da
  `system/apps`, controllo delle release su GitHub o GitLab, scaricamento, verifica del checksum,
  scompattamento e aggiornamento (`NAS_PACKAGES_MODE=real`).
- **Aggiornamento del launcher**: lo stesso meccanismo scarica la release, una unit di root salva
  database e configurazione, installa, controlla che il launcher nuovo risponda e riavvia il NAS,
  tornando alla copia di sicurezza se qualcosa va storto. Il token del repository privato è una
  credenziale systemd cifrata, che l'interfaccia non chiede e non mostra.
- **Pacchetti di sistema**: elenco degli aggiornamenti Debian letto con `apt`, aggiornamento
  dell'indice e installazione dei pacchetti scelti da una unit di root (`NAS_APT_MODE=real`).
- **Terminale**: l’app esterna `nas-system-terminal` usa il servizio del launcher per aprire
  una console come utente locale creato dal wizard, senza `sudo`, e seguire il journal di sistema
  (`NAS_TERMINAL_MODE=real`).
- **Configurazione**: esportata in un file, cifrato nel browser se si sceglie una password per il
  file, e reimportata; entrambe dopo la password dell'account, verificata dal gateway.
- **Ripristino**: dopo la password dell'account, verificata dal gateway, una unit di root cancella
  i dati di launcher, gateway e applicazioni, l'utente del terminale e il token, e riavvia il wizard;
  i dischi dati restano intatti (`NAS_RESET_MODE=real`).
- **Telemetria**: processore, memoria, swap, carico, temperatura, processi e sensori sono raccolti
  dal backend dell’app esterna nas-system-monitor, che disegna anche il widget.

**Quello che il launcher ricorda**, in SQLite, e che sopravvive a un riavvio: impostazioni divise
per sezione, applicazioni installate con la loro versione, collegamenti sul desktop, registro delle
notifiche, indirizzi ammessi e registro delle operazioni in corso. In sviluppo il database è
`backend/data/launcher.db`. Account e sessioni stanno invece nel database del gateway, che è un
progetto a sé ([../nas-gateway](../nas-gateway/README.md)).

**L'interfaccia**: schede delle app con lo stato conservato al cambio vista; desktop con le app di
sistema già presenti alla prima accensione e icone gestite dal menu contestuale, anche con
`Shift+F10`; temi chiaro, scuro e di sistema, colore di rilievo a scelta e sei sfondi locali, con
le etichette del desktop che restano leggibili su qualunque di essi; cataloghi italiano e inglese;
notifiche filtrabili dal vivo, a comparsa sullo schermo quando arrivano; due widget a scomparsa dall'header: processi con quadranti animati,
e operazioni con avanzamento, pausa e arresto, divise fra in corso e completate.

**Le applicazioni esterne** si installano da una release GitHub o GitLab, si aggiornano e si
aprono: ognuna è un processo a sé su un socket unix, e il launcher la mette in un iframe sotto
`/apps/<id>/` dello stesso indirizzo, facendo da reverse proxy: nessuna configurazione di rete né
dei computer da cui si naviga. È il launcher a farle partire, con
una unit systemd sola per tutte (`nas-app@<id>.service`), e dentro il frame shell e applicazione si
parlano con `postMessage`: tema, colore di rilievo e lingua da una parte, notifiche, richieste di
aprire una finestra e lettura delle notifiche dall'altra. Una notifica porta al suo log dettagliato
in nas-log-manager. Il contratto completo è in [APPS.md](docs/development/external_apps/APPS.md).

**Come gira sul NAS**: il browser apre `http://<indirizzo del NAS>/` oppure HTTPS sulla porta 443, dove il gateway serve
l'interfaccia, gestisce l'accesso con l'account creato dalla configurazione iniziale, che tiene
con le sessioni in un database suo, e inoltra al launcher e alle applicazioni solo con una sessione
aperta, dicendo loro chi è collegato in `X-NAS-User`.
Il launcher gira come utente di servizio senza privilegi né capability, in ascolto solo su
`127.0.0.1`. Quello che richiede root (riavvio, filtro, aggiornamenti, avvio delle applicazioni)
lo fa una unit dedicata, che il launcher può solo avviare grazie a una regola polkit per ciascuna, e
ogni script di root ricontrolla quello che il launcher gli passa. Il resto è nella
[revisione di sicurezza](docs/development/SECURITY_REVIEW.md).
L'installazione attiva un firewall con ingresso e uscita chiusi per impostazione predefinita; SSH
resta accessibile dalla rete privata. Il terminale dell'interfaccia usa un utente locale creato dal
wizard, separato sia da `nas-os` sia dall'amministratore `nas`.
nas-file-manager mostra le partizioni dati supportate nell'area dedicata all'utente del wizard:
il servizio storage le rileva e monta automaticamente senza formattarle, e non espone le
partizioni del sistema o i dati preesistenti fuori dall'area utente. Vedere la
[guida di installazione](system/packages/ui_installation/README.md).

**HTTPS e mTLS**: il gateway ha un'autorità propria del NAS e un certificato per la macchina. Il
browser si fida dopo aver installato l'autorità, scaricabile da `/auth/ca.crt`. L'accesso solo con
certificato (mTLS) è facoltativo e si configura solo nel Centro sicurezza dopo l'accesso al NAS
con password. Da lì si crea e si installa il certificato del primo dispositivo, poi si attiva
mTLS dalla connessione HTTPS confermando la password. Ogni dispositivo ha il suo certificato, che si rinnova o si revoca
da solo; il gateway rifiuta l'attivazione se il browser in uso non presenta un certificato valido.

**Quello che è ancora finto**, e lo dice: la consegna delle notifiche per email e webhook; la
ricerca dei file: il provider nas-file-manager è disponibile come app esterna, ma richiede
installazione e avvio per rispondere. Lo stato completo è nella [roadmap](ROADMAP.md).

## Avvio

Servono Node `^22.18.0 || >=24.12.0`, pnpm e Rust stable. In locale si installa con pnpm; le
pipeline installano con `npm ci` da `package-lock.json`. `pnpm install` lo riallinea da solo
(`postinstall`, che non installa niente con npm); dopo `pnpm add`, `update` o `remove` va lanciato
`pnpm run lock:npm`, e `pnpm run lint` fallisce finché i due non corrispondono.

```bash
pnpm install
pnpm run backend:dev
```

In un secondo terminale avviare il gateway, che è un progetto a sé nella cartella accanto e tiene
account e sessioni in `../nas-gateway/data/gateway.db`:

```bash
cargo run --manifest-path ../nas-gateway/Cargo.toml
```

In un terzo terminale:

```bash
pnpm run dev
```

Vite inoltra `/api` a `http://127.0.0.1:3000` e `/auth` al gateway su `127.0.0.1:3003`. Il backend ascolta solo su loopback.
Per usare una porta diversa: `PORT=3001 pnpm run backend:dev` e
`NAS_API_TARGET=http://127.0.0.1:3001 pnpm run dev`.

## Installazione sul NAS

Sul NAS non si installa questo pacchetto a mano: lo fa il wizard, un progetto a sé
([../nas-wizard](../nas-wizard/README.md), release in `ValerioGc/nas-wizard`). L'utente scarica solo la
release del wizard, la estrae sulla macchina con Debian 13 ed esegue `sudo ./install.sh`: lo script
configura il sistema operativo e avvia il wizard, che dal browser raccoglie account, lingua, tema,
rete e token, poi scarica e installa il gateway e questo pacchetto all'ultima versione.

Ogni release di questo progetto pubblica `nas-os-<versione>-linux-x64.tar.gz`, con
`ui_installation` (launcher, interfaccia, helper, unit e regole polkit) e `ui_configuration`
(token, utente del terminale, volumi dati), e il suo `.sha256`. Lo scaricano il wizard e
l'aggiornamento del launcher, che cerca proprio questo nome.

Sulla macchina restano `/usr/local/lib/nas-os/check.sh` (verifica), `configure.sh` (token) e
`uninstall.sh` (rimozione del launcher). La rimozione completa del NAS è l'unico file che il
wizard lascia nella cartella del suo pacchetto: `sudo ./uninstall.sh`.

Il dettaglio dei file installati è in [ui_installation](system/packages/ui_installation/README.md),
la procedura completa nel manuale di installazione, che sta nel progetto del wizard.

## Verifiche

`pnpm run release:test` esegue tutto in ordine, compilazione del pacchetto compresa. Uno per uno:

```bash
pnpm run build
pnpm run test:unit
pnpm run test:component
pnpm run test:e2e
pnpm run format:check
pnpm run lint
cargo fmt --manifest-path backend/Cargo.toml --check
cargo clippy --manifest-path backend/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path backend/Cargo.toml
```

I test frontend rispecchiano la struttura del sorgente sotto `src/tests/`; quelli degli script di
installazione stanno in `src/tests/scripts/` e girano in un processo node vero, contro comandi di
sistema finti. I test Rust sono tutti sotto `backend/tests/`, con gli stessi percorsi dei moduli che verificano.

## Documentazione

- [Sviluppo e convenzioni](DEVELOPMENT.md)
- [Scrivere un'applicazione per questo NAS](docs/development/external_apps/APPS.md): requisiti, pacchetto e API esposte
- [API](docs/development/API.md) e lo schema [OpenAPI](docs/development/openapi.json)
- [File installati sul NAS](system/packages/ui_installation/README.md)
- [Token per gli aggiornamenti del launcher](system/packages/ui_configuration/README.md)
- [Gateway di autenticazione](../nas-gateway/README.md) e [wizard](../nas-wizard/README.md), progetti a sé
- [Manuale utente](docs/user_manual), in PDF nel pacchetto di rilascio
- [Roadmap](ROADMAP.md)
- [Diario del lavoro e verifiche](docs/development/IMPLEMENTATION.md)
- [Revisione di sicurezza](docs/development/SECURITY_REVIEW.md)
