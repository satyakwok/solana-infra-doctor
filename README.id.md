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

[![CI](https://github.com/satyakwok/solana-infra-doctor/actions/workflows/ci.yml/badge.svg)](https://github.com/satyakwok/solana-infra-doctor/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/satyakwok/solana-infra-doctor/branch/main/graph/badge.svg)](https://codecov.io/gh/satyakwok/solana-infra-doctor)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#lisensi)
[![Rust](https://img.shields.io/badge/rust-1.76%2B-orange.svg)](https://www.rust-lang.org/)
[![Status](https://img.shields.io/badge/status-active-blue.svg)](#perintah)

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
- Menghasilkan output terminal yang mudah dibaca, JSON, dan laporan Markdown.

Tool ini local-first, ringan dependensi, dan dibangun di atas HTTP JSON-RPC
mentah via `reqwest`.

## Pratinjau CLI

Bandingkan dua endpoint untuk sebuah workload (`sol-doctor compare --profile bot`)
— endpoint yang lebih cepat tidak otomatis menang kalau slot yang disajikannya
lebih basi:

![tabel ringkasan sol-doctor compare membandingkan dua endpoint RPC mainnet untuk profil bot](https://raw.githubusercontent.com/satyakwok/solana-infra-doctor/main/docs/images/cli/compare.png)

Mendiagnosis satu endpoint (`sol-doctor check`):

![ringkasan kesiapan sol-doctor check untuk satu endpoint RPC mainnet](https://raw.githubusercontent.com/satyakwok/solana-infra-doctor/main/docs/images/cli/check.png)

Memeriksa kesiapan realtime WebSocket (`sol-doctor ws`):

![ringkasan kesiapan sol-doctor ws menampilkan langkah connect, subscribe, dan notifikasi pertama](https://raw.githubusercontent.com/satyakwok/solana-infra-doctor/main/docs/images/cli/ws.png)

Screenshot di atas adalah run nyata terhadap endpoint publik. Nilainya bervariasi
menurut waktu, region, dan kondisi endpoint. Ini adalah snapshot diagnostik,
bukan jaminan dari provider. Tampilan default ringkas; jalankan dengan
`--verbose` untuk detail penuh per-pemeriksaan. Lihat
[CLI Output Guide](docs/cli-output.md) dan
[cara screenshot ini dibuat](docs/readme-screenshots.md).

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

Run nyata terhadap `https://api.mainnet-beta.solana.com`. Tampilan default ringkas
(di terminal akan berwarna; lihat [Pratinjau CLI](#pratinjau-cli)):

```text
Solana Infra Doctor · RPC Readiness

Target
Endpoint   api.mainnet-beta.solana.com

Result
GOOD      All RPC readiness checks passed
Latency   23 ms average
Checks    7 passed · 0 failed

Checks
Category       Status    Summary
Core           PASS      4 / 4
Blockhash      PASS      2 / 2
Performance    PASS      1 / 1

Tip: run with --verbose to see full details.
```

Jalankan dengan `--verbose` untuk detail penuh per-pemeriksaan (URL ter-redaksi
penuh, latency per-method, hash penuh):

```text
Solana Infra Doctor · RPC Readiness

Target
RPC URL   https://api.mainnet-beta.solana.com/

Result
GOOD      All RPC readiness checks passed
Latency   22 ms average
Checks    7 passed · 0 failed

Checks

Core
- getHealth       PASS  86 ms  health is ok
- getVersion      PASS  13 ms  solana-core 4.0.0
- getGenesisHash  PASS  14 ms  5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d
- getSlot         PASS  11 ms  slot 424147058

Blockhash
- getLatestBlockhash  PASS  11 ms  4fzZUYN9uQR6HLTj5faRtJjbiXaLxUfz9k1T2N5ATELG
- isBlockhashValid    PASS  3 ms   latest blockhash is valid

Performance
- getRecentPerformanceSamples  PASS  19 ms  234389 transactions across 154 slots in 60s
```

## Contoh Output Compare

Perbandingan profil `bot` nyata atas dua endpoint mainnet publik. Perhatikan
bahwa endpoint dengan latency lebih rendah (#1) bukan pemenangnya: #2 menyajikan
slot yang lebih segar, yang ditimbang lebih berat oleh profil `bot`. (Jalankan
dengan `--verbose` untuk detail penuh per-endpoint.)

```text
Solana Infra Doctor · RPC Comparison

Profile: bot

RPC   Endpoint                      Verdict   Score    Latency   Slot lag
#1    api.mainnet-beta.solana.com   GOOD      83/100   20 ms     32 behind
#2    solana-rpc.publicnode.com     GOOD      90/100   98 ms     baseline

Recommendation
Best RPC: #2 · solana-rpc.publicnode.com
RPC #2 is recommended for bot workloads.
RPC #1 has lower latency, but RPC #2 is fresher. For bot workloads, slot freshness may matter more than raw HTTP latency.

Tip: run with --verbose to see full details per endpoint.
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

- `check` dan `compare` memakai HTTP JSON-RPC; `sol-doctor ws` hanya mencakup
  kesiapan WebSocket slot-subscription (belum ada subscription account/log/program).
- Pemeriksaan compare saat ini berjalan sekuensial.
- Skor adalah heuristik deterministik, bukan jaminan provider.
- Ini CLI local-first, bukan layanan monitoring terhosting.
- Kesiapan token memastikan akun program SPL Token dan Token-2022 disajikan;
  simulasi transaksi dan pemeriksaan account indexing belum tercakup.
- Belum ada dependensi Solana SDK yang dipakai.
- Tidak ada exporter Prometheus, dashboard, layanan cloud terhosting,
  marketplace, token, NFT, points, airdrop, atau fitur governance.

## Keamanan dan Privasi

Solana Infra Doctor meredaksi kredensial dan kemungkinan API key dari URL RPC yang
ditampilkan, pesan error, output JSON, dan laporan Markdown. Hindari membagikan
URL RPC privat mentah.

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

Berlandaskan kegunaan, bukan jumlah fitur.

**Jangka dekat**

- Mode sampling berulang untuk sinyal latency p50/p95 dan error-rate jendela
  pendek yang lebih baik.
- Template laporan yang lebih kaya.
- Wrapper GitHub Action untuk CI.
- Lebih banyak contoh laporan.

**Nanti**

- File histori benchmark lokal opsional.
- Playbook perbandingan provider.
- Perbaikan instalasi/distribusi.

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
