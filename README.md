<p align="center">
  <img src="https://github.com/user-attachments/assets/a12f45c1-d291-4541-981a-500f439e9682" width="95" height="95" style="border-radius: 16px" alt="yomine" />
</p>

<div align="center">

[![Build](https://img.shields.io/github/actions/workflow/status/mcgrizzz/Yomine/test.yml)](https://github.com/mcgrizzz/Yomine/actions/workflows/test.yml)
[![Downloads](https://img.shields.io/github/downloads/mcgrizzz/Yomine/total.svg)](https://github.com/mcgrizzz/Yomine/releases)
[![Release](https://img.shields.io/github/v/release/mcgrizzz/Yomine)](https://github.com/mcgrizzz/Yomine/releases/latest)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-yellow.svg)](#license)

</div>

# Yomine

### A Japanese vocabulary mining tool for extracting the most useful words from real content.


Yomine turns subtitle files into a ranked vocabulary list, hides words you already know with Anki, and creates cards using Yomitan and asbplayer.

<div align="center">

![Usage GIF](https://github.com/user-attachments/assets/6f8ae7df-dbfd-4c74-8572-05f9b452c22e)

</div>

<div align="center">

**[Download Yomine](https://github.com/mcgrizzz/Yomine/releases/latest)** · [Report a bug](https://github.com/mcgrizzz/Yomine/issues/new)

</div>

## Features

<table>
  <tr>
    <td width="50%"><strong>⛏️ One-click & batch mining</strong><br>Create individual cards or mine a queue using your Yomitan card formats.</td>
    <td width="50%"><strong>📖 Yomitan definitions</strong><br>Hold <strong>Shift</strong> over a term to view or mine an alternative definition.</td>
  </tr>
  <tr>
    <td><strong>🧠 Anki-aware results</strong><br>Hide known terms and view coverage, estimated knowledge, and sentence comprehension.</td>
    <td><strong>🔎 Ranking & filters</strong><br>Sort and filter by frequency, JLPT, part of speech, regex and more.</td>
  </tr>
  <tr>
    <td><strong>🎬 asbplayer & MPV</strong><br>Load subtitles, follow videos, seek timestamps, or launch MPV from Yomine.</td>
    <td><strong>🔊 Card media</strong><br>One click mining workflows support adding audio and screenshots via asbplayer automatically.</td>
  </tr>
  <tr>
    <td><strong>🎨 Customizable UI</strong><br>Choose from 14 themes, create your own, reorder columns, and scale the interface.</td>
    <td><strong>📚 Mining tools</strong><br>Manage ignore lists and frequency dictionaries or generate custom frequency dictionaries from your files.</td>
  </tr>
</table>

Supports `.srt`, `.ass`, `.ssa`, and `.txt` files. A premade frequency dictionary generated from the [AniList Top 500](https://github.com/user-attachments/files/23733337/Anilist.Top.500.zip) is also available.

## Quick start

1. **[Download the latest release](https://github.com/mcgrizzz/Yomine/releases/latest)** for your platform.
2. Launch Yomine. The tokenizer and default JPDB frequency dictionary install automatically.
3. Open **Settings → Setup Checklist** and connect [AnkiConnect](https://ankiweb.net/shared/info/2055492159).
4. Drop in a subtitle file or load one from asbplayer.

<details>
<summary><strong>Platform installation notes</strong></summary>

- **Windows:** use the `-setup.exe` installer or `.msi`.
- **macOS:** use the universal `.dmg`. If macOS blocks the unnotarized app, run `xattr -cr /Applications/Yomine.app`.
- **Linux:** use the `.AppImage`, `.deb`, or `.rpm`.

</details>

## Setup

<details>
<summary><strong>Anki & Yomitan</strong></summary>

1. Install [AnkiConnect](https://ankiweb.net/shared/info/2055492159) with add-on code `2055492159`, then restart Anki.
2. Open **Settings → Anki** in Yomine and map the term, reading, and optional sentence fields for each notetype.
3. For one-click mining, install [yomitan-api](https://github.com/yomidevs/yomitan-api), enable the API in Yomitan, and configure at least one term card format.
4. Yomine connects to the default yomitan-api server at `http://127.0.0.1:19633`. This can be changed in Settings -> Anki.

</details>

<details>
<summary><strong>asbplayer & MPV</strong></summary>

- **asbplayer:** enable **MISC → WebSocket client**. Yomine uses port `8766` by default. Its asbplayer menu can load subtitles or follow new videos and the active tab.
- **MPV:** choose a video from Yomine's MPV menu. If needed, locate the MPV executable when prompted.

asbplayer supports seeking, audio, and screenshots. MPV supports seeking only.

</details>

<details>
<summary><strong>Frequency dictionaries & ignore lists</strong></summary>

- Open **Mining → Frequency Dictionaries** to install, import, update, toggle, or weight Yomitan-compatible frequency dictionaries.
- Open **Mining → Ignore List** to remove, import, or export ignored terms.
- Use **Mining → Frequency Analyzer** to generate a frequency dictionary from your own files.

</details>

## Themes

Pick a bundled dark or light theme, or create your own.

<table>
  <tr>
    <th align="center">Dark themes</th>
    <th align="center">Light themes</th>
  </tr>
  <tr>
    <td><img alt="Yomine dark theme showcase" src="https://github.com/user-attachments/assets/5c1a2de3-4f0b-4218-bcaf-ffaeabbcb853" /></td>
    <td><img alt="Yomine light theme showcase" src="https://github.com/user-attachments/assets/c4b7ab4d-b7da-4e5b-9800-70d9e946d7dd" /></td>
  </tr>
</table>

## Building from source

<details>
<summary><strong>Build instructions</strong></summary>

Install [Rust](https://www.rust-lang.org/tools/install), [Node.js 22](https://nodejs.org/), [pnpm](https://pnpm.io/), the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for your platform, and Tauri CLI 2.

```bash
cargo install tauri-cli --version "^2.0.0" --locked
git clone https://github.com/mcgrizzz/Yomine.git
cd Yomine/src-tauri/ui
pnpm install
cd ../..
cargo tauri dev       # or: cargo tauri build
```

</details>

## License

Yomine is licensed under [MIT](LICENSE-MIT) OR [Apache-2.0](LICENSE-APACHE).

Built with [Vibrato](https://github.com/daac-tools/vibrato), [Tauri](https://tauri.app/), [Svelte](https://svelte.dev/), and [rsubs-lib](https://github.com/mcgrizzz/rsubs-lib). JLPT data is derived from [coolmule0/JLPT-N5-N1-Japanese-Vocabulary-Anki](https://github.com/coolmule0/JLPT-N5-N1-Japanese-Vocabulary-Anki/).

**Author and maintainer:** [@mcgrizzz](https://github.com/mcgrizzz)

---

Happy mining! ⛏️ 頑張りましょう！
