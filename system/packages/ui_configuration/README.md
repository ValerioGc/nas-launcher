# Configurazione del launcher

Da eseguire dopo `../ui_installation/install.sh`. Configura ciò che il launcher non può fare da sé
perché richiede root, usando gli stessi helper che il launcher avvia, così ogni cosa si fa in un
modo solo. L'installazione lo copia anche in `/usr/local/lib/nas-os/configure.sh`, dove resta quando
il pacchetto non c'è più.

**Il wizard** lo esegue una volta, da root, alla fine della prima configurazione: il token sull'input
(un input vuoto non salva nessun token), poi l'utente del terminale con il nome dell'account, poi la
riconciliazione dei volumi dati di quell'utente.

```bash
printf '%s' "$TOKEN" | sudo ./install.sh --user <nome>
```

**A mano** serve per il **token** con cui il launcher legge le release del proprio repository
privato, `ValerioGc/nas-launcher`, per aggiornarsi. Di solito lo si inserisce nel wizard o nell'App
Manager; questo script serve a farlo dalla macchina.

```bash
sudo /usr/local/lib/nas-os/configure.sh            # chiede il token senza mostrarlo, lo cifra e riavvia il launcher
sudo /usr/local/lib/nas-os/configure.sh --clear    # lo toglie
```

| File         | Scopo                                                                              |
| ------------ | ---------------------------------------------------------------------------------- |
| `install.sh` | Esegue `launcher-token.sh` e, per il wizard, `terminal-user.sh` e `nas-os-storage` |

Gli helper veri stanno nel backend (`backend/axum/src/services/packages/launcher-token.sh` e
`backend/axum/src/apps/services/nas_system_terminal/terminal-user.sh`) e `ui_installation` li mette
in `/usr/local/lib/nas-os/`. Il token lo salva anche il launcher, tramite `nas-os-token.service`,
quando arriva dall'App Manager: così il token si salva in un modo solo.

## Dove finisce il token

In `/etc/credstore.encrypted/nas-os.launcher-token`, cifrato da `systemd-creds` con la chiave della
macchina e, dove c'è, con il TPM. Il file copiato su un'altra macchina non si decifra. La unit del
launcher lo importa con `ImportCredential=nas-os.*`: systemd lo decifra a ogni avvio e lo consegna
**solo** al servizio, in una cartella che nessun altro processo vede.

Il token **non sta nel database**, che finisce nelle copie di sicurezza, e l'interfaccia non lo
mostra mai: dice soltanto se c'è.

Un unico token GitHub deve poter leggere `ValerioGc/nas-launcher` e i repository privati elencati
nel catalogo (per esempio `ValerioGc/nas-picture-manager`), con il permesso _Contents: read-only_. Il
launcher lo usa per sé e per tutte le applicazioni, e il catalogo non ne dice niente. Il wizard lo
usa anche per scaricare gateway e launcher, quindi deve leggere anche `ValerioGc/nas-gateway` e
`ValerioGc/nas-wizard`.

Per verificare che funzioni: `sudo /usr/local/lib/nas-os/check.sh`. Nella sezione _Release token_
dice se il token si decifra, se il launcher l'ha ricevuto e se GitHub lo accetta per ogni
repository, distinguendo token rifiutato (401), permessi mancanti (403) e repository non visibile
o senza release (404).
