# Yomine

Yomine is an Japanese vocabulary mining tool designed to help language learners extract and study words from subtitle files. Written in Rust, it integrates with asbplayer for timestamp navigation, ranks terms by frequency, and supports Anki integration to filter out known words.

## Status

🚧 **This project is under active development and not yet ready for public use.**  
There are currently no prebuilt binaries available. You'll need to build from source to try it out.

## Features

- Load subtitle files to mine Japanese vocabulary.
- Integrates with [asbplayer](https://github.com/killergerbah/asbplayer) to jump to term in streaming video.
- Ranks terms by frequency.
- Filter known terms based on anki decks.
- Displays terms, sentences, timestamps, frequency, and part of speech in a table.

### Screenshot

![Yomine UI](screenshot.png)

## Planned Features (in no particular order)

- **Anki Integration Customization**: Right now there is no way to change the decks that we read from.
- **Improved Segmentation**: There are still some issues with segmentation and part of speech tagging.
- **More File Types**: We want to add support for stuff like eBooks and web pages, etc.
- **Comprehensibility Estimate**: Using the anki integration, we should be able to estimate comprehensibility of a whole file and on the sentence level.
- **Custom Frequency Dictionaries**: Generate your own frequency lists from files you pick. For example you may want to generate a frequency dictionary for a certain show and then prioritize those terms in your mining.
- **Frequency Dictionary Weighting and Toggling**: Tweak how much each frequency list affects the rankings
- **Advanced Filtering Options**: Add better ways to filter terms, like by part of speech, min-max frequency, n+1 comprehensibility (or even n+i)
- **Prebuilt Binaries**

## Building from Source

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (with Cargo)
- [asbplayer](https://github.com/killergerbah/asbplayer) (optional, for timestamp navigation)
- Anki (optional, for known vocab filtering)

### Steps

1. Clone the repository:
   ```bash
   git clone https://github.com/yourusername/yomine.git
   cd yomine

2. Build and run:
    ```bash
    cargo build --release
    cargo run --release