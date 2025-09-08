use std::{collections::HashMap, sync::Arc};

use csrf::{AesGcmCsrfProtection, CsrfProtection};
use rand::RngCore;
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
    pub csrf_token: csrf::CsrfToken,
    pub link: LinkDAO,
    pub stats: StatsDAO,
}

impl LinkDAO {
    fn new(connection: Arc<Mutex<rusqlite::Connection>>) -> Self {
        Self { connection }
    }

    pub async fn insert(&self, link: &model::Link) -> Result<String, Box<DbError>> {
        let conn = self.connection.lock().await;

        conn.execute(
            r#"INSERT INTO link (ID, short, long, created, updated, owner) values (?1, ?2, ?3, ?4, ?5, ?6)"#,
            params![link.id, link.short, link.long, link.created, link.updated, link.owner],
        )
        .map_err(|e| DbError::from(e))?;

        conn.execute(
            r#"INSERT INTO stats (ID, created, clicks) values (?1, ?2, ?3)"#,
            params![link.id, chrono::Utc::now(), rusqlite::types::Null],
        )
        .map_err(|e| DbError::from(e))?;

        Ok(link.id.clone())
    }

    pub async fn update(&self, link: &model::Link) -> Result<(), Box<DbError>> {
        let conn = self.connection.lock().await;

        conn.execute(
            r#"UPDATE link 
    SET ID = ?1, 
        short = ?2, 
        long = ?3, 
        created = ?4, 
        updated = ?5, 
        owner = ?6 
    WHERE 
        id = ?7"#,
            params![
                link.id,
                link.short,
                link.long,
                link.created,
                link.updated,
                link.owner,
                link.id
            ],
        )
        .map_err(|e| DbError::from(e))?;

        Ok(())
    }

    pub async fn get(&self, id: &str) -> Result<model::Link, Box<DbError>> {
        let conn = self.connection.lock().await;

        let mut stmt = conn
            .prepare("SELECT * FROM link WHERE ID = ?1")
            .map_err(|e| DbError::from(e))?;
        stmt.query_one([id], |row| {
            Ok(model::Link {
                id: row.get(0)?,
                short: row.get(1)?,
                long: row.get(2)?,
                created: row.get(3)?,
                updated: row.get(4)?,
                owner: row.get(5)?,
            })
        })
        .map_err(|e| Box::new(DbError::from(e)))
    }

    pub async fn get_all(&self) -> Result<HashMap<String, model::Link>, Box<DbError>> {
        let conn = self.connection.lock().await;

        let mut stmt: rusqlite::Statement<'_> = conn.prepare(r#"SELECT * FROM link"#).map_err(|e| DbError::from(e))?;
        let rows = stmt.query([]).map_err(|e| DbError::from(e))?;
        let results: Vec<model::Link> = rows
            .map(|row| {
                Ok(model::Link {
                    id: row.get(0)?,
                    short: row.get(1)?,
                    long: row.get(2)?,
                    created: row.get(3)?,
                    updated: row.get(4)?,
                    owner: row.get(5)?,
                })
            })
            .collect()
            .map_err(|e| Box::new(DbError::from(e)))?;

        let mut mapped_results: HashMap<String, model::Link> = HashMap::new();
        results.iter().for_each(|link| {
            mapped_results.insert(link.id.clone(), link.clone());
        });

        Ok(mapped_results)
    }
}

impl StatsDAO {
    fn new(connection: Arc<Mutex<rusqlite::Connection>>) -> Self {
        Self { connection }
    }

    pub async fn incr(&self, id: &str) -> Result<(), Box<DbError>> {
        let conn = self.connection.lock().await;

        conn.execute(r#"UPDATE stats SET clicks = IFNULL(clicks, 0) + 1 WHERE ID = ?1"#, [id])
            .map_err(|e| DbError::from(e))?;

        Ok(())
    }

    pub async fn get(&self, id: &str) -> Result<Option<model::ClickStats>, Box<DbError>> {
        let conn = self.connection.lock().await;

        let mut stmt: rusqlite::Statement<'_> = conn
            .prepare(r#"SELECT * FROM stats WHERE ID = ?1"#)
            .map_err(|e| DbError::from(e))?;
        match stmt.query_one([id], |row| {
            Ok(model::ClickStats {
                id: row.get(0)?,
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

        let mut stmt: rusqlite::Statement<'_> = conn.prepare(r#"SELECT * FROM stats"#).map_err(|e| DbError::from(e))?;
        let rows = stmt.query([]).map_err(|e| DbError::from(e))?;
        let results: Result<Vec<model::ClickStats>, rusqlite::Error> = rows
            .map(|row| {
                Ok(model::ClickStats {
                    id: row.get(0)?,
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
    ID       TEXT    PRIMARY KEY,         -- normalized version of short (foobar)
	short    TEXT    NOT NULL DEFAULT "", -- user-provided short name (Foo-Bar)
	long     TEXT    NOT NULL DEFAULT "",
	created  INTEGER NOT NULL DEFAULT (strftime('%s', 'now')), -- unix seconds
	updated  INTEGER NOT NULL DEFAULT (strftime('%s', 'now')), -- unix seconds
	owner	 TEXT    NOT NULL DEFAULT ""
)"#,
        (),
    )?;

    Ok(())
}

fn create_stats_table(conn: &rusqlite::Connection) -> Result<(), rusqlite::Error> {
    conn.execute(
        r#"create table if not exists stats(
    ID       TEXT    NOT NULL DEFAULT "",
	created  INTEGER NOT NULL DEFAULT (strftime('%s', 'now')), -- unix seconds
	clicks   INTEGER
)"#,
        (),
    )?;

    Ok(())
}

impl Db {
    pub fn new(connection: rusqlite::Connection) -> Result<Self, rusqlite::Error> {
        let mut secret_key = [0u8; 32];
        rand::rng().fill_bytes(&mut secret_key);
        let protect = AesGcmCsrfProtection::from_key(secret_key);

        let mut nonce = [0u8; 64];
        rand::rng().fill_bytes(&mut nonce);
        let csrf_token: csrf::CsrfToken = protect.generate_token(&mut nonce).unwrap();

        create_link_table(&connection)?;
        create_stats_table(&connection)?;

        let boxed_connection = boxed(connection);
        Ok(Self {
            csrf_token,
            link: LinkDAO::new(Arc::clone(&boxed_connection)),
            stats: StatsDAO::new(Arc::clone(&boxed_connection)),
        })
    }
}
