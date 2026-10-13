# nas-os: roadmap

Cosa manca, e in che ordine costruirlo. Quello che è fatto sta nel diario,
[docs/development/IMPLEMENTATION.md](docs/development/IMPLEMENTATION.md); come funziona, in
[DEVELOPMENT.md](DEVELOPMENT.md) e nei contratti di [openapi.json](docs/development/openapi.json),
[API.md](docs/development/API.md) e [APPS.md](docs/development/external_apps/APPS.md).

Un passo è chiuso quando vale il suo **Fatto quando**, non quando il codice è scritto.

---

## Manca

| Area                     | Cosa fa oggi                                                                                      | Cosa manca                                                         |
| ------------------------ | ------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------ |
| Consegna email e webhook | indirizzo e webhook si configurano, il pannello è disattivato                                     | qualcuno che spedisca davvero                                      |
| Spazio dati del wizard   | Il reconciler prepara aree utente sui volumi dati e nas-file-manager usa solo i mount autorizzati | convalidare montaggio, hotplug e permessi con dischi in una VM NAS |
| Copie dei database       | launcher e gateway tengono `copies/*.db`, fatta a servizio acceso con `VACUUM INTO`               | includerle nei set del Backup manager, quando eseguirà i backup    |

Il launcher ha un solo utente, quello creato dal wizard: non sono previsti utenti multipli.
Per il terminale, una fase separata valuterà il login come `nas` con nome utente e password;
la sessione predefinita resta quella dell'utente Unix creato dal wizard. SSH resta fuori ambito.

---

# S: Sistema

## S2: Permessi di rete delle app

- [ ] Permessi di rete per singola app e porte diverse da HTTP e HTTPS, concessi dall'amministratore

**Fatto quando** una app usa solo le porte autorizzate per lei e la revoca del permesso le blocca,
senza interrompere `apt update` e il controllo degli aggiornamenti delle app.

## S3: Cifratura del disco (LUKS)

LUKS è l'unica cosa che protegge i dati se la macchina viene portata via. Va predisposta durante
l'installazione di Debian, perché cifrare dopo vuol dire riformattare.

- Il disco di sistema con il partizionamento guidato di Debian, "LVM cifrato"
- I dischi dei dati con `cryptsetup luksFormat`, aperti all'avvio con una chiave che sta sul
  sistema cifrato, così basta sbloccare il primo
- Lo sblocco all'avvio senza tastiera: un NAS senza schermo non può chiedere la passphrase

Punti da chiarire:

1. **Come si sblocca all'avvio.** Passphrase da HDMI a ogni riavvio, TPM2 con `systemd-cryptenroll`
   (protegge il disco estratto, non la macchina intera), Tang sulla rete di casa con Clevis (si
   apre solo in casa) o SSH nell'initramfs con `dropbear`.
2. **Quali credenziali servono.** Una passphrase di recupero da conservare fuori dal NAS, oltre
   all'account del launcher; e se il wizard debba mostrarla una volta sola.
3. **Dove si vede.** Lo stato della cifratura nel widget di sistema o nelle impostazioni, e la
   formattazione cifrata dei nuovi dischi in nas-disk-manager.
4. **Copia dell'header LUKS.** Senza, un header danneggiato rende i dati illeggibili anche con la
   passphrase.
5. **Prestazioni.** Su una CPU senza AES-NI la cifratura rallenta i trasferimenti.

**Fatto quando** un disco estratto dal NAS e collegato a un altro computer non mostra i dati senza
la passphrase.

# B: Rete e identità

Nel gateway, un progetto a sé in `../nas-gateway`. HTTPS con l'autorità propria del NAS e mTLS
facoltativo ci sono già; i dispositivi si gestiscono dal Centro sicurezza.

## B2: Il launcher su un socket unix

- [ ] Il launcher smette di ascoltare su una porta e passa a un socket unix, raggiungibile solo dal
      gateway

**Fatto quando** il launcher non ha più porte TCP aperte e la shell risponde come prima.

---
