use std::sync::Arc;

use rusqlite::fallible_iterator::FallibleIterator;
use rusqlite::params;
use tokio::sync::Mutex;

use crate::model;

#[derive(Clone, Debug)]
pub struct DbError {
    pub message: String,
}

impl std::fmt::Display for DbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for DbError {}

impl DbError {
    pub fn new(message: String) -> Self {
        Self { message }
    }

    pub fn from(e: impl std::error::Error) -> Self {
        Self::new(format!("{}", e))
    }
}

#[derive(Clone, Debug)]
pub struct LinkDAO {
    connection: Arc<Mutex<rusqlite::Connection>>,
}

#[derive(Clone, Debug)]
pub struct StatsDAO {
    connection: Arc<Mutex<rusqlite::Connection>>,
}

#[derive(Clone, Debug)]
pub struct Db {
    pub link: LinkDAO,
    pub stats: StatsDAO,
}

impl LinkDAO {
    fn new(connection: Arc<Mutex<rusqlite::Connection>>) -> Self {
        Self { connection }
    }

    pub async fn save(&self, link: &model::Link) -> Result<(), Box<DbError>> {
        let conn = self.connection.lock().await;

        let rows_affected = conn
            .execute(
                r#"INSERT OR REPLACE INTO Links (ID, short, long, created, updated) values (?1, ?2, ?3, ?4, ?5)"#,
                params![
                    model::normalized_id(&link.short),
                    link.short,
                    link.long,
                    link.created,
                    link.updated
                ],
            )
            .map_err(DbError::from)?;
        if rows_affected != 1 {
            return Err(Box::new(DbError::new(format!(
                "expected to affect 1 row, affected {}",
                rows_affected
            ))));
        }

        Ok(())
    }

    pub async fn delete(&self, short: &str) -> Result<(), Box<DbError>> {
        let conn = self.connection.lock().await;

        let rows_affected = conn
            .execute("DELETE FROM Links WHERE ID = ?1", params![model::normalized_id(short)])
            .map_err(DbError::from)?;
        if rows_affected != 1 {
            return Err(Box::new(DbError::new(format!(
                "expected to affect 1 row, affected {}",
                rows_affected
            ))));
        }

        Ok(())
    }

    pub async fn load(&self, short: &str) -> Result<model::Link, Box<DbError>> {
        let conn = self.connection.lock().await;

        let mut stmt = conn
            .prepare("SELECT short, long, created, updated FROM Links WHERE ID = ?1")
            .map_err(DbError::from)?;
        stmt.query_one([model::normalized_id(short)], |row| {
            Ok(model::Link {
                short: row.get(0)?,
                long: row.get(1)?,
                created: row.get(2)?,
                updated: row.get(3)?,
            })
        })
        .map_err(|e| Box::new(DbError::from(e)))
    }

    pub async fn load_all(&self) -> Result<Vec<model::Link>, Box<DbError>> {
        let conn = self.connection.lock().await;

        let mut stmt: rusqlite::Statement<'_> = conn
            .prepare(r#"SELECT short, long, created, updated FROM Links"#)
            .map_err(DbError::from)?;
        let rows = stmt.query([]).map_err(DbError::from)?;
        let results: Vec<model::Link> = rows
            .map(|row| {
                Ok(model::Link {
                    short: row.get(0)?,
                    long: row.get(1)?,
                    created: row.get(2)?,
                    updated: row.get(3)?,
                })
            })
            .collect()
            .map_err(|e| Box::new(DbError::from(e)))?;

        Ok(results)
    }

    pub async fn most_popular(&self) -> Result<Vec<(model::Link, model::ClickStats)>, Box<DbError>> {
        let conn = self.connection.lock().await;

        let mut stmt: rusqlite::Statement<'_> = conn
            .prepare(
                r#"SELECT l.short, l.long, l.created, l.updated, s.created, s.clicks
        FROM Links l
        INNER JOIN Stats s ON s.ID = l.ID
        WHERE s.clicks NOT NULL
        ORDER BY s.clicks DESC
        LIMIT 10"#,
            )
            .map_err(DbError::from)?;

        let rows = stmt.query([]).map_err(DbError::from)?;
        let results: Vec<(model::Link, model::ClickStats)> = rows
            .map(|row| {
                Ok((
                    model::Link {
                        short: row.get(0)?,
                        long: row.get(1)?,
                        created: row.get(2)?,
                        updated: row.get(3)?,
                    },
                    model::ClickStats {
                        created: row.get(4)?,
                        clicks: row.get(5)?,
                    },
                ))
            })
            .collect()
            .map_err(|e| Box::new(DbError::from(e)))?;

        Ok(results)
    }
}

impl StatsDAO {
    fn new(connection: Arc<Mutex<rusqlite::Connection>>) -> Self {
        Self { connection }
    }

    pub async fn save(&self, short: &str) -> Result<(), Box<DbError>> {
        let conn = self.connection.lock().await;

        let rows_affected = conn
            .execute(
                r#"INSERT INTO Stats (ID, created, clicks) values (?1, ?2, ?3)"#,
                params![model::normalized_id(short), chrono::Utc::now(), rusqlite::types::Null],
            )
            .map_err(DbError::from)?;
        if rows_affected != 1 {
            return Err(Box::new(DbError::new(format!(
                "expected to affect 1 row, affected {}",
                rows_affected
            ))));
        }

        Ok(())
    }

    pub async fn incr(&self, short: &str) -> Result<(), Box<DbError>> {
        let conn = self.connection.lock().await;

        let rows_affected = conn
            .execute(
                r#"UPDATE Stats SET clicks = IFNULL(clicks, 0) + 1 WHERE stats.ID = ?1"#,
                [model::normalized_id(short)],
            )
            .map_err(DbError::from)?;
        if rows_affected != 1 {
            return Err(Box::new(DbError::new(format!(
                "expected to affect 1 row, affected {}",
                rows_affected
            ))));
        }
        Ok(())
    }

    pub async fn load(&self, short: &str) -> Result<Option<model::ClickStats>, Box<DbError>> {
        let conn = self.connection.lock().await;

        let mut stmt: rusqlite::Statement<'_> = conn
            .prepare(r#"SELECT created, clicks FROM Stats WHERE ID = ?1"#)
            .map_err(DbError::from)?;
        match stmt.query_one([model::normalized_id(short)], |row| {
            Ok(model::ClickStats {
                created: row.get(0)?,
                clicks: row.get(1)?,
            })
        }) {
            Ok(stats) => Ok(Some(stats)),
            Err(e) => match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                _ => Err(Box::new(DbError::from(e))),
            },
        }
    }

    pub async fn load_all(&self) -> Result<Vec<model::ClickStats>, Box<DbError>> {
        let conn = self.connection.lock().await;

        let mut stmt: rusqlite::Statement<'_> = conn
            .prepare(r#"SELECT created, clicks FROM Stats"#)
            .map_err(DbError::from)?;
        let rows = stmt.query([]).map_err(DbError::from)?;
        let results: Result<Vec<model::ClickStats>, rusqlite::Error> = rows
            .map(|row| {
                Ok(model::ClickStats {
                    created: row.get(0)?,
                    clicks: row.get(1)?,
                })
            })
            .collect();
        results.map_err(|e| Box::new(DbError::from(e)))
    }

    pub async fn delete(&self, short: &str) -> Result<(), Box<DbError>> {
        let conn = self.connection.lock().await;

        let rows_affected = conn
            .execute("DELETE FROM Stats WHERE ID = ?1", params![model::normalized_id(short)])
            .map_err(DbError::from)?;
        if rows_affected != 1 {
            return Err(Box::new(DbError::new(format!(
                "expected to affect 1 row, affected {}",
                rows_affected
            ))));
        }

        Ok(())
    }
}

fn create_link_table(conn: &rusqlite::Connection) -> Result<(), rusqlite::Error> {
    conn.execute(
        r#"create table if not exists Links(
    ID       TEXT    PRIMARY KEY,         -- normalized version of Short (foobar)
	short    TEXT    NOT NULL DEFAULT "", -- user-provided short name (Foo-Bar)
	long     TEXT    NOT NULL DEFAULT "",
	created  INTEGER NOT NULL DEFAULT (strftime('%s', 'now')), -- unix seconds
	updated  INTEGER NOT NULL DEFAULT (strftime('%s', 'now')), -- unix seconds
    UNIQUE(short)
)"#,
        (),
    )?;

    Ok(())
}

fn create_stats_table(conn: &rusqlite::Connection) -> Result<(), rusqlite::Error> {
    conn.execute(
        r#"create table if not exists Stats(
    ID       TEXT    NOT NULL DEFAULT "",                               
	created  INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),  -- unix seconds
	clicks   INTEGER
)"#,
        (),
    )?;

    Ok(())
}

impl Db {
    pub fn in_memory() -> Result<Self, rusqlite::Error> {
        let connection = rusqlite::Connection::open_in_memory()?;
        Self::new(connection)
    }

    pub fn new(connection: rusqlite::Connection) -> Result<Self, rusqlite::Error> {
        create_link_table(&connection)?;
        create_stats_table(&connection)?;

        let boxed_connection = Arc::new(Mutex::new(connection));
        Ok(Self {
            link: LinkDAO::new(Arc::clone(&boxed_connection)),
            stats: StatsDAO::new(Arc::clone(&boxed_connection)),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[tokio::test]
    async fn test_db() -> Result<(), Box<dyn std::error::Error + 'static>> {
        let connection = Connection::open_in_memory()?;
        let db = Db::new(connection)?;

        ////// Links
        let link_created = chrono::Utc::now();
        let test_link = model::Link {
            short: "nyt".to_string(),
            long: "https://www.nytimes.com".to_string(),
            created: link_created,
            updated: chrono::Utc::now(),
        };

        // Save
        db.link.save(&test_link).await?;
        db.stats.save(&test_link.short).await?;
        let clicks = db.stats.load(&test_link.short).await?;
        assert!(clicks.is_some());
        assert!(clicks.unwrap().clicks.is_none());

        // Load
        let from_db_link = db.link.load(&test_link.short).await?;
        assert_eq!(test_link, from_db_link);
        assert_eq!(from_db_link.short, test_link.short);

        // Load All
        let all_links = db.link.load_all().await?;
        assert_eq!(all_links.len(), 1);
        assert_eq!(*all_links.first().unwrap(), test_link);

        // Update
        let updated_link = model::Link {
            short: "nyt".to_string(), // cannot update the short -- MUST stay the same
            long: "https://nytimes.com".to_string(),
            created: link_created,
            updated: chrono::Utc::now(),
        };
        db.link.save(&updated_link).await?;
        let read_updated = db.link.load(&updated_link.short).await?;
        assert_eq!(read_updated.short, updated_link.short);

        ////// Stats INCR
        let mut stats = db.stats.load(&updated_link.short).await?;
        assert!(stats.is_some());
        assert!(stats.unwrap().clicks.is_none());

        db.stats.incr(&updated_link.short).await?;
        stats = db.stats.load(&updated_link.short).await?;
        assert!(stats.is_some());
        assert!(stats.unwrap().clicks.is_some_and(|clicks| clicks == 1));

        db.stats.incr(&updated_link.short).await?;
        stats = db.stats.load(&updated_link.short).await?;
        assert!(stats.is_some());
        assert!(stats.unwrap().clicks.is_some_and(|clicks| clicks == 2));

        db.stats.incr(&updated_link.short).await?;
        stats = db.stats.load(&updated_link.short).await?;
        assert!(stats.is_some());
        assert!(stats.unwrap().clicks.is_some_and(|clicks| clicks == 3));

        let res = db.link.most_popular().await?;
        assert!(res.len() == 1);
        let most_popular_links: Vec<model::PopularLink> = res
            .iter()
            .map(|(link, stats)| model::PopularLink {
                short: link.short.clone(),
                clicks: stats.clicks.or(Some(0)),
            })
            .collect();
        assert!(most_popular_links.len() == 1);

        // Delete
        db.link.delete(&test_link.short).await?;
        db.stats.delete(&test_link.short).await?;
        let clicks = db.stats.load(&test_link.short).await?;
        assert!(clicks.is_none());

        Ok(())
    }
}
