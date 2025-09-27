use std::sync::Arc;

use rusqlite::fallible_iterator::FallibleIterator;
use rusqlite::params;
use tokio::sync::Mutex;
use uuid::Uuid;

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

    pub async fn insert(&self, link: &model::Link) -> Result<Uuid, Box<DbError>> {
        let conn = self.connection.lock().await;

        conn.execute(
            r#"INSERT INTO link (ID, short, long, created, updated) values (?1, ?2, ?3, ?4, ?5)"#,
            params![link.id.to_string(), link.short, link.long, link.created, link.updated],
        )
        .map_err(|e| DbError::from(e))?;

        conn.execute(
            r#"INSERT INTO stats (ID, created, clicks) values (?1, ?2, ?3)"#,
            params![link.id.to_string(), chrono::Utc::now(), rusqlite::types::Null],
        )
        .map_err(|e| DbError::from(e))?;

        Ok(link.id)
    }

    pub async fn update(&self, link: &model::Link) -> Result<(), Box<DbError>> {
        let conn = self.connection.lock().await;

        conn.execute(
            r#"UPDATE link SET short = ?1, long = ?2, created = ?3, updated = ?4 WHERE ID = ?5"#,
            params![
                link.short,
                link.long,
                link.created,
                link.updated,
                link.id.to_string()
            ],
        )
        .map_err(|e| DbError::from(e))?;

        Ok(())
    }

    pub async fn delete(&self, id: &Uuid) -> Result<(), Box<DbError>> {
        let conn = self.connection.lock().await;

        let mut stmt = conn
            .prepare("DELETE FROM link WHERE ID = ?1")
            .map_err(|e| DbError::from(e))?;
        let rows = stmt.execute([id.to_string()]).map_err(|e| DbError::from(e))?;
        tracing::debug!("deleted {} row(s)", rows);

        Ok(())
    }

    pub async fn get(&self, short: &str) -> Result<model::Link, Box<DbError>> {
        let conn = self.connection.lock().await;

        let mut stmt = conn
            .prepare("SELECT ID, short, long, created, updated FROM link WHERE short = ?1")
            .map_err(|e| DbError::from(e))?;
        stmt.query_one([short], |row| {
            let id_string: String = row.get(0)?;
            let id = Uuid::parse_str(&id_string).expect("Malformed UUIDv4");
            Ok(model::Link {
                id,
                short: row.get(1)?,
                long: row.get(2)?,
                created: row.get(3)?,
                updated: row.get(4)?,
            })
        })
        .map_err(|e| Box::new(DbError::from(e)))
    }

    pub async fn get_by_id(&self, id: &Uuid) -> Result<model::Link, Box<DbError>> {
        let conn = self.connection.lock().await;

        let mut stmt = conn
            .prepare("SELECT ID, short, long, created, updated FROM link WHERE ID = ?1")
            .map_err(|e| DbError::from(e))?;
        stmt.query_one([id.to_string()], |row| {
            let id_string: String = row.get(0)?;
            let id = Uuid::parse_str(&id_string).expect("Malformed UUIDv4");
            Ok(model::Link {
                id: id,
                short: row.get(1)?,
                long: row.get(2)?,
                created: row.get(3)?,
                updated: row.get(4)?,
            })
        })
        .map_err(|e| Box::new(DbError::from(e)))
    }

    pub async fn get_all(&self) -> Result<Vec<model::Link>, Box<DbError>> {
        let conn = self.connection.lock().await;

        let mut stmt: rusqlite::Statement<'_> = conn.prepare(r#"SELECT ID, short, long, created, updated FROM link"#).map_err(|e| DbError::from(e))?;
        let rows = stmt.query([]).map_err(|e| DbError::from(e))?;
        let results: Vec<model::Link> = rows
            .map(|row| {
                let id_string: String = row.get(0)?;
            let id = Uuid::parse_str(&id_string).expect("Malformed UUIDv4");
                Ok(model::Link {
                    id: id,
                    short: row.get(1)?,
                    long: row.get(2)?,
                    created: row.get(3)?,
                    updated: row.get(4)?,
                })
            })
            .collect()
            .map_err(|e| Box::new(DbError::from(e)))?;

        Ok(results)
    }

    pub async fn most_popular(&self) -> Result<Vec<(model::Link, model::ClickStats)>, Box<DbError>> {
        let conn = self.connection.lock().await;

        let mut stmt: rusqlite::Statement<'_> = conn.prepare(r#"SELECT l.ID, l.short, l.long, l.created, l.updated, s.ID, s.created, s.clicks
        FROM link l
        INNER JOIN stats s ON s.ID = l.ID
        ORDER BY s.clicks DESC
        LIMIT 10"#).map_err(|e| DbError::from(e))?;

        let rows = stmt.query([]).map_err(|e| DbError::from(e))?;
        let results: Vec<(model::Link, model::ClickStats)> = rows
            .map(|row| {
                let id_string: String = row.get(0)?;
                let id = Uuid::parse_str(&id_string).expect("Malformed UUIDv4");
                Ok(
                    (
                        model::Link {
                            id: id,
                            short: row.get(1)?,
                            long: row.get(2)?,
                            created: row.get(3)?,
                            updated: row.get(4)?,
                        },
                        model::ClickStats {
                            id: id,
                            created: row.get(6)?,
                            clicks: row.get(7)?
                        }
                    )
                )
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

    pub async fn incr(&self, id: &Uuid) -> Result<(), Box<DbError>> {
        let conn = self.connection.lock().await;

        conn.execute(r#"UPDATE stats SET clicks = IFNULL(clicks, 0) + 1 WHERE ID = ?1"#, [id.to_string()])
            .map_err(|e| DbError::from(e))?;

        Ok(())
    }

    pub async fn get(&self, id: &Uuid) -> Result<Option<model::ClickStats>, Box<DbError>> {
        let conn = self.connection.lock().await;

        let mut stmt: rusqlite::Statement<'_> = conn
            .prepare(r#"SELECT ID, created, clicks FROM stats WHERE ID = ?1"#)
            .map_err(|e| DbError::from(e))?;
        match stmt.query_one([id.to_string()], |row| {
            let id_string: String = row.get(0)?;
            let id = Uuid::parse_str(&id_string).expect("Malformed UUIDv4");
            Ok(model::ClickStats {
                id: id,
                created: row.get(1)?,
                clicks: row.get(2)?,
            })
        }) {
            Ok(stats) => Ok(Some(stats)),
            Err(e) => match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                _ => Err(Box::new(DbError::from(e))),
            },
        }
    }

    pub async fn get_all(&self) -> Result<Vec<model::ClickStats>, Box<DbError>> {
        let conn = self.connection.lock().await;

        let mut stmt: rusqlite::Statement<'_> = conn.prepare(r#"SELECT ID, created, clicks FROM stats"#).map_err(|e| DbError::from(e))?;
        let rows = stmt.query([]).map_err(|e| DbError::from(e))?;
        let results: Result<Vec<model::ClickStats>, rusqlite::Error> = rows
            .map(|row| {
                let id_string: String = row.get(0)?;
                let id = Uuid::parse_str(&id_string).expect("Malformed UUIDv4");
                Ok(model::ClickStats {
                    id: id,
                    created: row.get(1)?,
                    clicks: row.get(2)?,
                })
            })
            .collect();
        results.map_err(|e| Box::new(DbError::from(e)))
    }
}

fn boxed(conn: rusqlite::Connection) -> Arc<Mutex<rusqlite::Connection>> {
    Arc::new(Mutex::new(conn))
}

fn create_link_table(conn: &rusqlite::Connection) -> Result<(), rusqlite::Error> {
    conn.execute(
        r#"create table if not exists link(
    ID       TEXT    PRIMARY KEY,         -- UUIDv4
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
        r#"create table if not exists stats(
    ID       TEXT    PRIMARY KEY,                               -- UUIDv4
	created  INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),  -- unix seconds
	clicks   INTEGER
)"#,
        (),
    )?;

    Ok(())
}

impl Db {
    pub fn default() -> Result<Self, rusqlite::Error> {
        let connection = rusqlite::Connection::open_in_memory()?;
        Self::new(connection)
    }
    
    pub fn new(connection: rusqlite::Connection) -> Result<Self, rusqlite::Error> {
        create_link_table(&connection)?;
        create_stats_table(&connection)?;

        let boxed_connection = boxed(connection);
        Ok(Self {
            link: LinkDAO::new(Arc::clone(&boxed_connection)),
            stats: StatsDAO::new(Arc::clone(&boxed_connection)),
        })
    }
}
