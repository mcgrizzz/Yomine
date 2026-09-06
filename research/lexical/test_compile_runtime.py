import json
import sqlite3
import unittest
from compile_runtime import compile_rows, encode
from source_policy import include_source, source_manifest, prepare_frequencies


class RuntimeTests(unittest.TestCase):
    def setUp(self):
        self.db = sqlite3.connect(":memory:")
        self.addCleanup(self.db.close)
        self.db.executescript('''
            CREATE TABLE pairs(entry_id,reading,spelling,sense,pos,spelling_info,reading_info,no_kanji,kana_preferred);
            CREATE TABLE jitendex(sequence,reading,term,kana_paths);
            CREATE TABLE frequencies(dictionary,term,reading,rank,kana_marker);
        ''')

    def pair(self, entry, spelling, pos='["n"]', supported=True, info="[]", no_kanji=0):
        self.db.execute("INSERT INTO pairs VALUES(?,?,?,?,?,?,?,?,?)",
                        (entry, "こと", spelling, 1, pos, info, "[]", no_kanji, 0))
        if supported:
            self.db.execute("INSERT INTO jitendex VALUES(?,?,?,?)", (entry, "こと", spelling, "[]"))

    def rank(self, spelling, rank, marker=False, source="A", reading="こと"):
        self.db.execute("INSERT INTO frequencies VALUES(?,?,?,?,?)", (source, spelling, reading, rank, marker))

    def preferred(self):
        self.pair(1, "事")
        self.pair(2, "琴")
        self.rank("事", 203)
        self.rank("事", 860, True)
        self.rank("琴", 20000)

    def test_kana_written_rank_spread_is_not_a_probability_or_a_gate(self):
        self.preferred()
        rows = dict(compile_rows(self.db))
        self.assertIn("事".encode(), rows["こと\t1".encode()])
        self.assertNotIn("琴".encode(), rows["こと\t1".encode()])

    def test_missing_competitor_rank_or_jitendex_does_not_create_uniqueness(self):
        self.preferred()
        self.pair(3, "古都", supported=False)
        self.assertEqual(compile_rows(self.db), [])

    def test_close_or_conflicting_linked_source_abstains(self):
        self.preferred()
        self.rank("琴", 300)
        self.assertEqual(compile_rows(self.db), [])
        self.db.execute("DELETE FROM frequencies WHERE rank=300")
        self.rank("事", 800, True, "B")
        self.rank("琴", 400, False, "B")
        self.assertEqual(compile_rows(self.db), [])

    def test_literal_kana_and_zero_ranks_cannot_establish_identity(self):
        self.pair(1, "事")
        for spelling, rank in [("こと", 65), ("事", 0)]:
            self.rank(spelling, rank, True)
        self.assertEqual(compile_rows(self.db), [])

    def test_shared_marker_does_not_merge_homophones(self):
        self.preferred()
        self.rank("琴", 860, True)
        rows = dict(compile_rows(self.db))
        self.assertIn("事".encode(), rows["こと\t1".encode()])
        self.assertNotIn("琴".encode(), rows["こと\t1".encode()])
        self.db.execute("DELETE FROM frequencies WHERE kana_marker=0")
        self.assertEqual(compile_rows(self.db), [])

    def test_transitive_shared_spellings_do_not_bridge_entries(self):
        self.preferred()
        self.pair(1, "共通")
        self.pair(2, "共通")
        self.rank("共通", 10, True)
        value = dict(compile_rows(self.db))["こと\t1".encode()]
        self.assertIn("事".encode(), value)
        self.assertNotIn("琴".encode(), value)
        self.assertNotIn("共通".encode(), value)

    def test_unknown_spelling_and_missing_reading_abstain(self):
        self.preferred()
        self.rank("未知", 100)
        self.assertEqual(compile_rows(self.db), [])
        self.db.execute("DELETE FROM frequencies WHERE term='未知'")
        self.db.execute("UPDATE frequencies SET reading=NULL WHERE kana_marker=1")
        self.assertEqual(compile_rows(self.db), [])

    def test_search_only_no_kanji_and_wrong_pos_do_not_become_targets(self):
        self.pair(1, "事", info='["sK"]')
        self.pair(1, "こと", no_kanji=1)
        self.rank("事", 15, True)
        self.assertEqual(compile_rows(self.db), [])
        self.db.execute("DELETE FROM pairs")
        self.pair(1, "事", pos='["prt"]')
        self.assertEqual(compile_rows(self.db), [])

    def test_jitendex_corroboration_is_required(self):
        self.preferred()
        self.db.execute("DELETE FROM jitendex")
        self.assertEqual(compile_rows(self.db), [])

    def test_generated_source_cannot_support_veto_or_supply_competitors(self):
        self.pair(1, "事")
        self.rank("事", 15, True, "Anilist Top 500")
        self.assertEqual(compile_rows(self.db), [])
        self.rank("事", 15, True)
        expected = compile_rows(self.db)
        self.assertTrue(expected)
        self.rank("未登録", 1, False, " Anilist Top 500 ")
        self.rank("事", 99999, True, "ANILIST TOP 500")
        self.assertEqual(compile_rows(self.db), expected)
        self.assertEqual(self.db.execute("SELECT count(*) FROM frequencies").fetchone()[0], 4)

    def test_import_policy_and_legacy_analysis_share_the_exclusion(self):
        self.assertFalse(include_source(" Anilist Top 500 "))
        self.assertTrue(include_source("Jiten"))
        self.db.execute("CREATE TABLE metadata(key,value)")
        self.db.execute("INSERT INTO metadata VALUES('manifest',?)", (json.dumps({
            "frequency_sources": [{"dictionary": "Anilist Top 500"}, {"dictionary": "Jiten"}]
        }),))
        self.rank("事", 1, source="Anilist Top 500")
        self.rank("事", 2, source="Jiten")
        prepare_frequencies(self.db)
        self.assertEqual(self.db.execute("SELECT dictionary FROM research_frequencies").fetchall(), [("Jiten",)])
        self.assertEqual(source_manifest(self.db)["frequency_sources"], [{"dictionary": "Jiten"}])

    def test_output_is_deterministic(self):
        self.preferred()
        self.assertEqual(encode(compile_rows(self.db)), encode(compile_rows(self.db)))
        self.assertTrue(encode(compile_rows(self.db)).startswith(b"KANAIDX2"))


if __name__ == "__main__":
    unittest.main()
