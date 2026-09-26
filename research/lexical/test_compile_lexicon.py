import sqlite3
import struct
import unittest

from compile_lexicon import MAGIC, compile_rows
from compile_runtime import encode


class LexiconTests(unittest.TestCase):
    def setUp(self):
        self.db = sqlite3.connect(":memory:")
        self.addCleanup(self.db.close)
        self.db.execute("CREATE TABLE pairs(entry_id,reading,spelling,pos)")

    def pair(self, entry, reading, spelling, pos='["n"]'):
        self.db.execute("INSERT INTO pairs VALUES(?,?,?,?)", (entry, reading, spelling, pos))

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
        self.assertEqual(rows["p\tことになる".encode()], bytes([3]))
        self.assertEqual(rows["p\t事になる".encode()], bytes([3]))
        self.assertEqual(rows["p\tではない".encode()], bytes([1]))

    def test_readings_of_one_entry_share_it_and_separate_entries_do_not(self):
        for reading in ["あした", "あす"]:
            self.pair(1, reading, "明日")
        self.pair(2, "おもて", "表")
        self.pair(3, "ひょう", "表")
        rows = self.rows()
        self.assertEqual(rows["e\t明日\tあした".encode()], struct.pack("<I", 1))
        self.assertEqual(rows["e\t明日\tあす".encode()], struct.pack("<I", 1))
        self.assertFalse([k for k in rows if k.startswith("e\t表".encode())])

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
