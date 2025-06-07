

    
# Yomine
    Yomine’s name comes from a mix of 読み ("yomi" for reading) + "mine" (as in mining vocab)
Yomine is an Japanese vocabulary mining tool designed to help language learners extract and study words from subtitle files. Written in Rust, it integrates with asbplayer for timestamp navigation, ranks terms by frequency, and supports Anki integration to filter out known words.

https://github.com/user-attachments/assets/5a680ec7-bd2a-437b-849e-2387240de9a4

## Status

[![GitHub Workflow Status (with event)](https://img.shields.io/github/actions/workflow/status/mcgrizzz/Yomine/test.yml)](https://github.com/mcgrizzz/Yomine/actions/workflows/test.yml)
[![Github All Releases](https://img.shields.io/github/downloads/mcgrizzz/Yomine/total.svg)](https://github.com/mcgrizzz/Yomine/releases)
    
🚧 **This project is under active development and may be buggy.**  
The macos and linux binaries have not been extensively tested. 

## Features

- Load subtitle files to mine Japanese vocabulary.
- Integrates with [asbplayer](https://github.com/killergerbah/asbplayer) to jump to term in streaming video.
- Ranks terms by frequency.
- Filter known terms based on anki decks.
- Displays terms, sentences, timestamps, frequency, and part of speech in a table.

## Planned Features (in no particular order)

- [x] **Anki Integration Customization**: ~~Right now there is no way to change the decks that we read from.~~
- [ ] **Improved Segmentation**: There are still some issues with segmentation and part of speech tagging.
- [ ] **More File Types**: We want to add support for stuff like eBooks and web pages, etc.
- [ ] **Comprehensibility Estimate**: Using the anki integration, we should be able to estimate comprehensibility of a whole file and on the sentence level.
- [ ] **Custom Frequency Dictionaries**: Generate your own frequency lists from files you pick. For example you may want to generate a frequency dictionary for a certain show and then prioritize those terms in your mining.
- [ ] **Frequency Dictionary Weighting and Toggling**: Tweak how much each frequency list affects the rankings
- [ ] **Advanced Filtering Options**: Add better ways to filter terms, like by part of speech, min-max frequency, n+1 comprehensibility (or even n+i)
- [x] **Prebuilt Binaries**

## Frequency Dictionaries for Yomine

Yomine uses frequency dictionaries to show you more relevant words for your learning and to help with the segmentation process.

### How Yomine Loads Frequency Dictionaries

Yomine automatically scans for frequency dictionaries located in the `frequency_dict/` directory of the project. Each dictionary must be in the valid [Yomitan](https://github.com/yomidevs/yomitan) format.

When you start Yomine, it loads and processes all compatible dictionaries found in this directory.

### Downloading Frequency Dictionaries

 You can grab Yomitan-compatible frequency dictionaries and drop them into the `frequency_dict/` directory. Yomine will automatically unzip them and load them when you start the app. Here are some recommended ones to get you started:

- **[JPDB v2.2 Frequency Kana (Recommended)](https://github.com/Kuuuube/yomitan-dictionaries/?tab=readme-ov-file#jpdb-v22-frequency-kana-recommended)**: Great for Japanese media like anime and visual novels
- **[BCCWJ (Recommended)](https://github.com/Kuuuube/yomitan-dictionaries/?tab=readme-ov-file#bccwj-suw-luw-combined)**: Based on the Balanced Corpus of Contemporary Written Japanese
- **[CC100](https://drive.google.com/file/d/1_AYh1VtCq0cj1hXtFO15zRuPUUhUCSHD/view?usp=sharing)**: A broader list from Common Crawl data

You can also check out these Google Drive folders for more frequency dictionaries: [Marv's](https://drive.google.com/drive/folders/1xURpMJN7HTtSLuVs9ZtIbE7MDRCdoU29) and [Shoui's](https://drive.google.com/drive/folders/1g1drkFzokc8KNpsPHoRmDJ4OtMTWFuXi). If you have questions or need help with installing them, feel free to raise an issue on our GitHub repo

**Note**: Always download from trusted sources to avoid corrupted or malicious files. If you can't find a specific dictionary, consider generating your own using tools mentioned in Yomitan's documentation or community guides. You may want to ask around on the moeway discord as well.

## Setting Up Anki Integration

Yomine connects to Anki to filter out terms you already know.

### Prerequisites

**Install AnkiConnect**: Install the [AnkiConnect](https://ankiweb.net/shared/info/2055492159) add-on in Anki
   - In Anki, go to Tools → Add-ons → Get Add-ons
   - Enter the code: `2055492159`
   - Restart Anki

### Configuring Model Mappings

Yomine needs to know which fields in your Anki notes contain the Japanese terms and readings. Here's how to configure this:

1. **Open Settings**: In Yomine, go to Settings → Anki Settings
2. **Check Connection**: Wait for Yomine to connect to Anki
3. **Add Model Mappings**: 
   - Select your Anki note type from the dropdown
   - Yomine will try to guess these fields for you but you may need to manually select them.
   - Choose the **Term Field**: The field containing the Japanese word or phrase
   - Choose the **Reading Field**: The field containing the term reading
   - Click "Add" to save the mapping
   - Save your settings

![Anki Setup](docs/imgs/anki_setup.png)

## Building from Source

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (with Cargo)
- [asbplayer](https://github.com/killergerbah/asbplayer) (optional, for timestamp navigation)
- Anki (optional, for known vocab filtering)

### Steps

1. Clone the repository:
   ```bash
   git clone https://github.com/mcgrizzz/Yomine.git
   cd yomine

2. Build and run:
    ```bash
    cargo build --release
    cargo run --release

### Credits

Yomine author and maintainer: [@mcgrizzz](https://github.com/mcgrizzz)

Yomine is licensed under [MIT](LICENSE-MIT) OR [Apache-2.0](LICENSE-APACHE)



 * [Vibrato](https://github.com/daac-tools/vibrato) for text segmentation, [MIT](https://github.com/daac-tools/vibrato/blob/main/LICENSE-MIT) or [Apache-2.0](https://github.com/daac-tools/vibrato/blob/main/LICENSE-APACHE)
 * [egui](https://github.com/emilk/egui) for user interface, [MIT](https://github.com/emilk/egui/blob/main/LICENSE-MIT) or [Apache-2.0](https://github.com/emilk/egui/blob/main/LICENSE-APACHE)
  * [WanaKana Rust](https://github.com/PSeitz/wana_kana_rust) for japanese text utilities, [MIT](https://github.com/PSeitz/wana_kana_rust/blob/master/LICENSE)
 * [jp-definflector](https://github.com/btrkeks/jp-deinflector) for deinflection

 * `NotoSansJP-Bold.tff` `NotoSansJP-Regular.tff` `NotoSansJP-Thin.tff` - [SIL Open Font License](https://openfontlicense.org/open-font-license-official-text/)
