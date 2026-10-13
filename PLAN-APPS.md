# Piano di armonizzazione delle applicazioni NAS

Documento di pianificazione, basato sulla lettura del repository e delle quattro copie in
`external-apps/`. Nessuna delle modifiche descritte è stata implementata durante la preparazione
del piano. I rilievi sono statici: non attestano che build, test o integrazione sul NAS passino.

## 0. Mandato, confini e istruzioni per l'esecutore

Armonizzare esclusivamente le quattro app `nas-backup-manager`, `nas-disk-manager`, `nas-log-manager` e
`nas-file-manager`, mantenendo le loro funzioni attuali e completando l'integrazione Explorer,
ricerca globale e collegamenti alle cartelle. Il launcher è il riferimento contrattuale;
intervenire sul suo codice solo dove serve a queste integrazioni e alla relativa documentazione.

- Le app restano progetti autonomi. `external-apps/` è una sistemazione temporanea, non un
  workspace da integrare nella build del NAS. Nessun import, symlink, dipendenza `file:` o
  percorso di CI deve richiedere quella cartella o il checkout del launcher.
- Non committare `external-apps/` nel repository principale. Non eseguire `git add .`,
  `git add --all`, merge, tag, push o pubblicazioni durante l'implementazione. Gli script di
  release esistenti fanno queste operazioni: provarli soltanto in repository temporanei isolati.
- Controllare lo stato Git prima e dopo ogni fase e preservare le modifiche dell'utente. Alla
  lettura iniziale `external-apps/` era untracked; alla ripresa era presente anche `E.md` nella
  root. Non modificarlo o includerlo nel lavoro. Non cancellare le app temporanee al termine.
- Non implementare backup reali, mount/unmount dei dischi, indicizzazione dei file, nuovi
  protocolli generici, autenticazione, TLS o altri punti della roadmap. Non riscrivere i backend
  di dominio: sono pertinenti spostamento dei test, confini del pacchetto e integrazione richiesta.
- Non eliminare desktop/mobile/Tauri da Explorer per renderlo uguale alle altre app. Sono
  funzionalità esistenti e costituiscono una personalizzazione motivata. Il pacchetto NAS deve
  invece avere lo stesso contratto delle altre app.
- Non creare un SDK, un monorepo, un generatore o un servizio remoto di configurazione. Bastano
  file comuni uguali nelle repo e metadati locali per le differenze necessarie.
- Lavorare una sezione per volta su tutte e quattro le app, verificando i riferimenti a ogni
  spostamento. Alla consegna elencare differenze residue reali, controlli eseguiti e impedimenti.

### Fonti da rileggere prima di modificare

Nel NAS: `CLAUDE.md`, `.claude/docs/conventions.md`, `README.md`, `DEVELOPMENT.md`,
`docs/development/external_apps/APPS.md`, `src/apps/nas-global-search/README.md`,
`backend/axum/migrations/README.md`. Per il ciclo di vita: `backend/axum/src/services/manifest.rs`,
`registry.rs`, `lifecycle.rs`, `packages.rs`, `http/proxy.rs` e
`system/packages/ui_installation/files/nas-app@.service`.

Nelle app: `README.md`, `NAS.md`, gli `APPS.md` presenti, `nas-file-manager/DEVELOPMENT.md`,
manifest, configurazioni e script effettivamente richiamati. Se compaiono nuove regole locali,
leggerle prima di agire. Non trattare appunti come `E.md` come prova di test già superati.

Regole da mantenere: Vue Composition API con `script setup` TypeScript, nessun `any`, import
tramite alias, nomi CSS composti con `_`, stili di componente scoped, testi UI nei cataloghi,
nessuna migrazione applicata riscritta, errori Rust tipizzati e niente I/O bloccante negli handler
async. Gli attributi `data-testid` restano disponibili nei test e vengono rimossi in produzione.
Non usare il carattere U+2014. Non aggiungere librerie estranee alla richiesta; gli strumenti
comuni vanno scelti fra quelli già utilizzati, dichiarando esplicitamente gli import diretti.

### Incoerenze documentali da risolvere senza copiarle

- `.claude/docs/architecture.md` parla ancora di test fuori da `src`; `CLAUDE.md`, convenzioni,
  `DEVELOPMENT.md` e configurazioni attive del launcher usano `src/tests/`.
- L'introduzione di `APPS.md` cita porte dedicate, mentre codice e sezioni successive descrivono
  il proxy same-origin `/apps/<id>/`. Il secondo è il contratto da mantenere.
- Il README del NAS dice che Explorer non esiste ancora. Explorer esiste nelle copie fornite,
  ma l'integrazione richiesta non è completa. Aggiornare questa descrizione solo dopo le verifiche.
- Il README e DEVELOPMENT di Explorer descrivono script Tauri e workflow non presenti nel
  checkout. `NAS.md` rimanda a un `APPS.md` mancante. I link API nelle altre copie puntano a un
  percorso vecchio. Allineare i documenti al risultato effettivo, senza ricreare funzioni obsolete.
- Nel launcher non esiste attualmente `_nas-apps.scss`: le app interne usano
  `src/apps/styles/_apps.scss`; la controparte per iframe è `_external-apps.scss` nella stessa
  cartella. I file `_nas-apps.scss` esistono in tre app esterne, con contenuti non tutti uguali.

## Stato rilevato e priorità

I percorsi di questa tabella sono relativi a `external-apps/<app>/`.

| Area          | nas-backup-manager                                                                               | nas-disk-manager                                                           | nas-log-manager                                            | nas-file-manager                                                             |
| ------------- | ------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------- | ---------------------------------------------------------- | ---------------------------------------------------------------------------- |
| Test FE       | Due test release in `tests/`; script puntano anche a file assenti sotto `src/test` e `tests/nas` | `src/tests`, più `tests/e2e`, `tests/nas`, `tests/pipeline`                | `src/test`, più `tests/e2e`, `tests/nas`, `tests/pipeline` | `tests/`, molti test strutturali leggono il sorgente                         |
| Test BE       | Nessuna suite dedicata rilevata                                                                  | `backend/axum/tests/src`                                                   | `backend/axum/src/test`                                    | Test inline in `backend/axum/src/lib.rs` e `backend/core/src/lib.rs`         |
| Toolchain     | TS `6.0.3`; manca `package-lock.json`                                                            | TS `7.0.2`, Sass e Vue divergenti                                          | TS `6.0.3`                                                 | TS `6.0.3`, Tauri presente                                                   |
| Vite/PostCSS  | Config comune con PostCSS e rimozione test id                                                    | PostCSS assente; iniezione di `styles/global` senza il file corrispondente | Config comune con PostCSS e rimozione test id              | Config autonoma; plugin comuni presenti su disco ma non collegati alla build |
| Logo sorgente | `logo.svg`, più `public/icon.svg`                                                                | `logo.svg`, più `public/icon.svg`                                          | `src/assets/logo.svg`                                      | `assets/logo.svg`; icone sparse in `src/assets`                              |
| Tema e bridge | `useNasLauncherContext.ts`                                                                       | `useShellContext.ts`                                                       | Mescolati in `useNotifications.ts`                         | Preferenze locali e nessun bridge rilevato                                   |
| SCSS comune   | `_nas-apps.scss`, placeholder locali                                                             | Stessa copia di Backup più molto CSS in `common.css`                       | Copia modificata con stili specifici delle notifiche       | Sistema di token e tema autonomo                                             |
| Workflow      | Identità errata `package-manager` nella build                                                    | Build simile a Log                                                         | Build di riferimento per il confronto                      | Build simile a Log, più audit con pnpm e Node diverso                        |

Altri rilievi concreti:

- Nessuna app contiene `scripts/format-sfc-spacing.mjs`.
- Il `backend:dev` di Backup usa `/tmp/package-manager-dev.sock`; anche Vitest conserva
  `http://localhost/apps/package-manager/`.
- I tre `tsconfig.node.json` di Backup/Disk/Log estendono `@tsconfig/node24/tsconfig.json`, senza
  dichiarare il pacchetto nei rispettivi `package.json`. Non sono collegati dal tsconfig root.
- `scripts/axium/package-nas.mjs`, `test-nas-package.mjs`, `scripts/build/strip-test-ids.ts` e
  `frontend-boundary.ts` sono già identici nelle quattro copie. Evitare quattro riscritture.
- `frontend-boundary.ts` contiene percorsi e nome plugin di Picture Manager, estranei alle app.
  Se attivato senza adattamenti in Explorer, rifiuterebbe gli import Tauri oggi presenti.
- Sonar di Explorer indica cartelle test inesistenti e non include `backend/core/src`; la
  coverage FE di Explorer non produce LCOV e copre solo cinque moduli.
- I compose Sonar sono già identici: stesso container, porta e volumi `nas-os_*`. Non è un
  servizio diverso per ciascuna app, e quattro istanze simultanee entrerebbero in conflitto.
- Tutti i manifest hanno `provides: []`; Explorer ha anche `permissions.paths: []`, mentre
  l'installer NAS possiede già un drop-in dedicato a `/srv/storage`.
- La richiesta cita `merge/verify_pipeline.sh`, ma questo file non è presente. Gli equivalenti
  attivi sono `scripts/pipeline/merge_release.mjs`, `release-version.mjs` dove presente,
  `verify_release.mjs` e `prepare_release.mjs`.

Priorità: correggere riferimenti e contratti che impediscono build/installazione/invocazione,
poi uniformare struttura e tooling, infine verificare aspetto e integrazione completa. Non
assumere che la presenza di vecchi report di coverage o archivi equivalga a un checkout funzionante.

## 1. Struttura e scaffolding

Adottare questa struttura comune, con cartelle di dominio solo dove hanno contenuto reale:

```text
src/
  App.vue
  main.ts
  assets/
    logo.svg
    icons/*.svg
  components/
    common/
    <area>/
  composables/
    useNasLauncherContext.ts
    <logica di dominio>.ts
  services/
    nasShell.ts
    <servizi di dominio>.ts
  types/
    launcher.ts
    <tipi di dominio>.ts
  i18n/
    index.ts
    locales/en.ts
    locales/it.ts
  styles/
    main.scss
    global.scss
    _nas-apps.scss
    partials/_reset.scss
    partials/_variables.scss
    partials/_mixins.scss
    partials/_placeholders.scss
  tests/
    unit/<percorso relativo a src>/*.unit.spec.ts
    component/<percorso relativo a src>/*.component.spec.ts
    scripts/<percorso relativo a scripts>/*.{spec.ts,test.mjs}
    e2e/<area>/*.spec.ts
    fixtures/
backend/
  Cargo.toml
  Cargo.lock
  axum/Cargo.toml
  axum/src/
  tests/
    unit/axum/<percorso relativo ad axum/src>/*.rs
    integration/axum/*.rs
scripts/
  format-sfc-spacing.mjs
  build/
  axium/
  pipeline/
```

La suddivisione FE `unit/component/e2e`, uguale nelle quattro app, rende esplicita la richiesta
di cartelle per tipo di test. I sottopercorsi rispecchiano i sorgenti: per esempio
`src/composables/useDiskInventory.ts` corrisponde a
`src/tests/unit/composables/useDiskInventory.unit.spec.ts`. Documentare la convenzione nelle
app; non migrare per questo tutti i test del launcher, che usa già la separazione per suffisso.

Azioni:

1. Spostare le suite esistenti, aggiornando import, alias, fixture, config, package scripts,
   Sonar, coverage, riferimenti Rust `#[path]` e sezioni Cargo `[[test]]` insieme ai file.
2. Spostare i test Rust fuori dal codice produttivo. Explorer conserva `backend/core` e
   `backend/tauri`; usare rispettivamente `backend/tests/unit/core/...` e, dove esistono test,
   `backend/tests/unit/tauri/...`. I crate rimangono separati, non appiattirli in Axum.
3. I test di componente devono montare e interagire con gli SFC. Quelli Explorer denominati
   `*Structure.spec.ts` oggi verificano anche stringhe CSS e import: non rinominarli fingendo
   che siano test di componente. Convertire i controlli pertinenti ai componenti modificati;
   mantenere solo i controlli statici che esprimono realmente una convenzione di repository.
4. Backup necessita di una copertura minima reale per bridge, rendering e packaging, al posto
   di script verso file assenti. Non inventare test di funzioni backend non implementate.
5. Conservare `scripts/axium/` per evitare un rinomino puramente cosmetico: è un nome storico,
   già comune a tutte le repo. Aggiornare il contenuto, non creare un secondo `scripts/axum/`.
6. Explorer può conservare `src/apps/desktop`, `mobile`, `web` e i moduli necessari ai runtime.
   Uniformare la struttura comune non significa introdurre quelle cartelle nelle altre app.

**Completamento:** nessun glob obsoleto, ogni suite scopre davvero i test previsti, produzione
esclude test e fixture, nessun test inline Rust nei moduli interessati dalla migrazione.

## 2. Script e prima release senza tag

Rendere identici gli script generici di build, package, verifica e release. Identità e nomi
arrivano da `package.json`, `nas-app.json` e metadata Cargo, non da stringhe per app. Le sole
opzioni specifiche devono descrivere vere differenze, non replicare questi dati.

- Unificare `prepare_release.mjs` e l'uso di `release-version.mjs`. Rendere i moduli di logica
  importabili senza eseguire il merge al caricamento, come già fa il guard dell'entrypoint nel NAS.
- Conservare `verify_release.mjs` come verifica senza effetti Git. La creazione del tag appartiene
  a `merge_release.mjs`, dopo il superamento dei controlli. Non aggiungere uno shell script con
  il nome citato nella richiesta se non viene trovato un chiamante effettivo.
- Il NAS ha già una gestione `latestTag()`/`nextVersion(..., null, ...)` in
  `scripts/release/merge_release.mjs`. Usarla come riferimento, senza copiare il suo `catch`
  indiscriminato o il rollback `git checkout -- .` nelle app.

Comportamento richiesto, da fissare con test Git temporanei:

| Condizione                                                       | Risultato                                                                                            |
| ---------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------- |
| Nessun tag release valido raggiungibile da HEAD; package `0.1.0` | Prima release `v0.1.0`, senza bump implicito                                                         |
| Tag soltanto su un altro ramo                                    | Stessa regola per il ramo corrente; rilevare eventuale collisione del nome prima di scrivere         |
| Package avanti rispetto all'ultimo tag raggiungibile             | Usare la versione del package                                                                        |
| Package uguale o inferiore al tag raggiungibile                  | Applicare il bump patch/minor/major previsto                                                         |
| Tipo di bump o versione invalidi                                 | Errore esplicito, nessun tag                                                                         |
| Tag candidato già esistente                                      | Non sovrascriverlo e non fare force-push; errore esplicito o gestione di ripetizione già documentata |
| Comando Git fallisce per altro motivo                            | Propagare l'errore, non trattarlo come prima release                                                 |
| `--dry-run`                                                      | Mostrare versione e tag candidati; nessuna scrittura, commit, fetch/push o tag                       |

Non basta intercettare `git describe`: senza tag il changelog deve usare la cronologia di
`HEAD`, non un intervallo `null..HEAD`/`undefined..HEAD`. Selezionare tag di versione validi,
non qualsiasi nome che comincia con `v`. Considerare checkout shallow e tag non scaricati nei
messaggi di errore e nella documentazione; CI deve avere la cronologia necessaria.

Sincronizzare la versione nei campi corretti di package, lock npm, crate eseguibile, manifest NAS,
Sonar e changelog. Aggiornare Cargo.lock mediante Cargo; non sostituire indiscriminatamente le
versioni dei crate `core`/`tauri`. Rigenerare il lock npm con il comando comune, mai a mano.

Separare assemblaggio e compilazione: la catena attuale compila FE/BE e poi `nas:package` li
ricompila. Una sola compilazione per esecuzione, seguita dall'assemblaggio e dallo smoke test.
Conservare i controlli di percorso, versione, checksum e riproducibilità già implementati dal
packager. Non eseguire lo script di merge contro il Git del NAS per provare una copia esterna:
potrebbe risalire fino alla root e aggiungere tutti i file.

## 3. Workflow GitHub e Dependabot

Usare le stesse definizioni e gli stessi comandi nelle quattro repo. Non centralizzare i workflow
in una quinta repo né aggiungere dipendenze dal checkout NAS.

- `build.yml`: stesso trigger tag `v*.*.*`, runtime Node, runner di build Linux, toolchain Rust,
  cache, installazione `npm ci`, controllo versioni e appartenenza del commit al branch di
  rilascio, compilazione, smoke test dell'archivio, checksum, note, pubblicazione e cleanup.
- Togliere l'identità codificata nel workflow: leggere ID e nome da `nas-app.json` nei job che
  ne hanno bisogno. Correggere il `package-manager` residuo di Backup. Pubblicare solo gli asset
  della versione in esame, non vecchi archivi rimasti in `release-assets/`.
- Uniformare un `ci.yml` per pull request e invocazione manuale con format check, lint, type
  check, unit, component, E2E e controlli Rust. Il NAS contiene un esempio reale; adattarlo al
  backend delle app, senza job gateway o generazione API del launcher.
- Mantenere il cancello completo anche nel percorso release finché non è verificata una reale
  protezione del branch: non togliere test dal workflow assumendo ruleset remoti inesistenti.
  CI deve controllare la formattazione, non correggere silenziosamente i file che pubblicherà.
- Uniformare `audit.yml` partendo da quello Explorer: stessi trigger e strumenti, `npm ci` e
  audit npm coerenti con il lock usato per build, Cargo audit sul lock del workspace. Non
  mantenere l'installazione pnpm frozen se quel lock resta locale e ignorato.
- Dependabot è `.github/dependabot.yml`, non un workflow: npm `/`, Cargo `/backend`, Actions
  `/`, stesse cadenze mensili e raggruppamenti già presenti. Conservare pin SHA comuni delle
  Actions, senza introdurre un aggiornamento generale delle versioni estraneo al lavoro.
- La repo NAS usa `master`, gli script delle app assumono `main`. Verificare il branch effettivo
  delle repo autonome al momento dell'esecuzione; non dedurlo dal Git del contenitore temporaneo.
- Conservare x64 come target pubblicato attuale. ARM64 resta una build nativa supportata dal
  packager, non una nuova matrice da aggiungere automaticamente.

**Completamento:** confronto dei workflow senza differenze operative non motivate; la stessa
sequenza di comandi usata localmente costruisce l'asset che CI pubblicherà. La pubblicazione vera
non fa parte della verifica di questo task.

## 4. Package.json e dipendenze minime

Uniformare nomi e valori degli script comuni, non solo l'insieme delle chiavi. Esporre soltanto:

| Gruppo       | Interfaccia comune prevista                                                                       |
| ------------ | ------------------------------------------------------------------------------------------------- |
| Sviluppo     | `dev`, `backend:dev`                                                                              |
| Build        | `type-check`, `build`, `nas:package`, `nas:test`                                                  |
| Qualità      | `lint`, `format`, `format:check`, `format:sfc`                                                    |
| Test         | `test`, `test:unit`, `test:component`, `test:e2e`, `test:coverage`, `coverage:rust`, `test:sonar` |
| Release      | `lock:npm`, `release:check`, `release:test`, `release:merge`                                      |
| Sonar locale | `podman:sonar:up`, `podman:sonar:down`, `podman:sonar:logs`                                       |

Gli entrypoint generici leggono l'identità dal manifest. Per esempio il socket di sviluppo è
derivato dall'ID o fornito come opzione, non copiato come stringa diversa in quattro package.
`nas:package` compila e assembla; il gate non deve ripetere quella compilazione.
`test` esegue unit e component una volta; coverage è un'esecuzione separata. I test degli script
sono inclusi nel comando unit, eliminando `release:test:unit` se diventa un alias ridondante.
Mantenere alias vecchi solo se un chiamante effettivo non è migrabile nella stessa modifica.

Inventariare import TS/Vue, configurazioni, script e CLI prima di togliere una dipendenza.
Non considerare inutilizzata una libreria solo perché non compare in `src/`.

- Base runtime: Vue. Non introdurre Pinia, router o vue-i18n dove non servono.
- Base dev: Vite/plugin Vue, TypeScript/vue-tsc, Sass, ESLint/parser/plugin realmente usati,
  Prettier, Vitest/coverage, Vue Test Utils/jsdom, Playwright e PostCSS richiesto dalla sezione 6.
- `@vue/compiler-core` è un import diretto dello strip test id, ma solo Backup lo dichiara:
  dichiararlo nelle app che usano il plugin, allineato a Vue, senza dipendere dal transitive hoisting.
- Tipi Node e preset TS devono essere dichiarati se rimangono importati/estesi. Preferire config
  esplicite comuni ai preset orfani, evitando nuove dipendenze solo per copiare il launcher.
- `@tauri-apps/api` è effettivamente importato da Explorer. Non rimuoverlo come superfluo;
  isolare il codice nativo dal bundle NAS prima di applicare il controllo dei confini.
- Uniformare le versioni condivise e i lock risolti, con ESLint e Prettier fissati esattamente.
  La baseline dichiarata comune è ESLint `10.11.0` e Prettier `3.9.9`: verificarla con i lock e
  l'installazione pulita, senza presentarla come compatibilità già dimostrata. TS `7.0.2` di Disk
  rispetto a `6.0.3` degli altri richiede una scelta unica verificata con vue-tsc e parser.
  Non selezionare automaticamente il numero più alto o avviare un upgrade generalizzato.
- Allineare anche plugin Vue, parser, Sass, Vitest/coverage e PostCSS. Documentare una sola
  versione Node supportata in locale/CI, coerente con le dipendenze risolte.
- `package-lock.json` deve esistere in tutte le app ed essere aggiornato via `lock:npm`.
  Conservare la distinzione del NAS: pnpm locale, npm ci in pipeline; nessun lock modificato a mano.

**Completamento:** installazione pulita per ogni repo autonoma, nessun import diretto non
dichiarato, nessuno script verso file inesistenti, stessa toolchain risolta per gli strumenti comuni.

## 5. TypeScript

Uniformare `tsconfig.json`, `tsconfig.app.json`, `tsconfig.node.json` e un
`tsconfig.vitest.json` comune. Il comando `type-check` deve controllare davvero app, config e test,
non soltanto `tsconfig.app.json` come fa la build attuale.

- Strict mode, target/moduli comuni compatibili con Vite, `noEmit`, DOM e tipi Vite per FE,
  tipi Node per script/config, tipi test separati. Cache tsbuildinfo sotto `node_modules/.tmp`.
- Alias identici: `@/* -> src/*`, `@tests/* -> src/tests/*`, `@scripts/* -> scripts/*`.
- Escludere `src/tests/**` dal progetto produttivo; includerlo esplicitamente in quello test.
- Includere tutti i `vite.config.*`, Vitest, Playwright, ESLint e plugin TS di build nel controllo
  appropriato. Rimuovere riferimenti a Cypress se inutilizzato.
- Eliminare `tsconfig.axum.json` se non ha più un compito distinto, oppure collegarlo al percorso
  realmente invocato: attualmente non è il progetto selezionato da `build:axum`.
- Le specificità Tauri di Explorer devono stare nel progetto/adattatore dedicato, senza
  disabilitare il controllo comune o allargare indiscriminatamente gli include delle altre app.

## 6. Vite, PostCSS, alias e data-testid

Tenere una sola struttura: `vite.config.common.ts` per alias, SCSS, PostCSS e factory dei plugin;
`vite.config.axum.ts` per `base: './'`, output `dist/axum` e sviluppo NAS;
`vite.config.ts` come entrypoint predefinito NAS. Vitest riusa le parti compatibili senza strip
test id né produzione di report di build. La modalità nativa Explorer rimane una specializzazione.

- Stessa pipeline PostCSS e stesse versioni: `postcss-combine-duplicated-selectors`,
  `@fullhuman/postcss-purgecss`, `cssnano` (il pacchetto presente è `cssnano`, non `nano-css`).
  Applicare le ottimizzazioni alla build produttiva, mantenendo dev/test leggibili.
- Unica configurazione di content/extractor e safelist per classi dinamiche Vue, temi e stati
  comuni. Controllare esplicitamente `tone_*`, classi costruite a runtime e stati Explorer:
  non sacrificare stati visuali per una riduzione CSS apparente.
- Identici alias Vite/TS/Vitest e identica iniezione del file SCSS senza emissione di CSS.
  Eliminare alias duplicati tra config base e factory.
- Collegare in tutte le build produttive `stripTestIdsNodeTransform` e `stripTestIds()` già
  esistenti. In Explorer copiarli non basta: oggi non vengono usati.
- Provare attributi statici, `:data-testid`, HTML e ID passati tramite props/wrapper. Se un caso
  usato dal codice sfugge al transform, correggere quel caso senza regex sul JS minificato che
  potrebbe alterare dati. Dev ed E2E devono conservarli, bundle/DOM produttivo no.
- Ripulire `frontend-boundary.ts` dai riferimenti a Picture Manager. Per Explorer separare gli
  adattatori HTTP e Tauri e selezionarli a build time: un ramo runtime con import statico Tauri
  non isola il bundle NAS. Non introdurre una piattaforma di adapter nelle altre tre app.
- Tutte le URL FE dell'app NAS, incluse preview, download e upload, devono restare relative
  all'app. Un proxy Vite che fa funzionare `/api/...` in locale non dimostra l'integrazione NAS.
- Per sviluppo ed E2E usare un inoltro HTTP verso il socket Unix in configurazione di test/dev
  se necessario. Non aprire una porta TCP nel processo NAS per aggirare il contratto.

**Completamento:** una build vera caricata sotto `/apps/<id>/` mostra UI, asset e chiamate API
corrette; test id assenti in produzione e presenti in dev; niente moduli Tauri nel bundle NAS.

## 7. SonarQube e Podman

- Stessa struttura `sonar-project.properties`, con soli ID/nome/versione e sorgenti aggiuntivi
  effettivi per app. Test in `src/tests` e `backend/tests`, esclusi da `sonar.sources` e coverage.
- Explorer include `backend/core/src`, oltre ad Axum; trattare il codice nativo esistente
  esplicitamente, distinguendo ciò che viene analizzato da ciò che viene compilato nel gate NAS.
- FE produce `coverage/lcov.info`; Rust `backend/coverage/lcov.info`; Clippy
  `backend/clippy-report.json`. Verificare che i path `SF:` risolvano file reali dopo gli
  spostamenti, anche per `core`. Non conservare un `sed` che gestisce soltanto `axum`.
- Escludere file generati e traduzioni in modo uniforme. Non nascondere moduli poco coperti o
  abbassare soglie solo per fare passare il gate. Le soglie 80% già presenti vanno riconciliate
  con le suite realmente incluse, annotando la baseline e le lacune, non inventando copertura.
- Mantenere lo stesso compose già comune e documentare una sola istanza locale Sonar condivisa
  fra i progetti. Gli ID Sonar distinguono le analisi. Non lanciare quattro container omonimi e
  non cambiare nomi dei volumi causando perdita apparente dello storico.
- Uniformare comandi up/down/logs e scanner rootless con
  `--userns=keep-id:uid=1000,gid=1000`, token da ambiente e stessi mount/opzioni. Niente token
  nei file. `down` non deve rimuovere volumi.
- Documentare i prerequisiti Cargo LLVM e Podman. Se Sonar non è disponibile, dichiarare la
  mancata analisi separatamente dai test e dalla generazione dei report riusciti.

## 8. ESLint, Prettier e SFC

Prendere come base le convenzioni del launcher, mantenendo file di configurazione identici fra
le app. Nessuna personalizzazione del nome è necessaria per ESLint o Prettier.

- Unificare su `eslint.config.ts`, scope browser/Node/test, parser Vue/TS, `curly: all`, semicolon,
  import type e gestione unused. Non escludere cartelle di sorgenti per nascondere errori.
- Usare la stessa `.prettierrc.json` del NAS e una `.prettierignore` coerente: build, lockfile,
  report e file generati esclusi; `.vue` demandati al formatter SFC.
- Copiare e usare lo stesso `scripts/format-sfc-spacing.mjs` in tutte le app. Uniformare anche
  la lingua dei messaggi CLI se si adatta la copia; non introdurre quattro versioni diverse.
- `format`: Prettier, ESLint fix, poi formatter SFC. `format:check`: controlli senza scrittura,
  incluso `format-sfc-spacing.mjs --check`. `format:sfc`: soltanto il formatter SFC.
- Versioni esatte uguali di ESLint, Prettier e plugin che influenzano l'output. Stesse regole
  editor per evitare che il salvataggio rimuova subito le righe vuote richieste.
- Formattazione idempotente: una seconda esecuzione non modifica file. Separare nel diff gli
  spostamenti/formattazione dalle correzioni funzionali, senza fare commit durante questo task.

## 9. Documentazione, lingua e manifest

Stesso insieme minimo di documenti nelle app: `README.md` (uso e avvio), `DEVELOPMENT.md`
(struttura, comandi, test e convenzioni), `NAS.md` (pacchetto e release), `APPS.md` (contratto
comune), `CHANGELOG.txt`. Non duplicare intere sezioni fra README e DEVELOPMENT.

- README e DEVELOPMENT in italiano in tutte le app; mantenere in italiano anche NAS.md.
  APPS.md può restare in inglese come contratto tecnico, con contenuto uniforme. I link verso
  il NAS devono funzionare dalla repo autonoma, non dipendere da `../../nas-os`.
- Codice, nomi di metodi/tipi, commenti tecnici, descrizioni dei test, messaggi CLI ed errori
  tecnici in inglese. Chiavi i18n inglesi coerenti; stringhe `it` in italiano e `en` in inglese.
  Non tradurre dati degli utenti, nomi dei file o log ricevuti. I test in italiano possono
  legittimamente aspettarsi etichette del catalogo italiano, con descrizione del test inglese.
- Non trasformare UI italiana in testo inglese hardcoded: ogni label, tooltip e aria-label
  passa dal catalogo. Eliminare residui Picture/Package Manager che non descrivono l'app.
- Schema `nas-app.json` comune: `manifest_version: 1`, `app`, `run`, `ui.protocol: 1`, `provides`,
  `permissions.paths`. ID stabili; nome, versione, eseguibile e capability sono differenze vere.
- Conservare `app.icon: "icon.svg"` nel pacchetto: il logo sorgente comune è copiato lì dal
  packager. Il percorso del socket deve essere coerente con l'ID; il NAS lo deriva comunque.
- Explorer dichiara ricerca e percorso di storage solo insieme all'implementazione verificata.
  Gli altri manifest non devono acquisire capability o permessi inutilizzati.
- Allineare documentazione del protocollo in tutte le copie e nel NAS. Rimuovere esempi di porte
  dedicate, referrer obbligatorio e stylesheet inesistenti. Conservare distinti esempio generico
  `fileExplorer` e ID reale `nas-file-manager`; non rinominare l'identità installata.
- Aggiornare README/DEVELOPMENT del NAS e manuali interessati soltanto per comportamenti
  effettivamente completati. Nella roadmap non marcare concluso il drag di collegamenti dentro
  cartelle: è una funzione diversa dai collegamenti a cartelle richiesta qui.

## 10. Asset e icone

Per gli asset sorgente FE usare `src/assets/icons/*.svg` e `src/assets/logo.svg` in tutte le app.
Qui `assets/icons` e `assets/logo.svg` sono intesi all'interno di `src`, come nel layout FE NAS.

- Migrare i loghi root di Backup/Disk, quello `assets/logo.svg` di Explorer e le icone sparse di
  Explorer. Aggiornare import, favicon, test, packaging e documenti nella stessa fase.
- Eliminare duplicazioni di logo/public icon solo dopo aver aggiornato tutti i consumatori.
  `icon.svg` dell'archivio è un output di packaging, non una seconda sorgente da mantenere.
- Le icone UI devono provenire da file SVG: sostituire emoji, glifi, font-icon e SVG duplicati
  inline che fanno da icona. Non convertire contenuti reali o anteprime di file in icone.
- Componente icona comune con API uguale e nome accessibile/uso decorativo esplicito. Conservare
  palette dei loghi e comportamento visivo, senza aggiungere una libreria di icone.
- I formati PNG/ICO/ICNS di `backend/tauri/icons` sono risorse native esistenti, non icone UI
  del NAS: non includerli nel pacchetto NAS e non distruggere un target nativo per imporre SVG
  a un formato di bundle che può richiedere raster. Documentare questa eccezione circoscritta.

## 11. Tema e i18n

Usare stessa posizione, nomi e API nelle quattro app:

- `src/i18n/index.ts`: stato reattivo della lingua, `useI18n()` e setter; cataloghi
  `src/i18n/locales/{it,en}.ts` con parità delle chiavi e tipi coerenti. Per le app conservare
  l'API `t` reattiva già diffusa, uniformandola; non imporre una nuova libreria.
- `src/composables/useNasLauncherContext.ts`: lifecycle del bridge, applicazione tema e lingua;
  tipi e trasporto nei file della sezione 13. Il dominio non crea un secondo listener globale.
- In iframe la fonte di tema/lingua/accent è il launcher; il tema ricevuto è già risolto
  `light|dark`. Aggiornare `data-theme`, `html.lang` e `--accent` a ogni context valido.
- Accent vuoto significa rimuovere l'override inline e usare il token del tema, non fissare
  `#3c8fbe` dal codice. Validare il colore prima di applicarlo.
- Standalone preserva preferenze e query `theme/lang` già supportate. Le preferenze native di
  Explorer (`auto`) non devono sovrascrivere un context NAS dopo il mount o il cambio del sistema.
  Rendere esplicita la precedenza context > preferenze standalone > default comune.
- Prefissare lo storage con l'ID dell'app. Se si rinominano chiavi Explorer già usate, mantenere
  la lettura/migrazione minima del valore precedente; non azzerare preferiti e preferenze.
- Nessun controllo tema/lingua dentro un'app in iframe deve contraddire le impostazioni del
  launcher. Preservare i controlli standalone/native dove servono.

**Completamento:** tutte le app reagiscono a cambio lingua, tema e accent senza riapertura;
nessun listener duplicato o rimasto attivo dopo l'unmount; traduzioni complete nei due cataloghi.

## 12. SCSS condiviso e riduzione del CSS specifico

Non modificare `src/apps/styles/_apps.scss` del launcher per adattarlo alle app esterne.
Non riscrivere automaticamente `_external-apps.scss` o i token interni: sono riferimenti da
confrontare, non file da aggiornare in massa.

1. Confrontare le tre copie `_nas-apps.scss` con `_external-apps.scss`. Backup e Disk sono uguali;
   Log ha aggiunte specifiche (`tone_*`, pannelli e altri stili). Estrarre queste aggiunte dove
   appartengono e scegliere una copia comune invariata per le quattro app, Explorer compreso.
2. Uniformare il flusso: `main.ts` importa solo `styles/main.scss`; quest'ultimo importa reset,
   `_nas-apps.scss` e regole globali necessarie una volta. `global.scss` inoltra solo variabili,
   mixin e placeholder ed è iniettato negli SFC. Non includere CSS che emette regole nel file
   iniettato, altrimenti ogni componente lo duplica.
3. I token mancanti di spaziatura, dimensioni, durata e layout comune stanno in
   `partials/_variables.scss`, uguale nelle app, senza riscrivere il file base `_nas-apps.scss`.
   Le personalizzazioni di dominio restano locali e minime.
4. Ricondurre colori/superfici/bordi alle variabili semantiche comuni. Explorer ha nomi e valori
   propri (`--surface`, ecc.): usare una mappatura minima ai token NAS nel tema NAS e migrare gli
   usi pertinenti, preservando i target nativi. Evitare una seconda palette NAS parallela.
5. Ridurre `nas-disk-manager/src/styles/common.css`, i literal negli SFC e gli override di Log
   riusando `.app_surface`, `.app_panel`, `.app_toolbar` e le primitive presenti. Non creare
   una classe nuova per ogni regola solo per spostare CSS in un altro file.
6. Reset unico, stesso ordine di import, mixin/placeholder comuni e senza regole inutilizzate.
   Eliminare duplicazioni dopo confronto degli usi; un cambio di nome non è una deduplicazione.
7. Root delle quattro app NAS con `.app_surface`. Token, focus e responsive devono funzionare
   anche nelle modali eventualmente teletrasportate fuori da quella root.

Le repo separate e gli iframe richiedono una copia locale del CSS condiviso: questa duplicazione
di distribuzione è intenzionale. Non sostituirla con import dal launcher o caricamenti di CSS
runtime. Il risultato va confrontato sia in dev sia nel CSS ottimizzato di produzione.

**Completamento:** `_nas-apps.scss` uguale nelle app, CSS comune emesso una volta, diff dei file
stile interni NAS vuoto salvo necessità concreta spiegata; verifica visuale chiaro/scuro, accent,
tabelle, modali, menu, liste e vista stretta delle quattro app.

## 13. Comunicazione con il launcher

Contratto esistente: `nas: 1`; app -> shell `ready`, `notify`, `open`, `describe`; shell -> app
`context`, `described`. Mantenerlo e aggiungere soltanto il messaggio per i collegamenti richiesti.

| File comune nelle app                      | Responsabilità                                                    |
| ------------------------------------------ | ----------------------------------------------------------------- |
| `src/types/launcher.ts`                    | Tipi del protocollo, context, messaggi e risultato collegamento   |
| `src/services/nasShell.ts`                 | Validazione messaggi unknown, invio/ricezione, origine e callback |
| `src/composables/useNasLauncherContext.ts` | Lifecycle Vue e applicazione di context a tema/i18n               |

- Validare sia `event.source === window.parent` sia `event.origin === window.location.origin`
  nel contesto NAS same-origin. Verificare versione e forma del payload prima di utilizzarlo.
  Il launcher continua a controllare origine e `contentWindow` dell'iframe registrato.
- Non basarsi su `document.referrer`: `AppHost.vue` usa `referrerpolicy="no-referrer"` e Disk
  oggi non invia `ready` quando il referrer manca. Nel NAS l'origine è nota dall'URL della pagina.
- Inviare `ready` dopo la registrazione del listener con target origin esplicito; context è
  applicabile più volte senza duplicare listener/timer. Standalone non invia messaggi a sé stesso.
- Le funzioni pubbliche comuni per notify/open/describe/shortcut devono avere stessa firma e
  posizione. L'app usa solo quelle necessarie; polling e lettura notifiche rimangono nel dominio Log.
- Estrarre da `nas-log-manager/useNotifications.ts` gestione context e listener; conservare richiesta
  `describe`, traduzione delle notifiche, selezione `?notification=` e cleanup delle richieste.
- Il codice esterno non importa `src/types/types.ts` dal NAS. Il contratto è copiato nelle repo,
  con test dello stesso payload, e documentato in APPS.md; niente dipendenze cross-repo.
- Non aprire nuove API dirette dalla pagina esterna alle API del launcher. Il backend Log può
  continuare a usare la sua integrazione HTTP già esistente.

**Completamento:** test di messaggi validi, origine/finestra sbagliata, versione/payload invalidi,
aggiornamento context, mount/unmount e descrizione notifiche. La modifica del bridge non deve
rompere notifiche, nas-log-manager o l'apertura delle altre app.

## 14. Explorer, launcher e ricerca globale

### 14.1 Stato attuale verificato nel codice

- Il launcher elenca già i provider con `GET /api/capabilities/search`. `nas-global-search/api.ts`
  interroga `/apps/<id><endpoint>` con `q`, `type`, `ext`, timeout di 10 secondi e risultati
  parziali; assegna `app_id` in base al provider, non a ciò che il provider dichiara.
- Explorer espone `/api/search?query=...` in `backend/axum/src/lib.rs`, con risposta `FileEntry[]`.
  Questo non è il contratto della shell: mancano `id`, `type`, `extension`, `target`; `modified`
  è numerico/opzionale e i tipi file non coincidono. La capability non è dichiarata nel manifest.
- `src/services/filesystem.ts` di Explorer usa URL assolute `/api/...`: dentro l'iframe arrivano
  al launcher invece che al backend dell'app. Sono coinvolti anche preview/upload/download.
- Non è stato trovato un bridge postMessage in Explorer né l'inizializzazione della navigazione
  da `?path=`. `useFilesystem()` carica la root al mount: verificare la precedenza del deep link.
- Il NAS ha già `ShortcutKind::Folder` e la tabella `desktop_shortcuts` con `kind/target/name`;
  le API di aggiunta/rimozione pubbliche e lo store/desktop gestiscono solo app. Il filtro in
  `appsStore.shortcuts` scarta le cartelle. Non serve inventare da zero la persistenza.
- `shellStore.open(id, { location })` e `AppHost.address()` aprono già una query nel frame.
  Cambiare `location` modifica `src`; provare anche un'app già aperta, senza creare nuove schede.
- Il backend Explorer accetta `--listen`, ma non legge `NAS_APP_SOCKET`; senza argomento apre
  TCP su 3000. Il drop-in NAS imposta già `FS_EXPLORER_ROOT=/srv/storage`; il manifest lo omette.

### 14.2 Raggiungibilità e contratto del pacchetto

1. Correggere tutte le URL dell'adattatore web Explorer in relative all'app e provarle dietro
   il proxy, non soltanto sul dev server. Conservare API native nell'adattatore Tauri.
2. Uniformare l'avvio NAS: `--listen` oppure `NAS_APP_SOCKET`, `/health` JSON con versione e
   statici del pacchetto. Nessun fallback TCP implicito nel percorso NAS. Se resta un runtime
   web standalone TCP, deve essere un'opzione esplicita e documentata separatamente.
3. Dichiarare `/srv/storage` in `permissions.paths` di Explorer e verificare il drop-in esistente
   `system/packages/ui_installation/files/nas-app@nas-file-manager.service.d/`. Non estendere
   i permessi del template generale o quelli delle altre app. Dichiarare un percorso non lo concede.
4. Verificare che l'avvio sul NAS usi la root di storage e non la directory del pacchetto. La
   cache preview di Explorer oggi è vicina al filesystem esplorato: nel runtime NAS indirizzare
   lo stato tecnico scrivibile a `$STATE_DIRECTORY` se necessario a rispettare il contratto,
   preservando i file dell'utente e il comportamento native. Niente pulizie indiscriminate.

### 14.3 Provider di ricerca

Conservare `/api/search?query=` per i chiamanti Explorer esistenti. Aggiungere un adattatore
dedicato, per esempio `/api/launcher/search`, e dichiararlo nel manifest:

```json
"provides": [
  { "capability": "search", "version": 1, "endpoint": "/api/launcher/search" }
]
```

L'adattatore deve riusare `Filesystem::search`, senza introdurre un indice o riscrivere il motore.
Accetta `q`, `type`, `ext` e restituisce il contratto documentato:

```ts
interface SearchResult {
  id: string;
  name: string;
  path: string;
  type: 'folder' | 'document' | 'image' | 'audio' | 'video' | 'archive';
  extension: string;
  size: number;
  modified: string;
  target: string;
}
```

- `id` stabile derivato dal percorso logico nella root Explorer; niente `app_id` deciso dall'app.
- `path` e `target` usano lo stesso spazio di percorsi della UI (es. `/Documents` relativo alla
  root configurata, non `/srv/storage/Documents` hardcoded). `target` è una query URL encoded.
- Classificare audio tramite estensione e mappare file generici al tipo compatibile `document`;
  mantenere la classificazione interna indipendente. Estensione normalizzata senza punto,
  filtri applicati realmente, non solo accettati nella query.
- Convertire il timestamp alla forma ISO 8601 dal valore effettivo del core. Gestire metadata
  mancanti senza date inventate o numeri NaN: usare convenzioni documentate compatibili con
  la UI (es. data vuota resa come `-`, size 0 quando il contratto numerico non ammette null).
- Cartella: `target=?path=<cartella>`. File: aprire la cartella contenitrice e selezionare il
  file tramite una query esplicita, ad esempio `?path=<cartella>&select=<file>`. Definire una
  sola funzione per costruire/leggere queste query, senza doppio encoding.
- Rispettare il limite di 2048 caratteri del contratto di apertura; non troncare un percorso
  trasformandolo in un altro. Trattare esplicitamente gli elementi non rappresentabili.
- Conservare limiti di scansione già presenti nel core e uso di `spawn_blocking`; query invalide
  producono un errore tipizzato, risultati assenti un array vuoto. Non aggiungere indicizzazione.
- Il provider può essere installato ma fermo: provare la ricerca dopo la chiusura dell'app,
  dato che la shell arresta il servizio alla chiusura. Se necessario il launcher avvia il
  provider tramite lifecycle esistente prima della query; non introdurre servizi sempre accesi
  né ignorare `NAS_APPS_MODE`. Gestire l'indisponibilità senza bloccare gli altri provider.

### 14.4 Apertura a percorso specifico

Inizializzare Explorer dalla query prima del caricamento predefinito della root. Integrare la
lettura con lo stato di navigazione desktop/web/mobile effettivamente usato, evitando due
caricamenti concorrenti che rimettono la root dopo il deep link.

Provare cartella root, sottocartella, spazi, Unicode, `#`, `&`, `%`, percorso inesistente e file
rimosso. Validare i percorsi con le regole del core, non con il solo FE. Le API devono continuare
a impedire traversal e fuga dalla root, compresi symlink. Una selezione non valida mostra un
errore comprensibile e non apre risorse diverse.

Riutilizzare `shell.open('nas-file-manager', { location })`. Aprire A, poi B dalla ricerca o dal
desktop mentre Explorer è già aperto deve portare a B; ripetere lo stesso collegamento deve
almeno riportare in primo piano l'app. Non aggiungere un protocollo di navigazione separato se
il cambio del `src` del frame esistente soddisfa questi casi.

### 14.5 Collegamenti alle cartelle: richiesta e persistenza

Aggiungere all'azione della cartella Explorer una richiesta al launcher. Schema minimo proposto:

```ts
// Explorer -> launcher, origine e iframe verificati dal bridge.
{ nas: 1, type: 'shortcut', requestId, path, name }

// Launcher -> iframe richiedente, dopo il salvataggio effettivo.
{ nas: 1, type: 'shortcut-result', requestId, success, error? }
```

Usare un identificatore per associare risposta e richiesta; nessun bus RPC generico. Il launcher
ricava l'app mittente dal frame e accetta questa operazione per `nas-file-manager`, non si fida
di un ID nel payload. In standalone l'azione NAS non va mostrata come funzionante.

Il launcher salva il collegamento tramite una sua azione store e API. L'app esterna non scrive
direttamente SQLite o `/api/desktop/...`. Aggiungere endpoint per cartelle distinti dagli
endpoint app esistenti, ad esempio POST/DELETE `/api/desktop/folders` con body tipizzato;
documentare il contratto e rigenerare OpenAPI/tipi TS con i comandi già presenti.

- Riutilizzare `kind: 'folder'`, `target` come percorso logico e `name` della tabella esistente.
  Per questo task tutte le cartelle si aprono in `nas-file-manager`: non creare un sistema
  multi-provider dei collegamenti e non aggiungere una tabella senza necessità.
- Aggiunta idempotente, validazione nome/percorso, controllo Explorer installato, errori JSON
  tipizzati. Confermare il successo solo dopo persistenza; in caso di errore niente icona finta.
- La validazione API tratta il body come non affidabile. Non interpolare nomi o percorsi in
  comandi shell. Il launcher conserva percorsi logici; l'accesso al filesystem resta ad Explorer.
- Non toccare `0004_desktop_shortcuts.sql`: aggiungere una migrazione solo se emerge un campo
  realmente indispensabile. Aggiornare repository/service secondo le convenzioni del progetto.

### 14.6 Desktop misto e apertura

Modificare `src/stores/appsStore.ts`, `src/components/DesktopGrid.vue`,
`src/components/desktop/GridItem.vue` e `ShortcutContextMenu.vue` dove necessario:

- Esponi voci desktop tipizzate app/cartella senza perdere nome, icona, tipo o percorso.
  Identità stabile comprensiva del tipo, distinta dall'ID app; collisioni tra path e app impossibili.
- Cartella: icona SVG, etichetta salvata, apertura tramite `nas-file-manager` e query condivisa.
  App: comportamento attuale. La rimozione di un collegamento non elimina mai la cartella reale.
- Adattare riordino, menu contestuale, tastiera e drag già esistenti a una lista mista. L'attuale
  `Order` accetta solo ID app e li limita a 64 caratteri: non riutilizzarlo per path arbitrari.
  Introdurre chiavi tipizzate `{ kind, target }`, preservando il payload app-only attuale se
  ancora usato; aggiornare validazione, store, API e test nella stessa modifica.
- Definire il caso Explorer disinstallato: conservare il collegamento salvato senza cancellare
  dati utente, mostrare indisponibilità all'apertura e consentirne comunque la rimozione. Con
  Explorer reinstallato deve funzionare di nuovo. Non avviare un'installazione automatica.
- Nessun desktop con cartelle contenitore o drag di file reali: è fuori dal task.

## Ordine operativo consigliato

1. Rilevare baseline e riferimenti, fissare la lista delle differenze ammesse; non aggiornare
   lock o formattare prima di aver finito l'inventario.
2. Migrare test/scaffolding e correggere riferimenti mancanti su tutte le app; poi unificare
   package, tsconfig, formatter e lint. Verificare le suite e i tipi prima di cambiare UI.
3. Uniformare Vite/PostCSS/strip test id e asset, con confine Tauri Explorer preservato.
4. Uniformare release e workflow insieme ai comandi che chiamano; provare il caso senza tag
   in repo temporanee. Allineare Sonar/Podman ai percorsi ormai definitivi.
5. Estrarre il bridge comune, uniformare tema/i18n e SCSS, verificando Log e gli stati visuali.
6. Completare prima URL e packaging Explorer, poi provider di ricerca e deep link, infine
   richiesta collegamento e desktop misto. Ogni passaggio con test del contratto interessato.
7. Aggiornare documentazione e manifest finali, eseguire verifiche congiunte e confronto residuo.

## Verifiche e criteri di consegna

I comandi seguenti sono destinati all'implementazione, non risultano eseguiti nella preparazione
di questo piano. Non confondere `release:merge` con un comando di test.

### Gate per ciascuna app

- Installazione riproducibile da lock (`npm ci`), senza dipendenze risolte dalla directory madre.
- `format:check`, `lint`, `type-check`, `test:unit`, `test:component`, `test:e2e`, `test:coverage`.
- `cargo fmt --manifest-path backend/Cargo.toml --all --check`;
  `cargo clippy --manifest-path backend/Cargo.toml --all-targets -- -D warnings`;
  `cargo test --locked --manifest-path backend/Cargo.toml`.
- Per Explorer verificare esplicitamente i default-members `core` e `axum`; i target nativi
  interessati dalle modifiche vanno verificati anche nel loro ambiente, oppure indicati come
  non verificati se mancano i prerequisiti. Non dichiarare un pass globale coprendo solo Axum.
- `build`, `nas:package`, `nas:test` sul nuovo archivio: checksum, manifest, binario eseguibile,
  UI e asset, `/health`, nessun listener TCP, avvio in directory isolata senza sorgenti.
- Report Rust/FE e analisi Sonar quando disponibile; errori e strumenti assenti riportati
  distintamente. Niente `--passWithNoTests` per nascondere glob sbagliati o suite previste vuote.
- Controllo visivo dev e produzione; idempotenza formatter; stesso output dei file comuni
  salvo differenze dichiarate. E2E di produzione usano ruoli/testo, poiché gli ID sono rimossi.

### Regressioni del launcher e integrazione reale

Eseguire il gate previsto da `CLAUDE.md` quando verrà modificato il launcher: format, lint, build,
unit/component/E2E, Cargo fmt/clippy/test. Aggiungere `api:generate`/`api:check` per nuove API;
verificare il gateway quando coinvolto dal percorso di integrazione.

Usare cartelle e database temporanei, una root storage fixture e la forge locale già presente
negli E2E NAS. Non puntare le prove di installazione/disinstallazione ai dati reali dell'utente.
I soli mock del provider non bastano per attestare la compatibilità dell'Explorer vero.

| Scenario                                              | Risultato atteso                                                                                             |
| ----------------------------------------------------- | ------------------------------------------------------------------------------------------------------------ |
| Quattro app installate da archivio/release locale     | Registro, icona, versione, apertura, UI e API coerenti sotto `/apps/<id>/`                                   |
| Aggiornamento e disinstallazione                      | Versione aggiornata, lifecycle corretto, app rimossa da menu/registro; nessuna dipendenza da `external-apps` |
| Tema/lingua/accent cambiano con quattro iframe aperti | Tutte reagiscono; niente richiesta dipendente dal referrer                                                   |
| Notifica -> nas-log-manager                           | Notifica corretta e testi descritti nella lingua corrente                                                    |
| Ricerca globale con Explorer installato               | Provider rilevato, query e filtri corretti, campi e risultati leggibili                                      |
| Explorer fermo o non disponibile                      | Avvio tramite lifecycle dove abilitato, altrimenti errore esplicito; gli altri risultati rimangono           |
| Clic risultato cartella/file                          | Explorer apre la cartella giusta ed eventualmente seleziona il file                                          |
| Clic risultato con Explorer già aperto altrove        | Una sola scheda, percorso aggiornato                                                                         |
| Explorer crea un collegamento                         | Richiesta via bridge, persistenza nel NAS, comparsa immediata sul desktop                                    |
| Reload/riavvio e riordino desktop misto               | Cartelle e app conservano destinazione, nome e ordine                                                        |
| Rimozione collegamento                                | Solo icona rimossa, filesystem invariato                                                                     |
| Explorer disinstallato/reinstallato                   | Collegamento gestibile e indisponibilità chiara; torna apribile dopo reinstallazione                         |
| Percorsi particolari, inesistenti o fuori root        | Encoding coerente, errore controllato, nessuna apertura del percorso sbagliato                               |
| PostMessage da origine/finestra/payload non validi    | Nessuna scrittura desktop o alterazione del contesto                                                         |
| Primo rilascio senza tag                              | Tag candidato dalla versione package; changelog valido; nessuna mutazione nel dry-run                        |

Test di lifecycle reale systemd richiedono un ambiente NAS di prova. Dove viene usato un processo
manuale su socket o una fixture lifecycle, dichiararlo: non equivale alla verifica dell'unit sul NAS.

### Informazioni da lasciare al termine dell'implementazione

1. Matrice finale dei file comuni e delle differenze residue, spiegando quelle funzionali
   (Tauri/core Explorer, provider ricerca, API dischi, integrazione notifiche), non solo nome/versione.
2. Lista dei comandi eseguiti per ogni app e per il NAS, esiti reali ed eventuali verifiche non
   possibili; distinguere regressioni introdotte da problemi già presenti nella baseline.
3. Evidenza di pacchetti autonomi e dei flussi ricerca -> Explorer -> collegamento -> Explorer.
4. Conferma che `external-apps/` non è staged/tracked nel repository NAS e che nessun file
   produttivo dipende dal suo percorso temporaneo. Non rimuovere la cartella per dimostrarlo:
   provare i pacchetti in una directory temporanea separata.
5. Nessuna pubblicazione, commit o tag nel progetto reale eseguiti come effetto collaterale dei test.
