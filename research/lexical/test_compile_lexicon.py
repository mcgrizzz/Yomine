import sqlite3
import struct
import unittest

from compile_lexicon import MAGIC, compile_rows
from compile_runtime import encode


class LexiconTests(unittest.TestCase):
    def setUp(self):
        self.db = sqlite3.connect(":memory:")
        self.addCleanup(self.db.close)
        self.db.execute("CREATE TABLE pairs(entry_id,reading,spelling,pos,misc)")

    def pair(self, entry, reading, spelling, pos='["n"]', misc="[]"):
        self.db.execute("INSERT INTO pairs VALUES(?,?,?,?,?)", (entry, reading, spelling, pos, misc))

    def rows(self):
        return dict(compile_rows(self.db))

    def test_phrase_forms_come_from_expression_and_adverb_entries(self):
        self.pair(1, "にとって", "に取って", '["exp"]')
        self.pair(2, "ついでに", "序でに", '["adv"]')
        self.pair(3, "あなた", "貴方", '["pn"]')
        rows = self.rows()
        for form in ["に取って", "にとって", "序でに", "ついでに"]:
            self.assertIn(("p\t" + form).encode(), rows)
        self.assertNotIn("p\tあなた".encode(), rows)

    def test_phrases_record_their_sense_count(self):
        for _ in range(3):
            self.pair(1, "ことになる", "事になる", '["exp","v5r"]')
        self.pair(2, "ではない", "では無い", '["exp"]')
        rows = self.rows()
        self.assertEqual(rows["p\tことになる".encode()][0], 3)
        self.assertEqual(rows["p\t事になる".encode()][0], 3)
        self.assertEqual(rows["p\tではない".encode()][0], 1)

    def test_forms_carry_the_type_of_their_first_sense(self):
        self.pair(1, "ほんとうに", "本当に", '["adv"]')
        self.pair(2, "それとも", "其れとも", '["conj"]')
        self.pair(3, "ことになる", "事になる", '["exp","v5r"]')
        self.pair(4, "おむかえ", "お迎え", '["n","vs","vt"]')
        self.pair(4, "おむかえ", "お迎え", '["n"]')
        self.pair(5, "だいじょうぶ", "大丈夫", '["adj-na"]')
        self.pair(5, "だいじょうぶ", "大丈夫", '["adv"]')
        self.pair(6, "だいじょうふ", "大丈夫", '["n"]')
        self.pair(7, "はいはい", "這い這い", '["n","vs","vi"]', '["uk"]')
        self.pair(8, "はいはい", "はいはい", '["int"]')
        self.pair(9, "どうだい", "同大", '["n"]')
        self.pair(10, "どうだい", "どうだい", '["exp"]')
        rows = self.rows()
        self.assertEqual(rows["p\t本当に".encode()][1:], b"d")
        self.assertEqual(rows["p\tそれとも".encode()][1:], b"c")
        self.assertEqual(rows["p\t事になる".encode()][1:], b"e")
        self.assertEqual(rows["w\tお迎え".encode()], b"n")
        self.assertEqual(rows["w\t大丈夫".encode()], b"a")
        self.assertEqual(rows["w\tはいはい".encode()], b"j")
        self.assertEqual(rows["p\tどうだい".encode()][1:], b"e")

    def test_readings_of_one_entry_share_it_and_separate_entries_do_not(self):
        for reading in ["あした", "あす"]:
            self.pair(1, reading, "明日")
        self.pair(2, "おもて", "表")
        self.pair(3, "ひょう", "表")
        rows = self.rows()
        self.assertEqual(rows["e\t明日\tあした".encode()], struct.pack("<I", 1))
        self.assertEqual(rows["e\t明日\tあす".encode()], struct.pack("<I", 1))
        self.assertFalse([k for k in rows if k.startswith("e\t表".encode())])

    def test_usually_kana_content_words_are_words(self):
        self.pair(1, "もったいない", "勿体無い", '["adj-i"]', '["uk"]')
        self.pair(2, "おすすめ", "お勧め", '["n","vs","adj-no"]', '["uk"]')
        self.pair(3, "こういう", "こう言う", '["adj-pn"]', '["uk"]')
        self.pair(4, "なんだって", "何だって", '["int"]', '["uk"]')
        self.pair(5, "しょくにん", "職人", '["n"]')
        rows = self.rows()
        for form in ["もったいない", "おすすめ"]:
            self.assertIn(("w\t" + form).encode(), rows)
        for form in ["こういう", "なんだって", "しょくにん"]:
            self.assertNotIn(("w\t" + form).encode(), rows)

    def test_output_is_sorted_and_deterministic(self):
        self.pair(1, "あした", "明日")
        self.pair(1, "あす", "明日")
        self.pair(2, "について", "について", '["exp"]')
        rows = compile_rows(self.db)
        self.assertEqual(rows, sorted(rows))
        self.assertEqual(encode(rows, MAGIC), encode(compile_rows(self.db), MAGIC))
        self.assertTrue(encode(rows, MAGIC).startswith(MAGIC))


if __name__ == "__main__":
    unittest.main()
