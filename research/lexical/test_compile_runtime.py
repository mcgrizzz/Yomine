import json
import sqlite3
import unittest
from compile_runtime import compile_rows, encode

class RuntimeTests(unittest.TestCase):
    def setUp(self):
        self.db = sqlite3.connect(":memory:")
        self.db.executescript("""CREATE TABLE pairs(entry_id,reading,spelling,sense,pos,spelling_info,reading_info,no_kanji,kana_preferred);
        CREATE TABLE jitendex(sequence,reading,term,kana_paths);""")

    def pair(self, entry, spelling, sense=1, preferred=1, supported=True, info="[]", no_kanji=0):
        self.db.execute("INSERT INTO pairs VALUES(?,?,?,?,?,?,?,?,?)", (entry,"こと",spelling,sense,'["n"]',info,"[]",no_kanji,preferred))
        if supported:
            self.db.execute("INSERT INTO jitendex VALUES(?,?,?,?)",(entry,"こと",spelling,'["uk"]'))

    def test_competing_preference_is_not_erased_by_missing_jitendex(self):
        self.pair(1,"事")
        self.pair(2,"異",supported=False)
        self.assertEqual(compile_rows(self.db), [])

    def test_sense_restrictions_do_not_create_transitive_aliases(self):
        self.pair(1,"甲",sense=1)
        self.pair(1,"乙",sense=1)
        self.pair(1,"乙",sense=2)
        self.pair(1,"丙",sense=2)
        rows=dict(compile_rows(self.db))
        self.assertIn("乙".encode(), rows["こと\t甲".encode()])
        self.assertNotIn("丙".encode(), rows["こと\t甲".encode()])

    def test_search_only_and_no_kanji_do_not_establish_written_identity(self):
        self.pair(1,"甲",info='["sK"]')
        self.pair(1,"こと",no_kanji=1)
        self.assertEqual(compile_rows(self.db), [])

    def test_missing_preference_or_jitendex_abstains(self):
        self.pair(1,"事",preferred=0)
        self.pair(2,"異",supported=False)
        self.assertEqual(compile_rows(self.db), [])

    def test_output_is_deterministic(self):
        self.pair(1,"事")
        self.assertEqual(encode(compile_rows(self.db)), encode(compile_rows(self.db)))
        self.assertTrue(encode(compile_rows(self.db)).startswith(b"KANAIDX1"))

if __name__ == "__main__":
    unittest.main()
