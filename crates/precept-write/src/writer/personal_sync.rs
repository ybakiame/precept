use super::*;
use precept_core::operator::{
    Channel, Enforcement, Origin, PersonalSyncChange, PersonalSyncContent, RuleOptions, Scope,
    proposal_digest, tags_json, tombstone_digest,
};
use rusqlite::OptionalExtension;

struct Current {
    digest: String,
}

type CurrentRow = (
    String,
    String,
    i64,
    Vec<u8>,
    i64,
    String,
    String,
    i64,
    String,
);

pub(super) fn apply(
    db: &Connection,
    changes: &[PersonalSyncChange],
    now: &str,
    allocated: &mut i64,
    sequence: i64,
    next_review_id: &mut i64,
    channel: Channel,
) -> Result<BTreeMap<Key, (i64, bool)>> {
    let mut versions = BTreeMap::new();
    for change in changes {
        let current = current(db, &change.key)?;
        let local_digest = current
            .as_ref()
            .map(|current| current.digest.clone())
            .unwrap_or_else(|| tombstone_digest(&change.key));
        if local_digest != change.expected_digest {
            return Err(Error::Conflict);
        }
        deprecate_confirmed(
            db,
            &change.key,
            now,
            sequence,
            next_review_id,
            channel,
            &mut versions,
        )?;
        let Some(entry) = &change.entry else {
            if current.is_none() {
                return Err(Error::Conflict);
            }
            continue;
        };
        let id = *allocated;
        if id <= 0 {
            return Err(Error::Capacity);
        }
        let digest = proposal_digest(
            &Scope::Global,
            None,
            &entry.content.key,
            &entry.content.value,
            Origin::Operator,
            &entry.content.tags,
            &entry.content.rule,
        );
        db.execute(
            "INSERT INTO preferences(id,scope,project,pref_key,value,status,tags,revision,created_at,updated_at,origin,proposal_digest,applies_to,priority,enforcement,enabled) VALUES(?1,'global',NULL,?2,?3,'suggested',?4,1,?5,?5,'operator',?6,?7,?8,?9,1)",
            rusqlite::params![
                id,
                entry.content.key,
                entry.content.value,
                tags_json(&entry.content.tags),
                now,
                digest.to_vec(),
                entry.content.rule.applies_to.json(),
                entry.content.rule.priority,
                entry.content.rule.enforcement.as_str(),
            ],
        )
        .map_err(crate::sql_error)?;
        *allocated = next_revision(*allocated)?;
        db.execute(
            "UPDATE acp_profile SET next_preference_id=?1 WHERE singleton=1",
            [*allocated],
        )
        .map_err(crate::sql_error)?;
        db.execute(
            "UPDATE preferences SET status='confirmed',revision=2,updated_at=?1 WHERE id=?2",
            (now, id),
        )
        .map_err(crate::sql_error)?;
        decision(
            db,
            next_review_id,
            id,
            "confirm",
            1,
            2,
            &digest,
            sequence,
            channel,
            now,
            None,
        )?;
        versions.insert(pref_key(id), (2, false));
        if !entry.content.enabled {
            db.execute(
                "UPDATE preferences SET enabled=0,revision=3,updated_at=?1 WHERE id=?2",
                (now, id),
            )
            .map_err(crate::sql_error)?;
            decision(
                db,
                next_review_id,
                id,
                "disable",
                2,
                3,
                &digest,
                sequence,
                channel,
                now,
                None,
            )?;
            versions.insert(pref_key(id), (3, false));
        }
    }
    Ok(versions)
}

fn current(db: &Connection, key: &str) -> Result<Option<Current>> {
    let row: Option<CurrentRow> = db
        .query_row(
            "SELECT value,tags,enabled,proposal_digest,revision,origin,applies_to,priority,enforcement FROM preferences WHERE scope='global' AND status='confirmed' AND pref_key=?1 ORDER BY id DESC LIMIT 1",
            [key],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                ))
            },
        )
        .optional()
        .map_err(crate::sql_error)?;
    let Some((
        value,
        raw_tags,
        enabled,
        proposal,
        revision,
        origin,
        applies,
        priority,
        enforcement,
    )) = row
    else {
        return Ok(None);
    };
    let tags: Vec<String> = serde_json::from_str(&raw_tags).map_err(|_| Error::InvalidRecord)?;
    let rule = RuleOptions {
        applies_to: serde_json::from_str(&applies).map_err(|_| Error::InvalidRecord)?,
        priority,
        enforcement: Enforcement::parse(&enforcement).ok_or(Error::InvalidRecord)?,
    };
    let origin = Origin::parse(&origin).ok_or(Error::InvalidRecord)?;
    if revision <= 0 || !rule.valid() {
        return Err(Error::InvalidRecord);
    }
    let content = PersonalSyncContent {
        key: key.to_owned(),
        value,
        tags,
        rule,
        enabled: enabled != 0,
    };
    if !content.valid()
        || proposal
            != proposal_digest(
                &Scope::Global,
                None,
                key,
                &content.value,
                origin,
                &content.tags,
                &content.rule,
            )
    {
        return Err(Error::InvalidRecord);
    }
    Ok(Some(Current {
        digest: content.digest(),
    }))
}

#[allow(clippy::too_many_arguments)]
fn deprecate_confirmed(
    db: &Connection,
    key: &str,
    now: &str,
    sequence: i64,
    next_review_id: &mut i64,
    channel: Channel,
    versions: &mut BTreeMap<Key, (i64, bool)>,
) -> Result<()> {
    let peers: Vec<(i64, i64, Vec<u8>)> = db
        .prepare(
            "SELECT id,revision,proposal_digest FROM preferences WHERE scope='global' AND status='confirmed' AND pref_key=?1 ORDER BY id",
        )
        .map_err(crate::sql_error)?
        .query_map([key], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })
        .map_err(crate::sql_error)?
        .collect::<rusqlite::Result<_>>()
        .map_err(crate::sql_error)?;
    for (id, previous, digest) in peers {
        let revision = next_revision(previous)?;
        db.execute(
            "UPDATE preferences SET status='deprecated',revision=?1,updated_at=?2 WHERE id=?3",
            (revision, now, id),
        )
        .map_err(crate::sql_error)?;
        decision(
            db,
            next_review_id,
            id,
            "deprecate",
            previous,
            revision,
            &digest,
            sequence,
            channel,
            now,
            None,
        )?;
        versions.insert(pref_key(id), (revision, false));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn decision(
    db: &Connection,
    next_review_id: &mut i64,
    preference_id: i64,
    action: &str,
    from: i64,
    to: i64,
    digest: &[u8],
    sequence: i64,
    channel: Channel,
    now: &str,
    replaced_by: Option<i64>,
) -> Result<()> {
    let id = *next_review_id;
    *next_review_id = next_revision(id)?;
    db.execute(
        "INSERT INTO acp_review_decisions(id,scope,project,preference_id,action,from_revision,to_revision,proposal_digest,receipt_commit_sequence,channel,decided_at,note,replaced_by) VALUES(?1,'global',NULL,?2,?3,?4,?5,?6,?7,?8,?9,NULL,?10)",
        rusqlite::params![
            id,
            preference_id,
            action,
            from,
            to,
            digest.to_vec(),
            sequence,
            channel_name(channel),
            now,
            replaced_by,
        ],
    )
    .map_err(crate::sql_error)?;
    db.execute(
        "UPDATE acp_profile SET next_review_id=?1 WHERE singleton=1",
        [*next_review_id],
    )
    .map_err(crate::sql_error)?;
    Ok(())
}
