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


Yomine turns Japanese books, subtitles, and videos into ranked vocabulary lists and hides words you already know through Anki. Select words to mine, or let auto mode pick them for you.

<div align="center">

![Usage GIF](https://github.com/user-attachments/assets/6f8ae7df-dbfd-4c74-8572-05f9b452c22e)

</div>

<div align="center">

**[Download Yomine](https://github.com/mcgrizzz/Yomine/releases/latest)** · [Report a bug](https://github.com/mcgrizzz/Yomine/issues/new)

</div>

## Features

<table>
  <tr>
    <td width="50%"><strong>⛏️ Batch mining</strong><br>Select terms and mine them together using your Yomitan card formats.</td>
    <td width="50%"><strong>📖 Yomitan definitions</strong><br>Hold <strong>Shift</strong> over a term to view its definitions and choose which one to mine.</td>
  </tr>
  <tr>
    <td><strong>✦ Auto mode</strong><br>Picks the most useful cards from each new video, scored against what you already know, and mines them or lets you review them first.</td>
    <td><strong>🔊 Card media</strong><br>Add sentence audio and screenshots to your cards automatically, cut from local videos with ffmpeg or recorded through asbplayer.</td>
  </tr>
  <tr>
    <td><strong>🧠 Anki-aware results</strong><br>Hide known terms and view coverage, estimated knowledge, and sentence comprehension.</td>
    <td><strong>🔎 Ranking & filters</strong><br>Sort and filter by frequency, JLPT, part of speech, regex and more.</td>
  </tr>
  <tr>
    <td><strong>🎨 Customizable UI</strong><br>Choose from 14 themes, create your own, reorder columns, and scale the interface.</td>
    <td><strong>📚 Mining tools</strong><br>Manage ignore lists and frequency dictionaries or generate custom frequency dictionaries from your files.</td>
  </tr>
</table>

Supports `.srt`, `.ass`, `.ssa`, `.txt`, and `.epub` files, and videos (`.mkv`, `.mp4`, `.webm` and more) with Japanese subtitles beside them or embedded.

## Quick start

1. **[Download Yomine](https://github.com/mcgrizzz/Yomine/releases/latest)** for your platform.
2. Launch it. Language tools and the default frequency dictionary install automatically.
3. Follow **Settings → Setup Checklist** to finish setup.
4. Drop in a subtitle file or video, or load one from asbplayer.

<details>
<summary><strong>Platform installation notes</strong></summary>

- **Windows:** use the `-setup.exe` installer or `.msi`.
- **macOS:** use the universal `.dmg`. If macOS blocks the unnotarized app, run `xattr -cr /Applications/Yomine.app`.
- **Linux:** use the `.AppImage`, `.deb`, or `.rpm`.

</details>

## Setup

<details>
<summary><strong>Anki & Yomitan</strong></summary>

### 1. Connect Anki

Install [**Tsunagi**](https://ankiweb.net/shared/info/666370974) (recommended), then restart Anki.

**Code:** `666370974` · **Anki:** 26.08+  
*Live updates · Safe, undoable changes · Permission guidance*

| Your setup | What to do |
|---|---|
| Switching from AnkiConnect | Use Tsunagi's **Take over** option to preserve the port and key. |
| Anki older than 26.08 | Install [AnkiConnect](https://ankiweb.net/shared/info/2055492159) instead: `2055492159`. Restart Anki. |

### 2. Add your note types

Open **Settings → Anki** and add the note types you use.

- **Common note types:** fields are filled in automatically. Check and adjust if needed.
- **Local video mining:** also check the sentence audio and picture fields.

### 3. Connect Yomitan

1. Install [yomitan-api](https://github.com/yomidevs/yomitan-api).
2. Enable the API in Yomitan.
3. Configure at least one term card format.

**Default API address:** `http://127.0.0.1:19633`  
Change it under **Settings → Anki** if needed.

</details>

<details>
<summary><strong>Local videos, asbplayer & MPV</strong></summary>

Choose **Local** or **asbplayer** in the top bar for card audio and screenshots.

- **Local:** open videos or a folder. Locate or download ffmpeg under **Settings → Local Media**, where you can also adjust media settings.
- **asbplayer:** enable **MISC → WebSocket client** (default port `8766`), then load or follow videos from Yomine's asbplayer menu.
- **MPV:** choose a video from Yomine's MPV menu and locate the executable if prompted. MPV supports seeking only.

</details>

<details>
<summary><strong>Auto mode, dictionaries & ignore lists</strong></summary>

- **Mining → Auto Mode:** configure picks, then choose Review or Mine beside the file title.
- **Mining → Frequency Dictionaries:** manage dictionaries and weights. You can also import the [AniList Top 500 dictionary](https://github.com/user-attachments/files/31227863/Anilist.Top.500.zip).
- **Mining → Ignore List:** manage ignored terms.
- **Mining → Frequency Analyzer:** generate a dictionary from your files.

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

Built with [Vibrato](https://github.com/daac-tools/vibrato), [Tauri](https://tauri.app/), [Svelte](https://svelte.dev/), [jp-deinflector](https://github.com/btrkeks/jp-deinflector), [rbook](https://github.com/DevinSterling/rbook), and [rsubs-lib](https://github.com/adracea/rsubs-lib). JLPT data is derived from [coolmule0/JLPT-N5-N1-Japanese-Vocabulary-Anki](https://github.com/coolmule0/JLPT-N5-N1-Japanese-Vocabulary-Anki/).

**Author and maintainer:** [@mcgrizzz](https://github.com/mcgrizzz)

---

Happy mining! ⛏️ 頑張りましょう！
