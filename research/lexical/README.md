# Kana preference data

The offline generator selects preferred spellings per reading and POS using JMdict,
Jitendex coverage and independent frequency sources. It requires an explicit kana
link, tenfold rank separation and competitor coverage; conflicting or missing
evidence stays unresolved. Jitendex derives from JMdict and is not independent evidence.

Never use Anilist Top 500 as an input or validation source: Yomine generated it.

The app searches [kana-preference.bin](../../assets/kana-preference.bin) directly.
The [manifest](../../assets/kana-preference.json) records source versions and hashes.
Retain the [data attribution](../../assets/kana-preference.LICENSE.md).

To regenerate, use Python 3.10+, local [JMdict](https://www.edrdg.org/pub/Nihongo/JMdict_e.gz)
and [Jitendex](https://jitendex.org/pages/downloads.html) downloads, installed frequency
dictionaries, and a new scratch database path. Run from the repository root:

```sh
python3 -m unittest discover -s research/lexical -p 'test_*.py'
python3 research/lexical/build.py --jmdict /path/to/JMdict_e.gz \
  --jitendex /path/to/jitendex-yomitan.zip --frequency-dir /path/to/dictionaries/frequency \
  --output /tmp/new-lexical-reference.sqlite
python3 research/lexical/compile_runtime.py --db /tmp/new-lexical-reference.sqlite \
  --output /tmp/new-kana-preference.bin
```

Review and test regenerated data before replacing the bundled binary and manifest.
A code-only generator refactor must reproduce the existing binary checksum.
