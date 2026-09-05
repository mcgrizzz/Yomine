import gzip
import tempfile
import unittest
import xml.etree.ElementTree as ET
from pathlib import Path

from build import entry_record, legal_pairs, lexical_structure, normalize, parse_jmdict


class LexicalIndexTests(unittest.TestCase):
    def pairs(self, body):
        return list(legal_pairs(entry_record(ET.fromstring("<entry><ent_seq>1</ent_seq>" + body + "</entry>"))))

    def test_reading_restriction_does_not_cross_product_spellings(self):
        rows = self.pairs("<k_ele><keb>甲</keb></k_ele><k_ele><keb>乙</keb></k_ele>"
                          "<r_ele><reb>こう</reb><re_restr>甲</re_restr></r_ele><sense><pos>n</pos></sense>")
        self.assertEqual([(r[1], r[2]) for r in rows], [("こう", "甲")])

    def test_sense_restrictions_and_kana_preference_stay_local(self):
        rows = self.pairs("<k_ele><keb>甲</keb></k_ele><k_ele><keb>乙</keb></k_ele>"
                          "<r_ele><reb>こう</reb></r_ele><r_ele><reb>おつ</reb></r_ele>"
                          "<sense><stagk>甲</stagk><stagr>こう</stagr><pos>n</pos><misc>uk</misc></sense>"
                          "<sense><stagk>乙</stagk><pos>adv</pos></sense>")
        self.assertEqual([(r[1], r[2], r[6]) for r in rows], [("こう", "甲", 1), ("こう", "乙", 0), ("おつ", "乙", 0)])

    def test_no_kanji_is_not_an_alias_to_the_written_form(self):
        rows = self.pairs("<k_ele><keb>甲</keb></k_ele><r_ele><reb>こう</reb><re_nokanji/></r_ele><sense><pos>n</pos></sense>")
        self.assertEqual(rows[0][2], "こう")
        self.assertEqual(rows[0][7], 1)

    def test_pos_inheritance_preserved_without_spreading_misc(self):
        rows = self.pairs("<r_ele><reb>こう</reb></r_ele><sense><pos>v1</pos><misc>uk</misc></sense><sense/>")
        self.assertEqual(rows[0][4], rows[1][4])
        self.assertEqual([r[6] for r in rows], [1, 0])

    def test_search_only_spelling_is_retained_as_metadata(self):
        rows = self.pairs("<k_ele><keb>甲</keb><ke_inf>sK</ke_inf></k_ele><r_ele><reb>こう</reb></r_ele><sense/>")
        self.assertEqual(rows[0][8], '["sK"]')

    def test_entity_codes_with_hyphens_survive(self):
        xml = '<!DOCTYPE JMdict [<!ENTITY adj-na "adjectival noun"><!ENTITY uk "usually kana">]><JMdict><entry><ent_seq>1</ent_seq><r_ele><reb>あ</reb></r_ele><sense><pos>&adj-na;</pos><misc>&uk;</misc></sense></entry></JMdict>'
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "dictionary.gz"
            path.write_bytes(gzip.compress(xml.encode()))
            metadata, records = parse_jmdict(path)
            record = next(records)
        self.assertEqual(record["senses"][0]["pos"], ["adj-na"])
        self.assertEqual(record["senses"][0]["misc"], ["uk"])
        self.assertEqual(metadata["entities"]["adj-na"], "adjectival noun")

    def test_script_normalization_does_not_guess_long_vowels(self):
        self.assertEqual(normalize("ﾃﾞｷﾙ"), "できる")
        self.assertNotEqual(normalize("コー"), normalize("コウ"))

    def test_redirect_reading_is_preserved_and_not_inferred_from_spelling(self):
        tree = {"data":{"content":"redirect-glossary"}, "content": {
            "tag":"a", "href":"?query=%E6%82%AA%E7%9F%A5%E6%81%B5&primary_reading=%E3%82%8F%E3%82%8B%E3%81%A2%E3%81%88"}}
        self.assertEqual(lexical_structure(tree), [{"kind":"redirect", "target":"悪知恵", "reading":"わるぢえ"}])

    def test_jitendex_scope_and_codes_survive_without_glosses(self):
        tag = {"data": {"content": "misc-info", "code": "uk"}, "title": "usually kana"}
        tree = {"data": {"content": "sense-group"}, "content": [tag, {"data": {"content": "sense"}, "content": {"data": {"content": "glossary"}, "content": "not copied"}}]}
        result = lexical_structure(tree)
        self.assertEqual(result[0]["children"][0]["code"], "uk")
        self.assertEqual(result[0]["children"][1], {"kind": "sense", "children": []})
        self.assertNotIn("not copied", str(result))


if __name__ == "__main__":
    unittest.main()
