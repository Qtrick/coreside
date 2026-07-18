//! Persistent search session history (no credentials).

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::{Database, DbError, DbResult};

use super::models::{
    ImageSearchResponse, ImageSearchResult, VideoSearchResponse, VideoSearchResult,
    WebSearchResponse, WebSearchResult,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchSessionSummary {
    pub id: String,
    pub conversation_id: Option<String>,
    pub project_id: Option<String>,
    pub search_type: String,
    pub query: String,
    pub provider: String,
    pub created_at: String,
    pub result_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredSearchResult {
    pub id: String,
    pub session_id: String,
    pub result_type: String,
    pub title: Option<String>,
    pub url: Option<String>,
    pub display_domain: Option<String>,
    pub snippet: Option<String>,
    pub thumbnail_url: Option<String>,
    pub metadata_json: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchSessionDetail {
    pub session: SearchSessionSummary,
    pub results: Vec<StoredSearchResult>,
}

pub fn persist_web_session(
    db: &Database,
    conversation_id: Option<&str>,
    project_id: Option<&str>,
    query: &str,
    provider: &str,
    response: &WebSearchResponse,
) -> DbResult<String> {
    let session_id = insert_session(db, conversation_id, project_id, "web", query, provider)?;
    for result in &response.results {
        insert_web_result(db, &session_id, result)?;
    }
    Ok(session_id)
}

pub fn persist_image_session(
    db: &Database,
    conversation_id: Option<&str>,
    project_id: Option<&str>,
    query: &str,
    provider: &str,
    response: &ImageSearchResponse,
) -> DbResult<String> {
    let session_id = insert_session(db, conversation_id, project_id, "image", query, provider)?;
    for result in &response.results {
        insert_image_result(db, &session_id, result)?;
    }
    Ok(session_id)
}

pub fn persist_video_session(
    db: &Database,
    conversation_id: Option<&str>,
    project_id: Option<&str>,
    query: &str,
    provider: &str,
    response: &VideoSearchResponse,
) -> DbResult<String> {
    let session_id = insert_session(db, conversation_id, project_id, "video", query, provider)?;
    for result in &response.results {
        insert_video_result(db, &session_id, result)?;
    }
    Ok(session_id)
}

fn insert_session(
    db: &Database,
    conversation_id: Option<&str>,
    project_id: Option<&str>,
    search_type: &str,
    query: &str,
    provider: &str,
) -> DbResult<String> {
    let session_id = Uuid::new_v4().to_string();
    db.conn().execute(
        "INSERT INTO search_sessions (id, conversation_id, project_id, search_type, query, provider)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            session_id,
            conversation_id,
            project_id,
            search_type,
            query,
            provider
        ],
    )?;
    Ok(session_id)
}

fn insert_web_result(db: &Database, session_id: &str, result: &WebSearchResult) -> DbResult<()> {
    let result_id = Uuid::new_v4().to_string();
    let metadata = serde_json::json!({
        "rank": result.rank,
        "displayDomain": result.display_domain,
        "age": result.age,
    });
    db.conn().execute(
        "INSERT INTO search_results
         (id, session_id, result_type, title, url, display_domain, snippet, metadata_json)
         VALUES (?1, ?2, 'web', ?3, ?4, ?5, ?6, ?7)",
        params![
            result_id,
            session_id,
            result.title,
            result.url,
            result.display_domain,
            result.snippet,
            metadata.to_string()
        ],
    )?;
    Ok(())
}

fn insert_image_result(db: &Database, session_id: &str, result: &ImageSearchResult) -> DbResult<()> {
    let result_id = Uuid::new_v4().to_string();
    let metadata = serde_json::json!({
        "rank": result.rank,
        "imageUrl": result.image_url,
        "pageUrl": result.page_url,
        "width": result.width,
        "height": result.height,
        "source": result.source,
    });
    let domain = result
        .source
        .clone()
        .or_else(|| display_domain_from_url(&result.page_url));
    db.conn().execute(
        "INSERT INTO search_results
         (id, session_id, result_type, title, url, display_domain, snippet, thumbnail_url, metadata_json)
         VALUES (?1, ?2, 'image', ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            result_id,
            session_id,
            result.title,
            result.page_url,
            domain,
            result.image_url,
            result.thumbnail_url,
            metadata.to_string()
        ],
    )?;
    Ok(())
}

fn insert_video_result(db: &Database, session_id: &str, result: &VideoSearchResult) -> DbResult<()> {
    let result_id = Uuid::new_v4().to_string();
    let metadata = serde_json::json!({
        "rank": result.rank,
        "duration": result.duration,
        "creator": result.creator,
        "age": result.age,
    });
    let domain = display_domain_from_url(&result.url);
    db.conn().execute(
        "INSERT INTO search_results
         (id, session_id, result_type, title, url, display_domain, snippet, thumbnail_url, metadata_json)
         VALUES (?1, ?2, 'video', ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            result_id,
            session_id,
            result.title,
            result.url,
            domain,
            result.duration,
            result.thumbnail_url,
            metadata.to_string()
        ],
    )?;
    Ok(())
}

fn display_domain_from_url(url: &str) -> Option<String> {
    url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_string()))
}

pub fn list_search_sessions(
    db: &Database,
    conversation_id: Option<&str>,
    project_id: Option<&str>,
    search_type: Option<&str>,
    limit: usize,
) -> DbResult<Vec<SearchSessionSummary>> {
    let limit = limit.clamp(1, 200) as i64;
    let mut sql = String::from(
        "SELECT s.id, s.conversation_id, s.project_id, s.search_type, s.query, s.provider,
                s.created_at,
                (SELECT COUNT(*) FROM search_results r WHERE r.session_id = s.id) AS result_count
         FROM search_sessions s WHERE 1=1",
    );
    let mut binds: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(cid) = conversation_id.filter(|s| !s.is_empty()) {
        sql.push_str(" AND s.conversation_id = ?");
        binds.push(Box::new(cid.to_string()));
    }
    if let Some(pid) = project_id.filter(|s| !s.is_empty()) {
        sql.push_str(" AND s.project_id = ?");
        binds.push(Box::new(pid.to_string()));
    }
    if let Some(stype) = search_type.filter(|s| !s.is_empty()) {
        sql.push_str(" AND s.search_type = ?");
        binds.push(Box::new(stype.to_string()));
    }
    sql.push_str(" ORDER BY s.created_at DESC LIMIT ?");
    binds.push(Box::new(limit));

    let mut stmt = db.conn().prepare(&sql)?;
    let params_refs: Vec<&dyn rusqlite::types::ToSql> = binds.iter().map(|b| b.as_ref()).collect();
    let rows = stmt.query_map(params_refs.as_slice(), |row| {
        Ok(SearchSessionSummary {
            id: row.get(0)?,
            conversation_id: row.get(1)?,
            project_id: row.get(2)?,
            search_type: row.get(3)?,
            query: row.get(4)?,
            provider: row.get(5)?,
            created_at: row.get(6)?,
            result_count: row.get(7)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn get_search_session(db: &Database, session_id: &str) -> DbResult<SearchSessionDetail> {
    let session = db
        .conn()
        .query_row(
            "SELECT s.id, s.conversation_id, s.project_id, s.search_type, s.query, s.provider,
                    s.created_at,
                    (SELECT COUNT(*) FROM search_results r WHERE r.session_id = s.id) AS result_count
             FROM search_sessions s WHERE s.id = ?1",
            params![session_id],
            |row| {
                Ok(SearchSessionSummary {
                    id: row.get(0)?,
                    conversation_id: row.get(1)?,
                    project_id: row.get(2)?,
                    search_type: row.get(3)?,
                    query: row.get(4)?,
                    provider: row.get(5)?,
                    created_at: row.get(6)?,
                    result_count: row.get(7)?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| DbError::NotFound(format!("search session {session_id}")))?;

    let mut stmt = db.conn().prepare(
        "SELECT id, session_id, result_type, title, url, display_domain, snippet,
                thumbnail_url, metadata_json, created_at
         FROM search_results WHERE session_id = ?1
         ORDER BY rowid ASC",
    )?;
    let results = stmt
        .query_map(params![session_id], |row| {
            Ok(StoredSearchResult {
                id: row.get(0)?,
                session_id: row.get(1)?,
                result_type: row.get(2)?,
                title: row.get(3)?,
                url: row.get(4)?,
                display_domain: row.get(5)?,
                snippet: row.get(6)?,
                thumbnail_url: row.get(7)?,
                metadata_json: row.get(8)?,
                created_at: row.get(9)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(SearchSessionDetail { session, results })
}

/// Delete search history. When all filters are None, clears everything.
pub fn clear_search_history(
    db: &mut Database,
    conversation_id: Option<&str>,
    project_id: Option<&str>,
) -> DbResult<u64> {
    let deleted = match (
        conversation_id.filter(|s| !s.is_empty()),
        project_id.filter(|s| !s.is_empty()),
    ) {
        (Some(cid), Some(pid)) => db.conn().execute(
            "DELETE FROM search_sessions WHERE conversation_id = ?1 AND project_id = ?2",
            params![cid, pid],
        )?,
        (Some(cid), None) => db.conn().execute(
            "DELETE FROM search_sessions WHERE conversation_id = ?1",
            params![cid],
        )?,
        (None, Some(pid)) => db.conn().execute(
            "DELETE FROM search_sessions WHERE project_id = ?1",
            params![pid],
        )?,
        (None, None) => db.conn().execute("DELETE FROM search_sessions", [])?,
    };
    Ok(deleted as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use tempfile::tempdir;

    #[test]
    fn persists_lists_and_clears_sessions() {
        let dir = tempdir().unwrap();
        let mut db = Database::open_path(&dir.path().join("h.db")).unwrap();
        let web = WebSearchResponse {
            query: "photosynthesis".into(),
            provider: "mock".into(),
            results: vec![WebSearchResult {
                id: "1".into(),
                title: "Leaf".into(),
                url: "https://example.com/leaf".into(),
                display_domain: Some("example.com".into()),
                snippet: Some("green".into()),
                age: None,
                rank: 1,
            }],
            session_id: None,
            notice: None,
        };
        let sid = persist_web_session(&db, Some("c1"), Some("p1"), "photosynthesis", "mock", &web)
            .unwrap();
        let listed = list_search_sessions(&db, None, Some("p1"), None, 20).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, sid);
        assert_eq!(listed[0].result_count, 1);

        let detail = get_search_session(&db, &sid).unwrap();
        assert_eq!(detail.results.len(), 1);
        assert_eq!(detail.results[0].title.as_deref(), Some("Leaf"));

        let n = clear_search_history(&mut db, None, Some("p1")).unwrap();
        assert_eq!(n, 1);
        assert!(list_search_sessions(&db, None, None, None, 20)
            .unwrap()
            .is_empty());
    }
}
