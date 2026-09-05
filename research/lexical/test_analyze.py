import unittest
import sqlite3
import tempfile
import gzip
import json
from pathlib import Path
from analyze import card_result, select_hint, export_reference


def entry(identity, spellings, kana=True, corroborated=True, reading="はし"):
    return {"entry_id":identity, "spellings":spellings, "reading":reading,
            "kana_senses":[1] if kana else [], "jitendex_kana":corroborated}


class HypothesisTests(unittest.TestCase):
    def test_unique_kana_preference_is_not_enough_for_contextual_hint(self):
        entries=[entry(1,["嘴"]),entry(2,["橋"],False),entry(3,["箸"],False)]
        self.assertEqual(select_hint(entries)["entry_id"],1)
        self.assertIsNone(select_hint(entries,"橋"))

    def test_multiple_preferred_identities_remain_unresolved(self):
        self.assertIsNone(select_hint([entry(1,["甲"]),entry(2,["乙"])],"甲"))

    def test_jitendex_disagreement_does_not_establish_a_hint(self):
        self.assertIsNone(select_hint([entry(1,["甲"],corroborated=False)],"甲"))

    def test_card_snapshots_do_not_change_selected_identity(self):
        entries=[entry(1,["出来る","出きる"],reading="できる"),entry(2,["出切る"],False,reading="できる")]
        selected=select_hint(entries,"出来る")
        self.assertEqual(card_result(entries,selected,("出来る","デキル")),"Known")
        self.assertEqual(card_result(entries,selected,("出きる","できる")),"Known")
        self.assertEqual(card_result(entries,selected,("出切る","できる")),"Unmatched")
        self.assertEqual(card_result(entries,selected,None),"Unmatched")
        self.assertEqual(card_result(entries,selected,("出来る","しゅったい")),"Unmatched")
        self.assertEqual(selected["entry_id"],1)

    def test_export_preserves_unlinked_rows_and_signed_redirects(self):
        db=sqlite3.connect(":memory:")
        db.row_factory=sqlite3.Row
        db.executescript("CREATE TABLE metadata(key,value); CREATE TABLE entries(id,data); CREATE TABLE jitendex(sequence,term,reading,definition_tags,word_classes,score,sense_metadata);")
        db.execute("INSERT INTO metadata VALUES ('manifest','{}')")
        db.execute("INSERT INTO entries VALUES (2,'{\"id\":2}')")
        for seq in [-1,2,-2,3]:
            db.execute("INSERT INTO jitendex VALUES (?,'甲','','','',0,'[]')",(seq,))
        with tempfile.TemporaryDirectory() as directory:
            output=Path(directory)/"reference.gz"
            export_reference(db,output)
            with gzip.open(output,"rt") as stream: records=[json.loads(line) for line in stream]
        self.assertEqual(len([r for r in records[1:] if r["jmdict"]]),1)
        self.assertEqual(sorted(j["source_sequence"] for r in records[1:] for j in r["jitendex"]),[-2,-1,2,3])

    def test_unresolved_matching_card_remains_possible(self):
        entries=[entry(1,["橋"],False),entry(2,["箸"],False)]
        self.assertEqual(card_result(entries,None,("橋","はし")),"Possible")
        self.assertEqual(card_result(entries,None,None),"Unmatched")
        self.assertEqual(card_result([],None,("橋","はし")),"Unmatched")


if __name__ == "__main__":
    unittest.main()
