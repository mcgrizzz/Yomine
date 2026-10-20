//! The shows and films sources are matched to, and how each source was matched.

use rusqlite::{
    params,
    Connection,
    OptionalExtension,
};

use crate::catalog::{
    signature::{
        Signature,
        SAME_EPISODE,
    },
    Kind,
    Match,
    Work,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchedBy {
    /// The file's name, looked up on TMDB.
    Name,
    /// Another file with the same episode's text.
    Content,
    User,
}

impl MatchedBy {
    pub fn as_str(self) -> &'static str {
        match self {
            MatchedBy::Name => "name",
            MatchedBy::Content => "content",
            MatchedBy::User => "user",
        }
    }

    fn from_key(key: &str) -> Option<Self> {
        match key {
            "name" => Some(MatchedBy::Name),
            "content" => Some(MatchedBy::Content),
            "user" => Some(MatchedBy::User),
            _ => None,
        }
    }
}

/// Links the source to the match's work and episode. A link the user made stays until the
/// user changes it.
pub fn link(
    conn: &Connection,
    fingerprint: &str,
    found: &Match,
    by: MatchedBy,
) -> rusqlite::Result<()> {
    if by != MatchedBy::User && user_decided(conn, fingerprint)? {
        return Ok(());
    }
    let source = super::sources::id(conn, fingerprint)?;
    let work = work_id(conn, &found.work)?;
    let episode = match found.episode {
        Some(number) => {
            Some(episode_id(conn, work, found.season, number, found.episode_title.as_deref())?)
        }
        None => None,
    };
    conn.execute(
        "UPDATE sources SET work_id = ?2, episode_id = ?3, matched_by = ?4 WHERE id = ?1",
        params![source, work, episode, by.as_str()],
    )
    .map(|_| ())
}

/// Clears the source's link. `by_user` records that the user said it isn't a show or film,
/// so automatic matching leaves it alone.
pub fn unlink(conn: &Connection, fingerprint: &str, by_user: bool) -> rusqlite::Result<()> {
    if !by_user && user_decided(conn, fingerprint)? {
        return Ok(());
    }
    let source = super::sources::id(conn, fingerprint)?;
    conn.execute(
        "UPDATE sources SET work_id = NULL, episode_id = NULL, matched_by = ?2 WHERE id = ?1",
        params![source, by_user.then_some(MatchedBy::User.as_str())],
    )
    .map(|_| ())
}

/// Whether the user linked the source, or said it isn't a show or film.
pub fn user_decided(conn: &Connection, fingerprint: &str) -> rusqlite::Result<bool> {
    conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM sources WHERE fingerprint = ?1 AND matched_by = ?2)",
        params![fingerprint, MatchedBy::User.as_str()],
        |r| r.get(0),
    )
}

/// The path the source was last opened from.
pub fn opened_path(conn: &Connection, fingerprint: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "SELECT o.path FROM opens o JOIN sources s ON s.id = o.source_id
         WHERE s.fingerprint = ?1 ORDER BY o.opened_at DESC LIMIT 1",
        params![fingerprint],
        |r| r.get(0),
    )
    .optional()
}

/// Subtitle sources matched this way, or unmatched for `None`, with the path each was last
/// opened from.
pub fn matched_by(
    conn: &Connection,
    by: Option<MatchedBy>,
) -> rusqlite::Result<Vec<(String, String)>> {
    conn.prepare(
        "SELECT s.fingerprint,
                (SELECT o.path FROM opens o WHERE o.source_id = s.id
                 ORDER BY o.opened_at DESC LIMIT 1) AS path
         FROM sources s
         WHERE s.matched_by IS ?1 AND s.kind = 'subtitles' AND path IS NOT NULL",
    )?
    .query_map(params![by.map(MatchedBy::as_str)], |r| Ok((r.get(0)?, r.get(1)?)))?
    .collect()
}

/// The TMDB IDs of the shows sources are matched to.
pub fn shows(conn: &Connection) -> rusqlite::Result<Vec<i64>> {
    conn.prepare("SELECT DISTINCT tmdb_id FROM works WHERE kind = 'tv' AND tmdb_id IS NOT NULL")?
        .query_map([], |r| r.get(0))?
        .collect()
}

/// The episodes, as `(show, season, episode)` in TMDB's numbering, that the collection has
/// notes from that Yomine hasn't seen deleted. Notes from before collections were recorded
/// count for every collection.
pub fn mined_episodes(
    conn: &Connection,
    collection: &str,
) -> rusqlite::Result<Vec<(i64, Option<u32>, u32)>> {
    conn.prepare(
        "SELECT DISTINCT w.tmdb_id, e.season, e.number FROM notes n
         JOIN sources s ON s.id = n.source_id
         JOIN works w ON w.id = s.work_id
         JOIN episodes e ON e.id = s.episode_id
         WHERE n.deleted_at IS NULL AND (n.collection = ?1 OR n.collection IS NULL)
           AND w.kind = 'tv' AND w.tmdb_id IS NOT NULL AND e.number IS NOT NULL",
    )?
    .query_map(params![collection], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
    .collect()
}

fn work_id(conn: &Connection, work: &Work) -> rusqlite::Result<i64> {
    let existing: Option<i64> = conn
        .query_row(
            "SELECT id FROM works WHERE kind = ?1 AND tmdb_id = ?2",
            params![work.kind.as_str(), work.tmdb_id],
            |r| r.get(0),
        )
        .optional()?;
    match existing {
        Some(id) => {
            conn.execute(
                "UPDATE works SET title = ?2, original_title = ?3 WHERE id = ?1",
                params![id, work.title, work.original_title],
            )?;
            Ok(id)
        }
        None => {
            conn.execute(
                "INSERT INTO works (kind, title, tmdb_id, original_title) VALUES (?1, ?2, ?3, ?4)",
                params![work.kind.as_str(), work.title, work.tmdb_id, work.original_title],
            )?;
            Ok(conn.last_insert_rowid())
        }
    }
}

fn episode_id(
    conn: &Connection,
    work: i64,
    season: Option<u32>,
    number: u32,
    title: Option<&str>,
) -> rusqlite::Result<i64> {
    let existing: Option<i64> = conn
        .query_row(
            "SELECT id FROM episodes WHERE work_id = ?1 AND season IS ?2 AND number = ?3",
            params![work, season, number],
            |r| r.get(0),
        )
        .optional()?;
    match existing {
        Some(id) => {
            conn.execute(
                "UPDATE episodes SET title = coalesce(?2, title) WHERE id = ?1",
                params![id, title],
            )?;
            Ok(id)
        }
        None => {
            conn.execute(
                "INSERT INTO episodes (work_id, season, number, title) VALUES (?1, ?2, ?3, ?4)",
                params![work, season, number, title],
            )?;
            Ok(conn.last_insert_rowid())
        }
    }
}

/// The source's work and episode, and how it was matched.
pub fn source_match(
    conn: &Connection,
    fingerprint: &str,
) -> rusqlite::Result<Option<(Match, MatchedBy)>> {
    conn.query_row(
        &format!(
            "SELECT {MATCH_COLUMNS}, s.matched_by
             FROM sources s {MATCH_JOINS}
             WHERE s.fingerprint = ?1 AND w.tmdb_id IS NOT NULL"
        ),
        params![fingerprint],
        |r| {
            let by = r.get::<_, Option<String>>(7)?.as_deref().and_then(MatchedBy::from_key);
            Ok(read_match(r, 0)?.zip(by))
        },
    )
    .optional()
    .map(Option::flatten)
}

/// The columns [`read_match`] reads, from a source `s` joined with [`MATCH_JOINS`].
pub(super) const MATCH_COLUMNS: &str =
    "w.kind, w.tmdb_id, w.title, w.original_title, e.season, e.number, e.title";
pub(super) const MATCH_JOINS: &str =
    "LEFT JOIN works w ON w.id = s.work_id AND w.tmdb_id IS NOT NULL
     LEFT JOIN episodes e ON e.id = s.episode_id";

/// The match in [`MATCH_COLUMNS`], starting at column `at`; `None` for an unmatched source.
pub(super) fn read_match(r: &rusqlite::Row, at: usize) -> rusqlite::Result<Option<Match>> {
    let Some(kind) = r.get::<_, Option<String>>(at)?.as_deref().and_then(Kind::from_key) else {
        return Ok(None);
    };
    Ok(Some(Match {
        work: Work {
            kind,
            tmdb_id: r.get(at + 1)?,
            title: r.get(at + 2)?,
            original_title: r.get::<_, Option<String>>(at + 3)?.unwrap_or_default(),
        },
        season: r.get(at + 4)?,
        episode: r.get(at + 5)?,
        episode_title: r.get(at + 6)?,
    }))
}

pub fn set_signature(
    conn: &Connection,
    fingerprint: &str,
    signature: &Signature,
) -> rusqlite::Result<()> {
    let source = super::sources::id(conn, fingerprint)?;
    conn.execute(
        "UPDATE sources SET signature = ?2 WHERE id = ?1",
        params![source, signature.to_bytes()],
    )
    .map(|_| ())
}

/// The match of another linked source holding the same episode's text, if there is one.
pub fn same_episode(
    conn: &Connection,
    fingerprint: &str,
    signature: &Signature,
) -> rusqlite::Result<Option<Match>> {
    let mut statement = conn.prepare(
        "SELECT fingerprint, signature FROM sources
         WHERE signature IS NOT NULL AND work_id IS NOT NULL AND fingerprint != ?1",
    )?;
    let best = statement
        .query_map(params![fingerprint], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Vec<u8>>(1)?)))?
        .filter_map(Result::ok)
        .filter_map(|(other, bytes)| {
            let shared = signature.containment(&Signature::from_bytes(&bytes)?);
            (shared >= SAME_EPISODE).then_some((shared, other))
        })
        .max_by(|a, b| a.0.total_cmp(&b.0));
    match best {
        Some((_, other)) => Ok(source_match(conn, &other)?.map(|(found, _)| found)),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::db::tests::scratch;

    fn frieren(episode: u32) -> Match {
        Match {
            work: Work {
                kind: Kind::Tv,
                tmdb_id: 209867,
                title: "Frieren: Beyond Journey's End".into(),
                original_title: "葬送のフリーレン".into(),
            },
            season: Some(1),
            episode: Some(episode),
            episode_title: None,
        }
    }

    #[test]
    fn a_users_link_outlasts_automatic_matches() {
        let (dir, conn) = scratch("works");
        link(&conn, "a", &frieren(3), MatchedBy::User).unwrap();
        link(&conn, "a", &frieren(4), MatchedBy::Name).unwrap();
        assert_eq!(source_match(&conn, "a").unwrap(), Some((frieren(3), MatchedBy::User)));

        unlink(&conn, "a", false).unwrap();
        assert_eq!(source_match(&conn, "a").unwrap(), Some((frieren(3), MatchedBy::User)));
        assert!(matched_by(&conn, Some(MatchedBy::Name)).unwrap().is_empty());

        link(&conn, "b", &frieren(3), MatchedBy::Name).unwrap();
        let episodes: i64 =
            conn.query_row("SELECT count(*) FROM episodes", [], |r| r.get(0)).unwrap();
        assert_eq!(episodes, 1);
        drop(conn);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
