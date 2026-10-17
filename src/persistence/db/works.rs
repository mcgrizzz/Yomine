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
    let source = super::sources::id(conn, fingerprint)?;
    let current: Option<String> =
        conn.query_row("SELECT matched_by FROM sources WHERE id = ?1", [source], |r| r.get(0))?;
    if current.as_deref() == Some(MatchedBy::User.as_str()) && by != MatchedBy::User {
        return Ok(());
    }
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
        "SELECT w.kind, w.tmdb_id, w.title, w.original_title, e.season, e.number, e.title,
                s.matched_by
         FROM sources s
         JOIN works w ON w.id = s.work_id
         LEFT JOIN episodes e ON e.id = s.episode_id
         WHERE s.fingerprint = ?1 AND w.tmdb_id IS NOT NULL",
        params![fingerprint],
        |r| {
            let kind = Kind::from_key(&r.get::<_, String>(0)?);
            let by = r.get::<_, Option<String>>(7)?.as_deref().and_then(MatchedBy::from_key);
            let (Some(kind), Some(by)) = (kind, by) else { return Ok(None) };
            let found = Match {
                work: Work {
                    kind,
                    tmdb_id: r.get(1)?,
                    title: r.get(2)?,
                    original_title: r.get::<_, Option<String>>(3)?.unwrap_or_default(),
                },
                season: r.get(4)?,
                episode: r.get(5)?,
                episode_title: r.get(6)?,
            };
            Ok(Some((found, by)))
        },
    )
    .optional()
    .map(Option::flatten)
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

        link(&conn, "b", &frieren(3), MatchedBy::Name).unwrap();
        let episodes: i64 =
            conn.query_row("SELECT count(*) FROM episodes", [], |r| r.get(0)).unwrap();
        assert_eq!(episodes, 1);
        drop(conn);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
