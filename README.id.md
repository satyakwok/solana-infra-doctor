<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/satyakwok/solana-infra-doctor/main/assets/logo-wordmark-dark.svg" />
    <img alt="Solana Infra Doctor" src="https://raw.githubusercontent.com/satyakwok/solana-infra-doctor/main/assets/logo-wordmark-light.svg" width="640" />
  </picture>
</p>

<p align="center">
  <a href="./README.md">English</a> · <b>Bahasa Indonesia</b>
</p>

# Solana Infra Doctor

[![crates.io](https://img.shields.io/crates/v/solana-infra-doctor.svg)](https://crates.io/crates/solana-infra-doctor)
[![GitHub Marketplace](https://img.shields.io/badge/Marketplace-Solana%20Infra%20Doctor-blue?logo=github)](https://github.com/marketplace/actions/solana-infra-doctor)
[![CI](https://github.com/satyakwok/solana-infra-doctor/actions/workflows/ci.yml/badge.svg)](https://github.com/satyakwok/solana-infra-doctor/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/satyakwok/solana-infra-doctor/branch/main/graph/badge.svg)](https://codecov.io/gh/satyakwok/solana-infra-doctor)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#lisensi)
[![Rust](https://img.shields.io/badge/rust-1.88%2B-orange.svg)](https://www.rust-lang.org/)
[![Status](https://img.shields.io/badge/status-active-blue.svg)](#perintah)
[![Ask DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/satyakwok/solana-infra-doctor)

**CLI Rust yang local-first untuk diagnostik kesiapan produksi RPC Solana, perbandingan endpoint, pemeriksaan WebSocket, dan laporan yang aman-redaksi.**

> Bukan sekadar: *apakah RPC ini online?*
> Tapi: *endpoint RPC Solana mana yang benar-benar layak saya percaya untuk workload ini?*

Solana Infra Doctor mendiagnosis sebuah endpoint RPC Solana, membandingkan
beberapa endpoint, memeriksa kesiapan WebSocket, dan menghasilkan laporan
terminal, JSON, serta Markdown — supaya kamu bisa memutuskan RPC mana yang layak
dipercaya untuk bot, wallet, indexer, pipeline CI, dan tinjauan infrastruktur.

- Mendiagnosis method JSON-RPC inti, kesegaran blockhash, data slot, latency, dan
  performance sample.
- Memeriksa kesiapan SPL Token Program dan Token-2022 (apakah RPC menyajikan
  program token sebagai akun executable) lewat `getAccountInfo`.
- Membandingkan dua endpoint atau lebih dan memberi skor `0`–`100` per endpoint.
- Menyesuaikan skor dengan profil workload: `general`, `wallet`, `bot`,
  `indexer`, `ci`.
- Memeriksa kesiapan WebSocket (waktu ke notifikasi pertama `slotSubscribe`)
  lewat `sol-doctor ws`.
- Memeriksa kesiapan **Yellowstone gRPC** (connect, auth `x-token` opsional,
  unary probe aman, dan stream slot-only) lewat `sol-doctor grpc check`.
- Menghasilkan output terminal yang mudah dibaca, JSON, dan laporan Markdown.

Tool ini local-first dan ringan dependensi: HTTP JSON-RPC via `reqwest`,
WebSocket via `tokio-tungstenite`, dan Yellowstone gRPC via `tonic` dengan
definisi resmi `yellowstone-grpc-proto` (tanpa SDK Solana/Agave lengkap).

## Pratinjau CLI

Bandingkan dua endpoint untuk sebuah workload (`sol-doctor compare --profile bot`)
— endpoint yang lebih cepat tidak otomatis menang kalau slot yang disajikannya
lebih basi:

```text
Solana Infra Doctor · RPC Comparison

Profile: bot

RPC   Endpoint                      Verdict   Score     Latency   Slot lag
#1    api.mainnet-beta.solana.com   GOOD      99/100    16 ms     32 behind
#2    solana-rpc.publicnode.com     GOOD      100/100   105 ms    baseline

Recommendation
Best RPC: #2 · solana-rpc.publicnode.com
RPC #2 is recommended for bot workloads.
RPC #1 has lower latency, but RPC #2 is fresher. For bot workloads, slot freshness may matter more than raw HTTP latency.
```

Mendiagnosis satu endpoint (`sol-doctor check`):

```text
Solana Infra Doctor · RPC Readiness

Target
Endpoint   api.mainnet-beta.solana.com

Result
GOOD         All RPC readiness checks passed
Latency      10 ms average
Checks       11 passed · 0 failed
Block time   13s behind (finalized)
Fee market   median 0 micro-lamports/CU
Token        Token Program ready · Token-2022 ready

Checks
Category       Status    Summary
Core           PASS      4 / 4
Blockhash      PASS      2 / 2
Performance    PASS      3 / 3
Token          PASS      2 / 2
```

Memeriksa kesiapan realtime WebSocket (`sol-doctor ws`):

```text
Solana Infra Doctor · WebSocket Readiness

Target
RPC         api.mainnet-beta.solana.com
WebSocket   wss://api.mainnet-beta.solana.com/

Result
GOOD   WebSocket readiness checks passed

Checks
Check                 Status    Detail
Connect               PASS      94 ms
Subscribe             PASS      slotSubscribe · id 1
First notification    PASS      132 ms · slot 424282423
Unsubscribe           PASS
Close                 PASS
```

Ini run nyata terhadap endpoint publik. Nilainya bervariasi menurut waktu,
region, dan kondisi endpoint; ini snapshot diagnostik, bukan jaminan provider.
Tampilan default ringkas (yang ditampilkan di sini); jalankan dengan `--verbose`
untuk detail penuh per-pemeriksaan (lihat contoh verbose di bawah). Lihat
[CLI Output Guide](docs/cli-output.md).

## Instalasi

### Binary prebuilt (tanpa toolchain Rust)

Setiap rilis melampirkan binary prebuilt untuk Linux (gnu + musl statis), macOS
(Intel + Apple Silicon), dan Windows. Dengan
[`cargo-binstall`](https://github.com/cargo-bins/cargo-binstall):

```bash
cargo binstall solana-infra-doctor
```

Atau unduh arsip untuk platform-mu dari
[rilis terbaru](https://github.com/satyakwok/solana-infra-doctor/releases/latest)
(bernama `sol-doctor-<target>`), ekstrak, dan taruh `sol-doctor` di `PATH`-mu.

### Dari crates.io (kompilasi dari source)

```bash
cargo install solana-infra-doctor
```

Upgrade dengan `cargo install solana-infra-doctor --force` (atau
`cargo binstall --force solana-infra-doctor`).

Verifikasi:

```bash
sol-doctor --version
```

(Atau build dari source — lihat [Instalasi dari Source](#instalasi-dari-source).)

## Siapa yang Sebaiknya Memakai Ini?

| Pengguna | Kenapa penting |
| --- | --- |
| Pembuat bot | Latency dan kesegaran slot bisa memengaruhi kualitas eksekusi. |
| Backend wallet/dApp | Keandalan RPC dan kesiapan blockhash memengaruhi transaksi yang dilihat pengguna. |
| Operator indexer | Kesegaran slot dan ketersediaan data penting untuk pipeline indexing. |
| Tim infra | Bandingkan provider sebelum menyambungkan endpoint ke sistem produksi. |
| Pipeline CI | Pakai output JSON untuk pemeriksaan kesiapan yang deterministik. |
| Konsultan/auditor | Hasilkan laporan aman-redaksi untuk tinjauan kesiapan RPC. |

## Perintah

Periksa satu RPC:

```bash
sol-doctor check --rpc https://api.mainnet-beta.solana.com
```

Bandingkan beberapa endpoint RPC:

```bash
sol-doctor compare \
  --rpc https://api.mainnet-beta.solana.com \
  --rpc https://solana-rpc.publicnode.com \
  --profile bot
```

Hasilkan laporan Markdown:

```bash
sol-doctor compare \
  --rpc https://api.mainnet-beta.solana.com \
  --rpc https://solana-rpc.publicnode.com \
  --profile bot \
  --report rpc-report.md
```

Periksa kesiapan WebSocket:

```bash
sol-doctor ws --rpc https://api.mainnet-beta.solana.com
```

Periksa kesiapan Yellowstone gRPC:

```bash
sol-doctor grpc check --grpc https://example-yellowstone-endpoint
```

Output JSON (machine-readable, untuk CI):

```bash
sol-doctor compare \
  --rpc https://api.mainnet-beta.solana.com \
  --rpc https://solana-rpc.publicnode.com \
  --profile bot \
  --json
```

Mau lihat dulu sebelum menjalankan? Lihat [Contoh Output Tersimpan](#contoh-output-tersimpan)
untuk contoh output terminal dan laporan Markdown.

## Apa yang Diperiksa

| Area | Pemeriksaan |
| --- | --- |
| HTTP JSON-RPC | health, version, genesis hash, slot, blockhash, performance sample |
| Kesegaran & fee | kesegaran block-time (`getBlockTime`), prioritization fee terbaru |
| Mode compare | skor, latency, kesegaran slot, kesegaran block-time, pemeriksaan gagal, endpoint terbaik/terburuk |
| Keamanan jaringan | menolak perbandingan lintas-jaringan berdasarkan genesis hash |
| WebSocket | derivasi URL, connect, `slotSubscribe`/`logsSubscribe`, notifikasi pertama, reconnect, unsubscribe, close |
| Yellowstone gRPC | connect + TLS/HTTP-2, auth `x-token` opsional, unary probe (`Ping`/`GetVersion`/`GetSlot`/`GetBlockHeight`/`GetLatestBlockhash`/`IsBlockhashValid`), first-event stream slot-only, cross-check slot HTTP RPC opsional |
| Keamanan output | meredaksi kredensial dan kemungkinan API key di terminal, JSON, Markdown, dan error |

## Profil Workload

| Profil | Use case | Dioptimalkan untuk |
|---|---|---|
| `general` | Diagnostik default | pemeriksaan seimbang |
| `wallet` | Wallet dan dApp | keandalan dan kesiapan blockhash |
| `bot` | Bot dan otomasi | latency dan kesegaran slot |
| `indexer` | Indexer/pipeline data | slot lag dan ketersediaan data |
| `ci` | Pemeriksaan CI/deploy | perilaku pass/fail yang deterministik |

## Kenapa Ini Ada

Sebuah endpoint RPC Solana bisa saja terjangkau tapi tetap tidak cocok untuk
workload nyata. Pemeriksaan uptime sederhana tidak memberitahumu apakah:

- method JSON-RPC inti benar-benar berfungsi
- blockhash terbaru bisa dipakai
- data slot masih segar
- latency masih dapat diterima
- performance sample tersedia
- satu endpoint lebih baik dari yang lain untuk workload tertentu

Solana Infra Doctor menjawab pertanyaan-pertanyaan itu dengan diagnostik lokal
yang cepat, yang bisa kamu jalankan sebelum menyambungkan endpoint ke kode
aplikasi, job CI, otomasi infrastruktur, atau runbook operasional.

## Detail Pemeriksaan (HTTP JSON-RPC)

`sol-doctor check` menjalankan pemeriksaan JSON-RPC berikut:

| Kategori | Method | Tujuan |
| --- | --- | --- |
| Core | `getHealth` | Memastikan node melaporkan status sehat. |
| Core | `getVersion` | Memastikan metadata versi software validator tersedia. |
| Core | `getGenesisHash` | Memastikan endpoint dapat mengidentifikasi genesis hash cluster-nya. |
| Core | `getSlot` | Memastikan endpoint dapat mengembalikan data slot terkini. |
| Blockhash | `getLatestBlockhash` | Memastikan endpoint dapat menghasilkan blockhash terbaru. |
| Blockhash | `isBlockhashValid` | Memastikan blockhash terbaru yang dikembalikan valid. |
| Performance | `getRecentPerformanceSamples` | Memastikan data performance sample terbaru tersedia. |
| Performance | `getBlockTime` | Mengukur seberapa jauh waktu block finalized terbaru tertinggal dari jam dinding (sinyal kesegaran yang dipakai dalam scoring). |
| Performance | `getRecentPrioritizationFees` | Menampilkan median priority fee terbaru sebagai konteks fee-market (chain-wide, bukan sinyal skor per-endpoint). |
| Token | `getAccountInfo` | Memastikan akun SPL Token Program disajikan sebagai program executable. |
| Token | `getAccountInfo` | Memastikan akun program Token-2022 (Token Extensions) disajikan sebagai program executable. |

Pemeriksaan `Token` memastikan endpoint menyajikan program token kanonik
(`Tokenkeg…` dan `TokenzQd…`) sebagai akun executable — kesiapan yang
diandalkan kebanyakan workload yang menyentuh token (wallet, trading bot, token
indexer). Pemeriksaan ini bersifat informasional: kegagalannya membatasi verdict
maksimal di `WARNING`, bukan `BAD`, dan scoring profil memberi nilai untuk
kesiapan token pada profil `wallet`, `bot`, dan `indexer`. Lihat
[`examples/reports/token-readiness-report.md`](examples/reports/token-readiness-report.md)
untuk perbandingan nyata.

CLI mengukur latency tiap method dan menghitung verdict latency rata-rata dengan
ambang berikut:

- `GOOD`: latency rata-rata kurang dari atau sama dengan 500ms.
- `WARNING`: latency rata-rata lebih dari 500ms dan kurang dari atau sama dengan
  1500ms.
- `BAD`: latency rata-rata lebih dari 1500ms atau terjadi timeout berulang.

Jenis error diklasifikasikan sebagai:

- `invalid_url`
- `timeout`
- `http_error`
- `rpc_error`
- `malformed_response`
- `unknown_error`

## Instalasi dari Source

```bash
git clone https://github.com/satyakwok/solana-infra-doctor.git
cd solana-infra-doctor
cargo build --release
```

Binary dibangun di:

```bash
./target/release/sol-doctor
```

## Penggunaan

Periksa sebuah endpoint RPC:

```bash
sol-doctor check --rpc https://api.mainnet-beta.solana.com
```

Tampilkan detail penuh per-pemeriksaan (URL ter-redaksi penuh, latency per-method,
hash penuh):

```bash
sol-doctor check --rpc https://api.mainnet-beta.solana.com --verbose
```

Hasilkan JSON (untuk otomasi — utamakan ini daripada mem-parse teks human):

```bash
sol-doctor check --rpc https://api.mainnet-beta.solana.com --json
```

Probe latency beberapa kali dan laporkan persentil (satu sample menyembunyikan
tail latency):

```bash
sol-doctor check --rpc https://api.mainnet-beta.solana.com --samples 20
```

Ini menambah baris `Samples` (`p50 … · p95 …`) ke output human dan objek
`latency_samples` ke JSON. Default-nya satu sample, dan `--samples` tidak
mengubah verdict.

Pakai timeout per-request kustom:

```bash
sol-doctor check --rpc https://api.mainnet-beta.solana.com --timeout-ms 3000
```

Buat perilaku warning eksplisit untuk CI:

```bash
sol-doctor check --rpc https://api.mainnet-beta.solana.com --fail-on-warning
```

`--fail-on-warning` tidak mengubah pemetaan exit code. `WARNING` tetap keluar
dengan kode `1`; output-nya hanya membuat policy CI menjadi eksplisit.

Bandingkan dua endpoint RPC atau lebih:

```bash
sol-doctor compare \
  --rpc https://api.mainnet-beta.solana.com \
  --rpc https://example-rpc-provider.com
```

Bandingkan endpoint untuk profil workload tertentu:

```bash
sol-doctor compare \
  --rpc https://api.mainnet-beta.solana.com \
  --rpc https://example-rpc-provider.com \
  --profile bot
```

Hasilkan hasil compare sebagai JSON:

```bash
sol-doctor compare \
  --rpc https://api.mainnet-beta.solana.com \
  --rpc https://example-rpc-provider.com \
  --json
```

Tulis laporan perbandingan Markdown:

```bash
sol-doctor compare \
  --rpc https://api.mainnet-beta.solana.com \
  --rpc https://example-rpc-provider.com \
  --profile indexer \
  --report rpc-report.md
```

Mode compare mendukung profil berikut:

| Profil | Use case |
| --- | --- |
| `general` | Scoring default seimbang untuk kesiapan produksi umum. |
| `wallet` | Menekankan keberhasilan RPC inti dan validitas blockhash terbaru. |
| `bot` | Menghukum latency tinggi dan slot lag besar untuk workload yang sensitif latency. |
| `indexer` | Menghukum slot lag dan performance sample terbaru yang tidak tersedia. |
| `ci` | Memakai teks rekomendasi yang ketat untuk keputusan pass-gate. |

Mode compare membantu memilih endpoint RPC untuk workload wallet, bot, indexer,
dan CI dengan memberi skor tiap endpoint dari `0` sampai `100`, menghitung slot
lag relatif terhadap endpoint paling segar yang diamati, mendaftar pemeriksaan
yang gagal, dan merekomendasikan endpoint terbaik dan terburuk. Endpoint
diperiksa secara **konkuren**, jadi run-nya kira-kira selama endpoint paling
lambat, bukan jumlah dari semuanya.

HTTP client-nya juga resilient: tiap endpoint dibatasi rate-nya (agar sopan
terhadap RPC publik) dan kegagalan transien (timeout, error koneksi, HTTP 429)
di-retry dengan exponential backoff. Tidak satu pun dari ini mengubah CLI,
verdict, atau bentuk output.

Mode compare ditujukan untuk endpoint pada jaringan Solana yang sama. Jika
endpoint mengembalikan genesis hash berbeda, Solana Infra Doctor menolak
perbandingan karena slot lag dan ranking tidak bermakna lintas jaringan.

Diagnosis kesiapan WebSocket untuk workload realtime:

```bash
sol-doctor ws --rpc https://api.mainnet-beta.solana.com
```

`ws` menurunkan URL WebSocket dari URL HTTP RPC (`https` → `wss`,
`http` → `ws`), connect, subscribe dengan `slotSubscribe`, mengukur waktu ke
notifikasi pertama, unsubscribe, dan close. Override URL turunan dengan
`--ws wss://...` ketika provider memakai host WebSocket terpisah, dan hasilkan
JSON dengan `--json`.

Jika koneksi gagal terbentuk atau putus sebelum notifikasi pertama, `ws`
**reconnect dengan exponential backoff** (sampai beberapa percobaan) sebelum
menyerah; jumlah reconnect dilaporkan. Pilih subscription berbeda untuk diuji
dengan `--subscription` (`slot`, default, atau `logs`):

```bash
sol-doctor ws --rpc https://api.mainnet-beta.solana.com --subscription logs
```

### Kesiapan Yellowstone gRPC

Periksa apakah sebuah endpoint Yellowstone gRPC terjangkau, terautentikasi,
responsif, dan men-stream data slot yang segar:

```bash
sol-doctor grpc check --grpc https://example-yellowstone-endpoint
```

Kebanyakan endpoint Yellowstone butuh `x-token`. Berikan lewat **variabel
lingkungan** — token tidak pernah diterima langsung di command line dan tidak
pernah dicetak, diserialkan, atau di-log:

```bash
export YELLOWSTONE_X_TOKEN="token-kamu"

sol-doctor grpc check \
  --grpc https://example-yellowstone-endpoint \
  --x-token-env YELLOWSTONE_X_TOKEN
```

Opsional, cross-check slot terbaru stream gRPC terhadap endpoint HTTP RPC
(memakai ulang RPC client yang aman-redaksi):

```bash
sol-doctor grpc check \
  --grpc https://example-yellowstone-endpoint \
  --x-token-env YELLOWSTONE_X_TOKEN \
  --rpc https://api.mainnet-beta.solana.com
```

`grpc check` memvalidasi & meredaksi URL gRPC, connect (TLS + HTTP/2 untuk
`https`), melampirkan `x-token` hanya bila diberikan, menjalankan **unary** probe
aman (`Ping`, `GetVersion`, `GetSlot`, `GetBlockHeight`, `GetLatestBlockhash`,
`IsBlockhashValid`), lalu membuka stream `Subscribe` **slot-only yang sempit**
untuk mengukur waktu ke update slot pertama dan slot terbaru yang teramati. Aman
secara default: tidak pernah mengirim transaksi, tidak mengubah state remote,
tidak subscribe ke akun/transaksi/blok, dan membatasi setiap koneksi, request,
dan stream dengan deadline.

Method yang mengembalikan `UNIMPLEMENTED` dianggap kapabilitas opsional (`SKIP`),
bukan kegagalan, karena sebagian deployment Yellowstone hanya menyediakan stream
`Subscribe`. Verdict ditentukan oleh transport, autentikasi, dan stream slot;
pemeriksaan unary yang terdegradasi atau selisih slot besar adalah `WARNING`,
bukan `BAD`.

Opsi:

| Flag | Tujuan |
| --- | --- |
| `--grpc <URL>` | Endpoint Yellowstone gRPC (`http`/`https`). Wajib. |
| `--x-token-env <ENV>` | Baca `x-token` dari variabel lingkungan ini. |
| `--rpc <URL>` | Endpoint HTTP RPC opsional untuk cross-check kesegaran slot. |
| `--timeout-ms <MS>` | Timeout koneksi & per-request (default `10000`). |
| `--duration <MS>` | Jendela observasi stream slot (default `5000`). |
| `--json` | JSON machine-readable (termasuk `schema_version`). |
| `--report <PATH>` | Tulis laporan Markdown. |
| `--verbose` | Tampilkan detail per-method, cross-check, dan hint remediasi. |

Hasilkan JSON atau tulis laporan Markdown:

```bash
sol-doctor grpc check --grpc https://example-yellowstone-endpoint --json
sol-doctor grpc check --grpc https://example-yellowstone-endpoint --report yellowstone-grpc-report.md
```

Contoh output human (struktur ditampilkan; nilai bervariasi per endpoint dan
waktu):

```text
Solana Infra Doctor · Yellowstone gRPC Readiness

Target
Endpoint     example-yellowstone-endpoint

Result
GOOD         Yellowstone gRPC endpoint is ready
Connect      42 ms
Unary        6 passed · 0 failed
Stream       first slot update in 318 ms
Latest slot  424,000,123

Checks
Category         Status    Summary
Transport        PASS      Connected over TLS (HTTP/2)
Authentication   PASS      Token accepted
Unary            PASS      6 / 6 supported checks passed
Stream           PASS      first slot update in 318 ms
Freshness        PASS      Slot stream is active

Tip: run with --verbose to see full details.
```

Kode exit mengikuti pemetaan yang sama dengan command lain (lihat
[Kode Exit](#kode-exit)): `0` GOOD, `1` WARNING, `2` BAD, `3` UNKNOWN/error.

### Output berwarna

Output human diberi warna ketika stdout adalah terminal. Warnanya **semantik**:
verdict dan penanda `PASS`/`WARN`/`FAIL` membawa warna status, label diredam, dan
judul bagian ditekankan. Kontrol dengan flag global `--color` (`check`,
`compare`, dan `ws` semuanya menerimanya):

```bash
sol-doctor check --rpc https://api.mainnet-beta.solana.com --color auto    # default: warna hanya di TTY
sol-doctor check --rpc https://api.mainnet-beta.solana.com --color always  # paksa warna (mis. pipe ke pager yang me-render ANSI)
sol-doctor check --rpc https://api.mainnet-beta.solana.com --color never   # matikan warna
```

Output `--json` tidak pernah diberi warna; variabel lingkungan
[`NO_COLOR`](https://no-color.org/) dan `TERM=dumb` dihormati di bawah
`--color auto`. Ketika warna mati, output-nya identik byte-per-byte dengan output
tanpa warna. Lihat [CLI Output Guide](docs/cli-output.md) untuk referensi output
lengkap.

## Contoh Output Human

Run `--verbose` nyata terhadap `https://api.mainnet-beta.solana.com`, menampilkan
detail penuh per-pemeriksaan (URL ter-redaksi penuh, latency per-method, hash
penuh). Versi ringkas default ada di [Pratinjau CLI](#pratinjau-cli) di atas:

```text
Solana Infra Doctor · RPC Readiness

Target
RPC URL   https://api.mainnet-beta.solana.com/

Result
GOOD         All RPC readiness checks passed
Latency      18 ms average
Checks       11 passed · 0 failed
Block time   16s behind (finalized)
Fee market   median 0 micro-lamports/CU
Token        Token Program ready · Token-2022 ready

Checks

Core
- getHealth       PASS  35 ms  health is ok
- getVersion      PASS  9 ms   solana-core 4.0.0
- getGenesisHash  PASS  24 ms  5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d
- getSlot         PASS  5 ms   slot 424282448

Blockhash
- getLatestBlockhash  PASS  2 ms  FzsSsc1FBjsERVk6ZqJpqtCKSBLG7GywRFFNb2yBmLAz
- isBlockhashValid    PASS  5 ms  latest blockhash is valid

Performance
- getRecentPerformanceSamples  PASS  67 ms  214347 transactions across 152 slots in 60s
- getBlockTime                 PASS  21 ms  finalized block time 16s behind wall clock
- getRecentPrioritizationFees  PASS  11 ms  median priority fee 0 micro-lamports/CU (max 0)

Token
- getAccountInfo  PASS  8 ms   Token Program ready: executable 36-byte program owned by BPFLoaderUpgradeab1e11111111111111111111111
- getAccountInfo  PASS  19 ms  Token-2022 ready: executable 36-byte program owned by BPFLoaderUpgradeab1e11111111111111111111111
```

## Contoh Output Compare

Perbandingan profil `bot` nyata (`--verbose`) atas dua endpoint mainnet publik,
dengan detail penuh per-endpoint. Endpoint dengan latency lebih rendah (#1) bukan
pemenangnya: #2 menyajikan slot yang lebih segar, yang ditimbang lebih berat oleh
profil `bot`. (Tabel ringkasnya ada di [Pratinjau CLI](#pratinjau-cli) di atas.)

```text
Solana Infra Doctor · RPC Comparison

Profile: bot

RPC #1
URL                   https://api.mainnet-beta.solana.com/
Genesis               5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d
Verdict               GOOD
Score                 99/100
Slot                  424282690
Slot lag              32 slots behind
Average latency       18 ms
Block time lag        15s behind
Median priority fee   0 micro-lamports/CU
Token Program         ready
Token-2022            ready
Failed checks         none
Blockhash valid       yes

RPC #2
URL                   https://solana-rpc.publicnode.com/
Genesis               5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d
Verdict               GOOD
Score                 100/100
Slot                  424282722
Slot lag              baseline
Average latency       95 ms
Block time lag        2s behind
Median priority fee   0 micro-lamports/CU
Token Program         ready
Token-2022            ready
Failed checks         none
Blockhash valid       yes

Recommendation
Best RPC: #2 · solana-rpc.publicnode.com
RPC #2 is recommended for bot workloads.
RPC #1 has lower latency, but RPC #2 is fresher. For bot workloads, slot freshness may matter more than raw HTTP latency.
```

## Contoh Output JSON

```json
{
  "verdict": "GOOD",
  "rpc_url": "https://api.mainnet-beta.solana.com/",
  "summary": "All RPC readiness checks passed",
  "average_latency_ms": 42,
  "fail_on_warning": false,
  "checks": [
    {
      "category": "core",
      "method": "getHealth",
      "status": "success",
      "latency_ms": 35,
      "detail": "health is ok",
      "error_kind": null,
      "critical": true
    },
    {
      "category": "blockhash",
      "method": "isBlockhashValid",
      "status": "success",
      "latency_ms": 40,
      "detail": "latest blockhash is valid",
      "error_kind": null,
      "critical": true
    },
    {
      "category": "performance",
      "method": "getRecentPerformanceSamples",
      "status": "success",
      "latency_ms": 47,
      "detail": "124000 transactions across 64 slots in 60s",
      "error_kind": null,
      "critical": false
    }
  ]
}
```

## Contoh Output JSON Compare

```json
{
  "profile": "bot",
  "endpoints": [
    {
      "index": 1,
      "url": "https://api.mainnet-beta.solana.com/",
      "verdict": "GOOD",
      "score": 90,
      "slot": 347000000,
      "slot_lag": 0,
      "average_latency_ms": 142,
      "failed_checks": [],
      "blockhash_valid": true,
      "notes": []
    },
    {
      "index": 2,
      "url": "https://***.provider.com/",
      "verdict": "WARNING",
      "score": 15,
      "slot": 346999700,
      "slot_lag": 300,
      "average_latency_ms": 812,
      "failed_checks": ["getRecentPerformanceSamples"],
      "blockhash_valid": true,
      "notes": [
        "Average latency is high for latency-sensitive bot workloads.",
        "Slot lag is high for slot-sensitive bot workloads."
      ]
    }
  ],
  "best_endpoint_index": 1,
  "worst_endpoint_index": 2,
  "recommendation": "Best RPC: RPC #1\nWorst RPC: RPC #2\nRPC #1 is recommended for bot workloads.\nAvoid RPC #2 for latency-sensitive or slot-sensitive workloads."
}
```

## Contoh Laporan Markdown

```markdown
# Solana Infra Doctor RPC Compare Report

Profile: `indexer`

## Summary

- Best RPC: RPC #1
- Worst RPC: RPC #2

## Comparison

| RPC | URL | Verdict | Score | Slot | Slot lag | Average latency | Failed checks | Blockhash valid |
| --- | --- | --- | ---: | --- | --- | --- | --- | --- |
| RPC #1 | `https://api.mainnet-beta.solana.com/` | `GOOD` | 90/100 | 347000000 | baseline | 142ms | none | yes |
```

## Contoh Output Tersimpan

Contoh output di-commit di [`examples/`](examples/) supaya kamu bisa memeriksa apa
yang dihasilkan tool tanpa harus menjalankannya dulu. Ini adalah run diagnostik
ilustratif, bukan benchmark provider.

- [`examples/terminal/check-mainnet.txt`](examples/terminal/check-mainnet.txt)
  — output terminal `check` satu-RPC.
- [`examples/terminal/compare-bot.txt`](examples/terminal/compare-bot.txt)
  — output terminal `compare` untuk profil `bot`.
- [`examples/terminal/ws-mainnet.txt`](examples/terminal/ws-mainnet.txt)
  — output terminal kesiapan WebSocket `ws`.
- [`examples/reports/compare-bot-report.md`](examples/reports/compare-bot-report.md)
  — laporan perbandingan Markdown untuk profil `bot`.
- [`examples/reports/compare-indexer-report.md`](examples/reports/compare-indexer-report.md)
  — laporan perbandingan Markdown untuk profil `indexer`.
- [`examples/mixed-network-rejection.md`](examples/mixed-network-rejection.md)
  — bagaimana compare menolak endpoint dari jaringan Solana berbeda.
- [`examples/reports/compare-bot-live.md`](examples/reports/compare-bot-live.md)
  — perbandingan profil `bot` nyata atas dua endpoint mainnet publik.

Laporan-laporan ini berguna sebagai sinyal kesiapan untuk perbandingan RPC,
tinjauan kesiapan bot/indexer, diskusi CI, dan diagnostik bergaya konsultasi.
Skornya adalah heuristik deterministik, bukan jaminan perilaku provider.

## Kode Exit

| Kode | Verdict | Arti |
| --- | --- | --- |
| `0` | `GOOD` | Pemeriksaan wajib lolos dan latency dapat diterima. |
| `1` | `WARNING` | Endpoint terjangkau, tapi latency meninggi atau satu pemeriksaan non-kritis gagal. |
| `2` | `BAD` | URL tidak valid, endpoint tidak terjangkau, pemeriksaan kritis gagal, timeout berulang, atau latency terlalu tinggi. |
| `3` | `UNKNOWN` atau error internal | Data tidak cukup untuk verdict yang andal, atau terjadi error internal tak terduga. |

## Pakai di CI (GitHub Action)

Gate sebuah workflow berdasarkan kesiapan RPC dengan composite action bawaan — ia
meng-install `sol-doctor` dan menjalankannya, sehingga job gagal ketika endpoint
belum siap:

```yaml
- name: Check Solana RPC readiness
  uses: satyakwok/solana-infra-doctor@v1
  with:
    rpc: https://api.mainnet-beta.solana.com
    fail-on-warning: "true"
```

Input: `command` (`check`/`ws`/`compare`, default `check`), `rpc`,
`fail-on-warning`, `samples`, `timeout-ms`, `json`, `verbose`, `version`, dan
`args` (passthrough mentah — mis. `--rpc` tambahan untuk `compare`). Keberhasilan
job mengikuti [kode exit](#kode-exit) di atas, jadi endpoint `BAD` (atau
`WARNING` dengan `fail-on-warning`) menggagalkan step. Pakai moving major tag
`@v1`, atau pin tag rilis spesifik (mis. `@v0.9.0`) untuk run yang sepenuhnya
reproducible.

> `fail-on-warning` dan `samples` hanya berlaku untuk `command: check`.

## Batasan Saat Ini

- `check` dan `compare` memakai HTTP JSON-RPC; `sol-doctor ws` mencakup kesiapan
  subscription slot dan logs (belum ada subscription account/program).
- `grpc check` adalah pemeriksaan kesiapan satu endpoint yang subscribe **hanya**
  ke slot; perbandingan endpoint gRPC dan diagnostik subscription lain belum
  termasuk. Ini diagnostik point-in-time, bukan benchmark atau SLA.
- Skor adalah heuristik deterministik, bukan jaminan provider.
- Ini CLI local-first, bukan layanan monitoring terhosting.
- Kesiapan token memastikan akun program SPL Token dan Token-2022 disajikan;
  simulasi transaksi dan pemeriksaan account indexing belum tercakup.
- Tidak ada SDK Solana atau Agave lengkap yang dipakai; satu-satunya crate Solana
  yang ikut adalah `solana-pubkey` yang ringan (transitif, via definisi proto gRPC).
- Tidak ada exporter Prometheus, dashboard, layanan cloud terhosting,
  marketplace, token, NFT, points, airdrop, atau fitur governance.

## Keamanan dan Privasi

Solana Infra Doctor meredaksi kredensial dan kemungkinan API key dari URL RPC dan
gRPC yang ditampilkan, pesan error, output JSON, dan laporan Markdown. Hindari
membagikan URL RPC privat mentah.

`x-token` Yellowstone gRPC dibaca **hanya** dari variabel lingkungan yang disebut
oleh `--x-token-env` (tidak pernah dari argumen command line) dan tidak pernah
dicetak, diserialkan ke JSON, ditulis ke laporan, atau di-log. Variabel token
yang kosong/tidak diset dilaporkan sebagai error konfigurasi lokal sebelum koneksi
dicoba.

## Use Case Praktis

Solana Infra Doctor dapat menghasilkan artefak diagnostik aman-redaksi untuk:

- perbandingan provider RPC sebelum memilih endpoint
- tinjauan kesiapan bot/indexer
- pemeriksaan RPC wallet/backend
- pemeriksaan kesiapan CI (output JSON, exit code)
- laporan audit RPC teknis singkat

Untuk contoh nyata mengubah output CLI menjadi laporan yang bisa dibagikan, lihat
[`docs/rpc-readiness-report.md`](docs/rpc-readiness-report.md).

Repositori ini tidak menyediakan monitoring terhosting, SaaS berbayar, atau
jaminan SLA. Ini adalah tool diagnostik lokal.

## Yang Bukan Ini

- Bukan layanan monitoring terhosting
- Bukan penyedia SLA
- Bukan pengganti observability provider
- Bukan jaminan performa trading
- Bukan produk dashboard atau SaaS
- Tidak berafiliasi dengan atau didukung oleh Solana Foundation

## Kebijakan Coverage

CI menegakkan minimal `95%` line coverage dengan `cargo llvm-cov`. Laporan
coverage dihasilkan sebagai `lcov.info`, diunggah ke Codecov, dan diabaikan
secara lokal supaya artefak laporan tidak ikut di-commit.

## Roadmap

Berlandaskan kegunaan, bukan jumlah fitur. Lihat
[`docs/roadmap.md`](docs/roadmap.md) untuk daftar milestone lengkap dan batas
ruang lingkup.

**Baru saja dirilis**

- **Pemeriksaan kesiapan Yellowstone gRPC** (`grpc check`).
- Mode sampling berulang (`--samples`) dengan persentil latency p50/p95.
- Pemeriksaan kesiapan SPL Token dan Token-2022.
- Wrapper GitHub Action dan binary prebuilt (`cargo binstall`) untuk CI dan
  instalasi mudah.

**Jangka dekat**

- Perbandingan endpoint Yellowstone gRPC (rank beberapa endpoint gRPC).
- Laporan Markdown untuk `check` (saat ini hanya `compare` dan `grpc check` yang
  menghasilkannya) dan template laporan yang lebih kaya.
- Lebih banyak contoh laporan dan dokumentasi terlokalisasi.

**Nanti**

- Subscription WebSocket tambahan (account/program), di luar slot dan logs.
- File histori benchmark lokal opsional.
- Playbook perbandingan provider.

**Belum sekarang**

- Dashboard terhosting, SaaS, akun pengguna, database, alerting, atau API
  berbayar.

## Lisensi

Proyek ini dilisensikan di bawah salah satu dari:

- Apache License, Version 2.0
- MIT License

sesuai pilihanmu.

Copyright 2026 Satya Kwok.

## Disclaimer

Solana Infra Doctor adalah tool open-source independen dan tidak berafiliasi
dengan atau didukung oleh Solana Foundation.
