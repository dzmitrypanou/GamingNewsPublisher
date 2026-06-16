# Gaming News Publisher

Desktop app for collecting gaming news from RSS feeds, rewriting posts with AI, deduplicating similar stories locally, and publishing to VK and Telegram.

Built with **Tauri 2** (Rust backend) and **React 19** (TypeScript + Tailwind CSS).

## Features

### News pipeline
- **RSS ingestion** — configurable sources with built-in presets (IGN, PC Gamer, Eurogamer, and others)
- **Parallel fetch** — concurrent sources and items with cancellation support
- **Article enrichment** — optional full article text from source pages (no Tavily; article-only web context)
- **Content filtering** — excludes puzzles, navigation boilerplate, non-gaming junk
- **Image processing** — hero image extraction, 1280×720 cover fit, source-specific crops (e.g. IGN)

### AI rewriting
- **Cloud (DeepSeek)** — recommended for generation: short headline, social text, hashtags
- **Local LLM (GGUF)** — optional offline generation via `llama-server` (Vikhr-Nemo 12B, Qwen2.5 14B)
- **Configurable prompt** — language, tone, and field format (JSON output)
- **Queue workflow** — fetch → AI process → review → approve → publish

### Duplicate detection
Local embedding models compare new headlines against recent posts before they enter the queue.

**Pipeline (per incoming item):**
1. Exact normalized title match
2. Lexical headline match (strong / medium overlap, model-specific rules)
3. Embedding similarity on top-ranked candidates (min. 50, up to configured limit)
4. **Post-fetch sweep** — second pass over posts inserted in the same batch (fixes parallel-ingest races)

**Available dedup encoders:**

| Model | ID | Size | Notes |
|-------|-----|------|--------|
| Multilingual E5 Base | `multilingual-e5-base` | ~220 MB | Fast default, tuned thresholds |
| Multilingual E5 Large | `multilingual-e5-large` | ~400 MB | Better on long / rephrased headlines |
| BGE-M3 | `bge-m3` | ~440 MB | Strongest semantic matching, separate threshold profile |

Generation and dedup models are **independent** — e.g. DeepSeek for writing + E5 Large for duplicates.

Filtered duplicates are stored in `duplicate_records` and shown on the **Duplicates** page.

### Publishing
- **VK** — group posts with photo upload (community token + optional user token for photos)
- **Telegram** — channel posts via bot
- **Watermark** — optional logo overlay with presets, custom position/size, backdrop styles
- **Auto-publish** — scheduled publishing with jitter
- **History** — publish log per platform

### Automation
- Scheduled RSS fetch (interval or datetime + repeat)
- Auto AI processing after fetch
- Auto-approve (optional)
- Database backup scheduler

## Requirements

- **Windows 10/11** (WebView2 — bundled in the NSIS installer)
- **Node.js** 18+
- **Rust** 1.77+ ([rustup](https://www.rust-lang.org/tools/install))
- **Visual Studio 2022 Build Tools** with C++ workload (for release builds and the silent launcher)
- **GPU recommended** for local LLM / encoder models (CUDA via llama.cpp; CPU fallback supported)

### Install Rust (Windows)

```powershell
winget install Rustlang.Rustup
```

Restart the terminal, then verify:

```powershell
rustc --version
cargo --version
```

## Development

```powershell
cd D:\Social
npm install
npm run tauri dev
```

Frontend only:

```powershell
npm run dev
```

## Build

### NSIS installer

```powershell
npm run tauri build
```

Output: `src-tauri/target/release/bundle/nsis/Gaming News Publisher_0.1.0_x64-setup.exe`

### Portable package (recommended)

```powershell
npm run build:portable
```

Output folder: `GamingNewsPublisher/`

| File / folder | Purpose |
|---------------|---------|
| `Gaming News Publisher.exe` | Silent launcher (no console window) |
| `app/Gaming News Publisher.exe` | Main application |
| `app/data/` | SQLite DB, settings, images |
| `app/llm/` | `llama-server.exe` and downloaded GGUF models |
| `*_setup.exe` | Optional installer copy for other PCs |

Copy the entire `GamingNewsPublisher` folder anywhere and run `Gaming News Publisher.exe`.

If startup fails, check `launcher-error.log` in the portable root.

> Close the app before rebuilding the portable package — the EXE cannot be overwritten while running.

## First run

1. Open **Settings** and enter API keys (see below).
2. Under **Local models**, download an encoder for duplicate detection if using local dedup.
3. Open **Sources** — add RSS feeds or import presets.
4. On the **Dashboard**, click **Fetch news**.
5. Review posts under **Posts**, check **Duplicates** for filtered items.
6. Edit, approve, and publish.

## API keys

### DeepSeek (generation)

1. Register at [platform.deepseek.com](https://platform.deepseek.com)
2. Create an API key
3. Set in Settings → default model: `deepseek-chat`

### VKontakte

1. **Community token** — group → API → access key with `wall`, `photos`, `manage`
2. **User token** (recommended for photo upload) — admin OAuth with `wall`, `photos`, `groups`, `offline`
3. **Group ID** — numeric ID without minus, e.g. `188809704`

### Telegram

1. Create a bot via [@BotFather](https://t.me/BotFather)
2. Add the bot as a **channel administrator**
3. Channel ID: `@channelname` or `-1001234567890`

## Local models

Download from **Settings → Local models**:

| Role | Models |
|------|--------|
| Generation (LLM) | Vikhr-Nemo 12B Instruct (RU), Qwen2.5 14B Instruct |
| Dedup (encoder) | Multilingual E5 Base / Large, BGE-M3 |

Models are stored in `app/llm/models/` (portable) or next to the executable data directory.

**Suggested dedup settings for production:**

```json
{
  "ai_duplicate_check": true,
  "ai_duplicate_provider": "local",
  "local_dedup_model_id": "multilingual-e5-large",
  "ai_duplicate_window_days": 30,
  "ai_duplicate_check_limit": 200,
  "ai_duplicate_llm_top_k": 50
}
```

`bge-m3` is the most accurate but scores higher overall — use the built-in BGE threshold profile. `multilingual-e5-base` is the fastest option.

## Duplicate detection tips

- Duplicates are checked **at ingest time**; changing the dedup model does not re-process old queue items automatically.
- Run a new **fetch** after switching models or thresholds.
- The post-fetch sweep catches pairs ingested in parallel during the same batch (e.g. Mario $1bn, Nintendo Direct).
- Franchise-only overlap (e.g. two different Crazy Taxi articles sharing the game name) is intentionally **not** merged.
- For borderline cases, use the **Duplicates** page to review what was filtered.

## Data storage

Portable layout (next to the executable):

```
app/
  data/
    gaming_news.db      # posts, sources, categories, duplicate_records
    settings.json       # app settings
    images/             # cached post images
    watermark/          # watermark assets
  llm/
    bin/llama-server.exe
    models/*.gguf
```

Installed builds use the same layout relative to the application directory (`data/` beside the main EXE).

API keys and tokens are stored locally in `settings.json` on disk — keep backups private.

## Project structure

```
src/                    React UI (TypeScript, Tailwind, React Router)
  pages/                Dashboard, Posts, Sources, Settings, Duplicates, …
  components/           UI primitives, layout, post preview
src-tauri/              Rust backend (Tauri 2)
  src/commands/         IPC commands (settings, posts, RSS, AI, …)
  src/services/
    dedup_pipeline.rs   Duplicate check orchestration + post-fetch sweep
    embedding_dedup.rs  Encoder similarity + model-specific thresholds
    duplicate.rs        Lexical headline matching helpers
    deepseek.rs         Cloud AI + duplicate ranking
    rss_fetcher.rs      RSS/Atom parsing
    image_processor.rs  Image download, crop, template
    watermark.rs        Post image watermarking
    local_model_catalog.rs  Built-in GGUF catalog
  src/db/               SQLite schema and queries
launcher/               Silent Windows launcher (no console)
scripts/
  build-portable.ps1    Portable package build script
```

## Tech stack

- [Tauri 2](https://v2.tauri.app/) — desktop shell
- [React 19](https://react.dev/) + [Vite 7](https://vite.dev/)
- [Tailwind CSS 4](https://tailwindcss.com/)
- [SQLite](https://www.sqlite.org/) — local database
- [llama.cpp](https://github.com/ggerganov/llama.cpp) — local LLM and embedding server
- [DeepSeek API](https://platform.deepseek.com/) — cloud text generation

## License

This project is **free to use for any purpose** — personal, commercial, or otherwise. You may copy, modify, and distribute it without payment or permission.

**No attribution required.** You do not need to credit the author.

The software is provided as-is, without warranty. See [LICENSE](LICENSE) for the full legal text (Unlicense / public domain).
