import gzip
import json
import sqlite3
import zipfile
import tempfile
import unittest
import xml.etree.ElementTree as ET
from pathlib import Path

from build import build, entry_record, legal_pairs, normalize, parse_jmdict
from compile_runtime import compile_rows
from source_policy import source_manifest


class LexicalIndexTests(unittest.TestCase):
    def pairs(self, body):
        return list(legal_pairs(entry_record(ET.fromstring("<entry><ent_seq>1</ent_seq>" + body + "</entry>"))))

    def test_reading_restriction_does_not_cross_product_spellings(self):
        rows = self.pairs("<k_ele><keb>甲</keb></k_ele><k_ele><keb>乙</keb></k_ele>"
                          "<r_ele><reb>こう</reb><re_restr>甲</re_restr></r_ele><sense><pos>n</pos></sense>")
        self.assertEqual([(r[1], r[2]) for r in rows], [("こう", "甲")])

    def test_sense_restrictions_keep_their_pos(self):
        rows = self.pairs("<k_ele><keb>甲</keb></k_ele><k_ele><keb>乙</keb></k_ele>"
                          "<r_ele><reb>こう</reb></r_ele><r_ele><reb>おつ</reb></r_ele>"
                          "<sense><stagk>甲</stagk><stagr>こう</stagr><pos>n</pos><misc>uk</misc></sense>"
                          "<sense><stagk>乙</stagk><pos>adv</pos></sense>")
        self.assertEqual([(r[1], r[2], r[3]) for r in rows], [("こう", "甲", '["n"]'), ("こう", "乙", '["adv"]'), ("おつ", "乙", '["adv"]')])

    def test_no_kanji_is_not_an_alias_to_the_written_form(self):
        rows = self.pairs("<k_ele><keb>甲</keb></k_ele><r_ele><reb>こう</reb><re_nokanji/></r_ele><sense><pos>n</pos></sense>")
        self.assertEqual(rows[0][2], "こう")
        self.assertEqual(rows[0][6], 1)

    def test_pos_inheritance_is_preserved(self):
        rows = self.pairs("<r_ele><reb>こう</reb></r_ele><sense><pos>v1</pos><misc>uk</misc></sense><sense/>")
        self.assertEqual(rows[0][3], rows[1][3])

    def test_search_only_spelling_is_retained_as_metadata(self):
        rows = self.pairs("<k_ele><keb>甲</keb><ke_inf>sK</ke_inf></k_ele><r_ele><reb>こう</reb></r_ele><sense/>")
        self.assertEqual(rows[0][4], '["sK"]')

    def test_entity_codes_with_hyphens_survive(self):
        xml = '<!DOCTYPE JMdict [<!ENTITY adj-na "adjectival noun"><!ENTITY uk "usually kana">]><JMdict><entry><ent_seq>1</ent_seq><r_ele><reb>あ</reb></r_ele><sense><pos>&adj-na;</pos><misc>&uk;</misc></sense></entry></JMdict>'
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "dictionary.gz"
            path.write_bytes(gzip.compress(xml.encode()))
            metadata, records = parse_jmdict(path)
            record = next(records)
        self.assertEqual(record["senses"][0]["pos"], ["adj-na"])
        self.assertEqual(metadata["entities"]["adj-na"], "adjectival noun")

    def test_script_normalization_does_not_guess_long_vowels(self):
        self.assertEqual(normalize("ﾃﾞｷﾙ"), "できる")
        self.assertNotEqual(normalize("コー"), normalize("コウ"))

    def test_imported_reference_compiles_and_excludes_generated_sources(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            jmdict = root / "JMdict.gz"
            jmdict.write_bytes(gzip.compress(
                b'<JMdict><entry><ent_seq>1</ent_seq><k_ele><keb>word</keb></k_ele>'
                b'<r_ele><reb>reading</reb></r_ele><sense><pos>n</pos></sense></entry></JMdict>'
            ))
            jitendex = root / "jitendex.zip"
            with zipfile.ZipFile(jitendex, "w") as archive:
                archive.writestr("index.json", json.dumps({"revision": "test"}))
                archive.writestr("term_bank_1.json", json.dumps([["word", "reading", "", "", 0, [], 1, ""]]))
            frequency = root / "frequency"
            for name, data in [("Jiten", json.dumps([["word", "freq", {"reading": "reading", "frequency": {"value": 1, "displayValue": "1㋕"}}]])),
                               ("Anilist Top 500", "must never be parsed")]:
                bank = frequency / name / "term_meta_bank_1.json"
                bank.parent.mkdir(parents=True)
                bank.write_text(data)
            output = root / "reference.sqlite"
            build(jmdict, jitendex, frequency, output)
            with sqlite3.connect(output) as db:
                rows = dict(compile_rows(db))
                self.assertIn(b"word", rows[b"reading\t1"])
                self.assertEqual([s["dictionary"] for s in source_manifest(db)["frequency_sources"]], ["Jiten"])
                db.execute("UPDATE jitendex SET sequence=-1")
                self.assertEqual(compile_rows(db), [], "a redirect is not positive entry coverage")


if __name__ == "__main__":
    unittest.main()
