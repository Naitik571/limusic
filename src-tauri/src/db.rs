//! Local SQLite state. context/11 §state. `rusqlite` (bundled) behind a Mutex — one file, low
//! write volume, no async pool needed (plan decision).

use std::sync::Mutex;

use rusqlite::Connection;

pub struct Db(Mutex<Connection>);

/// Log a failed write instead of dropping it on the floor. These used to be `let _ =`, which
/// turned a full disk or a locked file into silently lost data with no trace anywhere. Cache
/// tables degrade gracefully when this fires; settings/history losses are worth shouting about.
/// Returns 0 so it slots into `execute(...).unwrap_or_else(...)` (whose Ok type is row count).
fn warn_write(what: &'static str, table: &'static str) -> impl Fn(rusqlite::Error) -> usize {
    move |e| {
        tracing::warn!(error = %e, table, "sqlite write failed: {what}");
        0
    }
}

/// Unix seconds. Lives here because every wall-clock value in the app is a column in this file
/// (`expires_at`, `played_at`, `fetched_at`) or something stored alongside them.
pub fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// A cached stream URL with its expiry. Never a source of truth — purely a latency cache.
pub struct CachedStream {
    pub url: String,
    pub itag: i64,
    pub expires_at: i64,
    /// Raw `loudnessDb` (main-client metadata) so a cache-hit replay still normalizes loudness.
    pub loudness_db: Option<f64>,
}

impl Db {
    /// Expose inner mutex for modules that need raw queries (artist_packs, remote).
    pub fn conn_lock(&self) -> std::sync::MutexGuard<'_, rusqlite::Connection> {
        self.0.lock().unwrap()
    }

    pub fn open(path: &std::path::Path) -> rusqlite::Result<Self> {
        let conn = Connection::open(path)?;
        // WAL + NORMAL sync: a commit stops taking the file's full rollback journal plus an fsync.
        // A power cut can now lose the last transaction instead of corrupting the file either
        // way — for this data (a cache, play counts, settings) that trade is right, and it makes
        // every write in this file dramatically cheaper. `journal_mode` answers with a row, so
        // it goes through query_row; the pragma persists across restarts once set.
        let _ = conn.query_row("PRAGMA journal_mode = WAL", [], |r| r.get::<_, String>(0));
        let _ = conn.execute_batch("PRAGMA synchronous = NORMAL;");
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS settings (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS stream_url_cache (
                video_id    TEXT PRIMARY KEY,
                url         TEXT NOT NULL,
                itag        INTEGER NOT NULL,
                expires_at  INTEGER NOT NULL,
                loudness_db REAL
            );
            CREATE TABLE IF NOT EXISTS lyrics_cache (
                video_id   TEXT PRIMARY KEY,
                lyrics     TEXT,
                fetched_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS plays (
                id        INTEGER PRIMARY KEY,
                video_id  TEXT NOT NULL,
                played_at INTEGER NOT NULL,
                song_json TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS plays_played_at ON plays(played_at);
            CREATE TABLE IF NOT EXISTS local_tracks (
                path          TEXT PRIMARY KEY,
                title         TEXT NOT NULL,
                artist        TEXT NOT NULL,
                album         TEXT NOT NULL,
                album_key     TEXT NOT NULL,
                track_no      INTEGER NOT NULL,
                duration_secs INTEGER NOT NULL,
                cover         TEXT,
                mtime         INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS local_tracks_album ON local_tracks(album_key);
            CREATE TABLE IF NOT EXISTS downloads (
                video_id   TEXT PRIMARY KEY,
                file_path  TEXT NOT NULL,
                title      TEXT NOT NULL,
                artists    TEXT NOT NULL,
                album      TEXT,
                duration   INTEGER NOT NULL,
                thumb      TEXT,
                quality    TEXT NOT NULL,
                format     TEXT NOT NULL,
                size_bytes INTEGER NOT NULL,
                added_at   INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS waveforms (
                video_id    TEXT PRIMARY KEY,
                bars        BLOB NOT NULL,
                bar_count   INTEGER NOT NULL,
                computed_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS lyric_offsets (
                video_id  TEXT PRIMARY KEY,
                offset_ms INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS lyric_votes (
                video_id TEXT NOT NULL,
                source   TEXT NOT NULL,
                vote     INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                PRIMARY KEY (video_id, source)
            );
            CREATE TABLE IF NOT EXISTS artist_packs (
                id           TEXT PRIMARY KEY,
                name         TEXT NOT NULL,
                version      TEXT NOT NULL,
                description  TEXT,
                artist_ids   TEXT NOT NULL,
                aliases      TEXT NOT NULL,
                layout       TEXT,
                style_css    TEXT,
                installed_at INTEGER NOT NULL,
                thumbnail    TEXT
            );
            "#,
        )?;
        // Migrate pre-Phase-4 DBs that predate the loudness_db column. Errors ("duplicate column")
        // on fresh DBs are expected and ignored — the cache is disposable anyway.
        let _ = conn.execute(
            "ALTER TABLE stream_url_cache ADD COLUMN loudness_db REAL",
            [],
        );
        // Local files are no longer recorded as plays (see `AppState::on_position`), but 0.3.1
        // recorded them for a while, so clear out anything already sitting in On Repeat's table.
        let _ = conn.execute("DELETE FROM plays WHERE video_id LIKE 'LOCAL:%'", []);
        // Sweep dead stream URLs here as well as on write. `put_stream` only runs on a cache miss,
        // so a session spent replaying cached tracks never triggers one, and the backlog that
        // built up before anything pruned at all (1803 rows, 1772 of them expired, on a real
        // install) would sit there until it happened to.
        let _ = conn.execute(
            "DELETE FROM stream_url_cache WHERE expires_at <= ?1",
            [now_secs()],
        );
        Ok(Db(Mutex::new(conn)))
    }

    // --- settings ---------------------------------------------------------------------------

    pub fn get_setting(&self, key: &str) -> Option<String> {
        let conn = self.0.lock().unwrap();
        conn.query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| {
            r.get(0)
        })
        .ok()
    }

    pub fn set_setting(&self, key: &str, value: &str) {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT INTO settings(key, value) VALUES(?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [key, value],
        )
        .unwrap_or_else(warn_write("set_setting", "settings"));
    }

    pub fn delete_setting(&self, key: &str) {
        let conn = self.0.lock().unwrap();
        conn.execute("DELETE FROM settings WHERE key = ?1", [key])
            .unwrap_or_else(warn_write("delete_setting", "settings"));
    }

    /// Persist the canonical selected identity and its two legacy projections atomically. Older
    /// releases still read `data_sync_id` / `account_json`; keeping all three in one transaction
    /// prevents a restart from pairing one channel's request delegation with another's display.
    pub fn set_auth_identity(
        &self,
        session_cookie: &str,
        selected_json: &str,
        data_sync_id: Option<&str>,
        account_json: &str,
    ) -> rusqlite::Result<()> {
        let mut conn = self.0.lock().unwrap();
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO settings(key, value) VALUES('session_cookie', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [session_cookie],
        )?;
        tx.execute(
            "INSERT INTO settings(key, value) VALUES('selected_identity_json', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [selected_json],
        )?;
        if let Some(id) = data_sync_id {
            tx.execute(
                "INSERT INTO settings(key, value) VALUES('data_sync_id', ?1)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                [id],
            )?;
        } else {
            tx.execute("DELETE FROM settings WHERE key = 'data_sync_id'", [])?;
        }
        tx.execute(
            "INSERT INTO settings(key, value) VALUES('account_json', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [account_json],
        )?;
        tx.execute(
            "DELETE FROM settings WHERE key = 'account_selection_pending'",
            [],
        )?;
        tx.commit()
    }

    /// Persist an authenticated cookie while deliberately leaving the account unfinished. Keeping
    /// the marker and removal of stale identity projections in the same transaction means a crash
    /// during the required picker cannot restart into YouTube's default channel silently.
    pub fn set_pending_auth_selection(&self, session_cookie: &str) -> rusqlite::Result<()> {
        let mut conn = self.0.lock().unwrap();
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO settings(key, value) VALUES('session_cookie', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [session_cookie],
        )?;
        for key in ["selected_identity_json", "data_sync_id", "account_json"] {
            tx.execute("DELETE FROM settings WHERE key = ?1", [key])?;
        }
        tx.execute(
            "INSERT INTO settings(key, value) VALUES('account_selection_pending', 'true')
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [],
        )?;
        tx.commit()
    }

    pub fn clear_auth_identity(&self) -> rusqlite::Result<()> {
        let mut conn = self.0.lock().unwrap();
        let tx = conn.transaction()?;
        for key in [
            "selected_identity_json",
            "data_sync_id",
            "account_json",
            "account_selection_pending",
        ] {
            tx.execute("DELETE FROM settings WHERE key = ?1", [key])?;
        }
        tx.commit()
    }

    pub fn all_settings(&self) -> Vec<(String, String)> {
        let conn = self.0.lock().unwrap();
        let mut out = Vec::new();
        if let Ok(mut stmt) = conn.prepare("SELECT key, value FROM settings") {
            if let Ok(rows) = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?))) {
                out.extend(rows.flatten());
            }
        }
        out
    }

    // --- stream url cache -------------------------------------------------------------------

    /// Return the cached URL only if still valid (`expires_at` in the future). context/11.
    pub fn get_stream(&self, video_id: &str, now: i64) -> Option<CachedStream> {
        let conn = self.0.lock().unwrap();
        conn.query_row(
            "SELECT url, itag, expires_at, loudness_db FROM stream_url_cache WHERE video_id = ?1 AND expires_at > ?2",
            rusqlite::params![video_id, now],
            |r| {
                Ok(CachedStream {
                    url: r.get(0)?,
                    itag: r.get(1)?,
                    expires_at: r.get(2)?,
                    loudness_db: r.get(3)?,
                })
            },
        )
        .ok()
    }

    /// Drop a cached URL (e.g. it 403'd on the real GET). context/06 §2.
    pub fn evict_stream(&self, video_id: &str) {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "DELETE FROM stream_url_cache WHERE video_id = ?1",
            [video_id],
        )
        .unwrap_or_else(warn_write("evict_stream", "stream_url_cache"));
    }

    pub fn put_stream(
        &self,
        video_id: &str,
        url: &str,
        itag: i64,
        expires_at: i64,
        loudness_db: Option<f64>,
    ) {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT INTO stream_url_cache(video_id, url, itag, expires_at, loudness_db) VALUES(?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(video_id) DO UPDATE SET url = excluded.url, itag = excluded.itag, expires_at = excluded.expires_at, loudness_db = excluded.loudness_db",
            rusqlite::params![video_id, url, itag, expires_at, loudness_db],
        )
        .unwrap_or_else(warn_write("put_stream", "stream_url_cache"));
    }

    /// Wipe the whole URL cache (settings "Clear caches"). context/11.
    pub fn clear_stream_cache(&self) {
        let conn = self.0.lock().unwrap();
        conn.execute("DELETE FROM stream_url_cache", [])
            .unwrap_or_else(warn_write("clear_stream_cache", "stream_url_cache"));
        conn.execute("DELETE FROM lyrics_cache", [])
            .unwrap_or_else(warn_write("clear_stream_cache", "lyrics_cache"));
    }

    // --- lyrics cache -----------------------------------------------------------------------

    /// Cached lyrics JSON for a track. `Some(None)` = a cached "no lyrics" verdict (NULL row),
    /// still valid; misses expire after `miss_ttl` secs while hits live forever.
    pub fn get_lyrics(&self, video_id: &str, now: i64, miss_ttl: i64) -> Option<Option<String>> {
        let conn = self.0.lock().unwrap();
        let (lyrics, fetched_at): (Option<String>, i64) = conn
            .query_row(
                "SELECT lyrics, fetched_at FROM lyrics_cache WHERE video_id = ?1",
                [video_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .ok()?;
        if lyrics.is_none() && now - fetched_at > miss_ttl {
            return None; // stale negative result → refetch
        }
        Some(lyrics)
    }

    /// `lyrics = None` records a "no lyrics found" verdict.
    pub fn put_lyrics(&self, video_id: &str, lyrics: Option<&str>, now: i64) {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT INTO lyrics_cache(video_id, lyrics, fetched_at) VALUES(?1, ?2, ?3)
             ON CONFLICT(video_id) DO UPDATE SET lyrics = excluded.lyrics, fetched_at = excluded.fetched_at",
            rusqlite::params![video_id, lyrics, now],
        )
        .unwrap_or_else(warn_write("put_lyrics", "lyrics_cache"));
    }

    // --- lyric per-song offset + votes (Kodama parity) --------------------------------------

    /// Per-song sync offset in milliseconds. Positive = delay lyrics, negative = advance.
    pub fn get_lyric_offset(&self, video_id: &str) -> i64 {
        let conn = self.0.lock().unwrap();
        conn.query_row(
            "SELECT offset_ms FROM lyric_offsets WHERE video_id = ?1",
            [video_id],
            |r| r.get(0),
        )
        .unwrap_or(0)
    }

    pub fn set_lyric_offset(&self, video_id: &str, offset_ms: i64) {
        let conn = self.0.lock().unwrap();
        if offset_ms == 0 {
            conn.execute("DELETE FROM lyric_offsets WHERE video_id = ?1", [video_id])
                .unwrap_or_else(warn_write("delete_lyric_offset", "lyric_offsets"));
        } else {
            conn.execute(
                "INSERT INTO lyric_offsets(video_id, offset_ms) VALUES(?1, ?2)
                 ON CONFLICT(video_id) DO UPDATE SET offset_ms = excluded.offset_ms",
                rusqlite::params![video_id, offset_ms],
            )
            .unwrap_or_else(warn_write("set_lyric_offset", "lyric_offsets"));
        }
    }

    pub fn get_lyric_vote(&self, video_id: &str, source: &str) -> Option<i32> {
        let conn = self.0.lock().unwrap();
        conn.query_row(
            "SELECT vote FROM lyric_votes WHERE video_id = ?1 AND source = ?2",
            rusqlite::params![video_id, source],
            |r| r.get(0),
        )
        .ok()
    }

    pub fn set_lyric_vote(&self, video_id: &str, source: &str, vote: i32) {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT INTO lyric_votes(video_id, source, vote, updated_at) VALUES(?1, ?2, ?3, ?4)
             ON CONFLICT(video_id, source) DO UPDATE SET vote = excluded.vote, updated_at = excluded.updated_at",
            rusqlite::params![video_id, source, vote, now_secs()],
        )
        .unwrap_or_else(warn_write("set_lyric_vote", "lyric_votes"));
    }

    // --- play history (the On Repeat playlist) ------------------------------------------------

    /// Record one completed play and drop everything that has fallen out of the window, so the
    /// table stays bounded at roughly a month of listening whether or not anyone opens the
    /// playlist. `song_json` is the serialized `SongItem`, kept per row so the playlist can be
    /// rebuilt without asking YouTube for metadata it already gave us.
    pub fn record_play(&self, video_id: &str, song_json: &str, now: i64, window: i64) {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT INTO plays(video_id, played_at, song_json) VALUES(?1, ?2, ?3)",
            rusqlite::params![video_id, now, song_json],
        )
        .unwrap_or_else(warn_write("record_play", "plays"));
        conn.execute("DELETE FROM plays WHERE played_at < ?1", [now - window])
            .unwrap_or_else(warn_write("record_play/prune", "plays"));
    }

    /// The most-played songs since `since`, as `(song_json, play_count)` ranked by plays and then
    /// by recency. Each row's JSON comes from that song's latest play: SQLite resolves a bare
    /// column against the row matching the single `max()` in the query.
    pub fn top_plays(&self, since: i64, limit: usize) -> Vec<(String, i64)> {
        let conn = self.0.lock().unwrap();
        let mut out = Vec::new();
        if let Ok(mut stmt) = conn.prepare(
            "SELECT song_json, COUNT(*) AS plays, MAX(played_at) AS last FROM plays
             WHERE played_at >= ?1
             GROUP BY video_id
             ORDER BY plays DESC, last DESC
             LIMIT ?2",
        ) {
            if let Ok(rows) = stmt.query_map(rusqlite::params![since, limit as i64], |r| {
                Ok((r.get(0)?, r.get(1)?))
            }) {
                out.extend(rows.flatten());
            }
        }
        out
    }

    /// The raw play diary, newest first, as `(played_at, song_json)` — duplicates included. On
    /// Repeat answers "what do I play most"; this answers "what did I play, when", which is what
    /// the History page renders.
    pub fn recent_plays(&self, limit: i64) -> Vec<(i64, String)> {
        let conn = self.0.lock().unwrap();
        let mut out = Vec::new();
        if let Ok(mut stmt) = conn.prepare(
            "SELECT played_at, song_json FROM plays ORDER BY played_at DESC, id DESC LIMIT ?1",
        ) {
            if let Ok(rows) = stmt.query_map([limit], |r| Ok((r.get(0)?, r.get(1)?))) {
                out.extend(rows.flatten());
            }
        }
        out
    }

    /// Wipe the whole play diary (History page → Clear). On Repeat rebuilds from new plays.
    pub fn clear_plays(&self) {
        let conn = self.0.lock().unwrap();
        conn.execute("DELETE FROM plays", [])
            .unwrap_or_else(warn_write("clear_plays", "plays"));
    }

    /// Play count per videoId since `since`. [`Db::top_plays`] answers "what are my N most played
    /// songs"; this answers "how many times have I played each of these", which is what sorting an
    /// arbitrary playlist by plays needs. Same table, so the same trailing window applies.
    pub fn play_counts(&self, since: i64) -> Vec<(String, i64)> {
        let conn = self.0.lock().unwrap();
        let mut out = Vec::new();
        if let Ok(mut stmt) = conn
            .prepare("SELECT video_id, COUNT(*) FROM plays WHERE played_at >= ?1 GROUP BY video_id")
        {
            if let Ok(rows) = stmt.query_map([since], |r| Ok((r.get(0)?, r.get(1)?))) {
                out.extend(rows.flatten());
            }
        }
        out
    }

    // --- local music library (local.rs) -------------------------------------------------------

    /// Every known file with its recorded mtime — the scanner re-reads tags only where it differs.
    pub fn local_mtimes(&self) -> std::collections::HashMap<String, i64> {
        let conn = self.0.lock().unwrap();
        let mut out = std::collections::HashMap::new();
        if let Ok(mut stmt) = conn.prepare("SELECT path, mtime FROM local_tracks") {
            if let Ok(rows) = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?))) {
                out.extend(rows.flatten());
            }
        }
        out
    }

    /// Upsert a batch in one transaction. SQLite fsyncs per statement otherwise, which is the
    /// difference between a first scan taking a second and taking minutes.
    pub fn put_local_tracks(&self, tracks: &[LocalTrack]) {
        if tracks.is_empty() {
            return;
        }
        let mut conn = self.0.lock().unwrap();
        let Ok(tx) = conn.transaction() else { return };
        for t in tracks {
            tx.execute(
                LOCAL_TRACK_UPSERT,
                rusqlite::params![
                    t.path,
                    t.title,
                    t.artist,
                    t.album,
                    t.album_key,
                    t.track_no,
                    t.duration_secs,
                    t.cover,
                    t.mtime
                ],
            )
            .unwrap_or_else(warn_write("put_local_tracks", "local_tracks"));
        }
        if let Err(e) = tx.commit() {
            tracing::warn!(error = %e, "sqlite write failed: put_local_tracks commit");
        }
    }

    /// Forget files that are no longer on disk (the user deleted or moved them).
    pub fn delete_local_tracks(&self, paths: &[String]) {
        if paths.is_empty() {
            return;
        }
        let mut conn = self.0.lock().unwrap();
        let Ok(tx) = conn.transaction() else { return };
        for p in paths {
            tx.execute("DELETE FROM local_tracks WHERE path = ?1", [p])
                .unwrap_or_else(warn_write("delete_local_tracks", "local_tracks"));
        }
        if let Err(e) = tx.commit() {
            tracing::warn!(error = %e, "sqlite write failed: delete_local_tracks commit");
        }
    }

    /// All tracks, or one album's, in album order. ponytail: loads the whole table — a personal
    /// collection is thousands of rows, so paging it would buy nothing.
    pub fn local_tracks(&self, album_key: Option<&str>) -> Vec<LocalTrack> {
        let conn = self.0.lock().unwrap();
        let sql =
            "SELECT path, title, artist, album, album_key, track_no, duration_secs, cover, mtime
                   FROM local_tracks {WHERE} ORDER BY album, track_no, title";
        let sql = sql.replace(
            "{WHERE}",
            if album_key.is_some() {
                "WHERE album_key = ?1"
            } else {
                ""
            },
        );
        let mut out = Vec::new();
        let row = |r: &rusqlite::Row| {
            Ok(LocalTrack {
                path: r.get(0)?,
                title: r.get(1)?,
                artist: r.get(2)?,
                album: r.get(3)?,
                album_key: r.get(4)?,
                track_no: r.get(5)?,
                duration_secs: r.get(6)?,
                cover: r.get(7)?,
                mtime: r.get(8)?,
            })
        };
        if let Ok(mut stmt) = conn.prepare(&sql) {
            let rows = match album_key {
                Some(k) => stmt.query_map([k], row),
                None => stmt.query_map([], row),
            };
            if let Ok(rows) = rows {
                out.extend(rows.flatten());
            }
        }
        out
    }
}

/// A track the user has saved for offline playback. The audio bytes live at `file_path`; this row
/// is the catalogue the UI lists and the resolver consults before hitting the network.
#[derive(Debug, Clone)]
pub struct DownloadTrack {
    pub video_id: String,
    pub file_path: String,
    pub title: String,
    pub artists: String,
    pub album: Option<String>,
    pub duration: i64,
    pub thumb: Option<String>,
    pub quality: String,
    pub format: String,
    pub size_bytes: i64,
    pub added_at: i64,
}

impl Db {
    /// Path of the downloaded audio file for `video_id`, if one exists.
    pub fn download_path(&self, video_id: &str) -> Option<String> {
        let conn = self.0.lock().unwrap();
        conn.query_row(
            "SELECT file_path FROM downloads WHERE video_id = ?1",
            [video_id],
            |r| r.get(0),
        )
        .ok()
    }

    /// Which video owns `path`, if any. Two distinct tracks can share a `Title - Artist` name;
    /// the writer consults this before renaming so the second one disambiguates instead of
    /// silently overwriting the first one's audio.
    pub fn video_id_for_path(&self, path: &str) -> Option<String> {
        let conn = self.0.lock().unwrap();
        conn.query_row(
            "SELECT video_id FROM downloads WHERE file_path = ?1",
            [path],
            |r| r.get(0),
        )
        .ok()
    }

    /// Record a finished download (replaces any prior entry for the same video id).
    pub fn put_download(&self, d: &DownloadTrack) {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT INTO downloads(video_id, file_path, title, artists, album, duration, thumb, quality, format, size_bytes, added_at)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)
             ON CONFLICT(video_id) DO UPDATE SET file_path=excluded.file_path, title=excluded.title,
                artists=excluded.artists, album=excluded.album, duration=excluded.duration, thumb=excluded.thumb,
                quality=excluded.quality, format=excluded.format, size_bytes=excluded.size_bytes, added_at=excluded.added_at",
            rusqlite::params![
                d.video_id, d.file_path, d.title, d.artists, d.album, d.duration, d.thumb,
                d.quality, d.format, d.size_bytes, d.added_at
            ],
        )
        .unwrap_or_else(warn_write("put_download", "downloads"));
    }

    /// All downloaded tracks, newest first.
    pub fn list_downloads(&self) -> Vec<DownloadTrack> {
        let conn = self.0.lock().unwrap();
        let mut out = Vec::new();
        if let Ok(mut stmt) = conn.prepare(
            "SELECT video_id, file_path, title, artists, album, duration, thumb, quality, format, size_bytes, added_at
             FROM downloads ORDER BY added_at DESC",
        ) {
            if let Ok(rows) = stmt.query_map([], |r| {
                Ok(DownloadTrack {
                    video_id: r.get(0)?,
                    file_path: r.get(1)?,
                    title: r.get(2)?,
                    artists: r.get(3)?,
                    album: r.get(4)?,
                    duration: r.get(5)?,
                    thumb: r.get(6)?,
                    quality: r.get(7)?,
                    format: r.get(8)?,
                    size_bytes: r.get(9)?,
                    added_at: r.get(10)?,
                })
            }) {
                out.extend(rows.flatten());
            }
        }
        out
    }

    /// Remove one download's row (the file itself is deleted by the caller).
    pub fn delete_download(&self, video_id: &str) {
        let conn = self.0.lock().unwrap();
        conn.execute("DELETE FROM downloads WHERE video_id = ?1", [video_id])
            .unwrap_or_else(warn_write("delete_download", "downloads"));
    }

    /// How much disk the offline library currently occupies.
    pub fn downloads_total_bytes(&self) -> i64 {
        let conn = self.0.lock().unwrap();
        conn.query_row(
            "SELECT COALESCE(SUM(size_bytes),0) FROM downloads",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0)
    }

    /// Cached waveform peaks for `video_id`: `(bars 0–255, bar count)`, if computed before.
    pub fn get_waveform(&self, video_id: &str) -> Option<(Vec<u8>, i64)> {
        let conn = self.0.lock().unwrap();
        conn.query_row(
            "SELECT bars, bar_count FROM waveforms WHERE video_id = ?1",
            [video_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .ok()
    }

    /// Store computed waveform peaks (replaces any prior entry for the same video id).
    pub fn put_waveform(&self, video_id: &str, bars: &[u8], count: i64) {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT INTO waveforms(video_id, bars, bar_count, computed_at)
             VALUES(?1,?2,?3,?4)
             ON CONFLICT(video_id) DO UPDATE SET bars=excluded.bars, bar_count=excluded.bar_count,
                computed_at=excluded.computed_at",
            rusqlite::params![video_id, bars, count, now_secs()],
        )
        .unwrap_or_else(warn_write("put_waveform", "waveforms"));
    }
}

const LOCAL_TRACK_UPSERT: &str =
    "INSERT INTO local_tracks(path, title, artist, album, album_key, track_no, duration_secs, cover, mtime)
     VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
     ON CONFLICT(path) DO UPDATE SET title = excluded.title, artist = excluded.artist,
        album = excluded.album, album_key = excluded.album_key, track_no = excluded.track_no,
        duration_secs = excluded.duration_secs, cover = excluded.cover, mtime = excluded.mtime";

/// One file in the local library. Tag data as read at scan time; `mtime` is the change detector.
#[derive(Debug, Clone)]
pub struct LocalTrack {
    pub path: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    /// Stable, human-readable album id fragment (`artist--album`, sanitized). See `local.rs`.
    pub album_key: String,
    pub track_no: i64,
    pub duration_secs: i64,
    /// Absolute path to the cover image (extracted or found next to the files).
    pub cover: Option<String>,
    pub mtime: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Db {
        Db::open(std::path::Path::new(":memory:")).unwrap()
    }

    #[test]
    fn top_plays_ranks_by_count_then_recency_and_carries_the_latest_metadata() {
        let d = db();
        // "a" twice, "b" three times, "c" once but most recently, "old" outside the window.
        for (id, json, at) in [
            ("old", "{\"old\":1}", 100),
            ("a", "{\"a\":1}", 1_000),
            ("a", "{\"a\":2}", 1_100),
            ("b", "{\"b\":1}", 1_000),
            ("b", "{\"b\":2}", 1_050),
            ("b", "{\"b\":3}", 1_060),
            ("c", "{\"c\":1}", 2_000),
        ] {
            // A window wide enough that inserting doesn't prune what the next row needs; the
            // "old" row is excluded by `since` below instead.
            d.record_play(id, json, at, 10_000);
        }

        let top = d.top_plays(900, 20);
        assert_eq!(
            top,
            vec![
                ("{\"b\":3}".into(), 3), // most plays
                ("{\"a\":2}".into(), 2), // latest json wins for a song, not the first
                ("{\"c\":1}".into(), 1), // ties on count break toward the recent play
            ],
            "'old' is outside the window and must not appear"
        );
        assert_eq!(d.top_plays(900, 2).len(), 2, "limit applies");
    }

    #[test]
    fn opening_the_db_clears_local_files_out_of_on_repeat() {
        // 0.3.1 counted local plays before On Repeat excluded them; opening the db drops the rows.
        let path = std::env::temp_dir().join("limusic-plays-purge-test.sqlite");
        std::fs::remove_file(&path).ok();
        {
            let d = Db::open(&path).unwrap();
            d.record_play("LOCAL:/music/a.mp3", "{\"local\":1}", 1_000, 10_000);
            d.record_play("dQw4w9WgXcQ", "{\"yt\":1}", 1_000, 10_000);
            assert_eq!(d.top_plays(0, 20).len(), 2, "both were recorded");
        }
        let d = Db::open(&path).unwrap();
        assert_eq!(
            d.top_plays(0, 20),
            vec![("{\"yt\":1}".to_string(), 1)],
            "only the YouTube play survives"
        );
        drop(d);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn record_play_prunes_outside_the_window() {
        let d = db();
        d.record_play("stale", "{}", 1_000, 60);
        d.record_play("fresh", "{}", 5_000, 60); // prunes anything before 4_940
        assert_eq!(d.top_plays(0, 20), vec![("{}".to_string(), 1)]);
    }

    #[test]
    fn auth_identity_projections_are_updated_and_cleared_together() {
        let d = db();
        d.set_auth_identity(
            "SAPISID=cookie-a",
            r#"{"data_sync_id":"channel-a"}"#,
            Some("channel-a"),
            r#"{"name":"Channel A"}"#,
        )
        .unwrap();
        assert_eq!(d.get_setting("data_sync_id").as_deref(), Some("channel-a"));
        assert_eq!(
            d.get_setting("selected_identity_json").as_deref(),
            Some(r#"{"data_sync_id":"channel-a"}"#)
        );
        assert_eq!(
            d.get_setting("account_json").as_deref(),
            Some(r#"{"name":"Channel A"}"#)
        );
        assert_eq!(
            d.get_setting("session_cookie").as_deref(),
            Some("SAPISID=cookie-a")
        );

        d.set_pending_auth_selection("SAPISID=cookie-b").unwrap();
        assert_eq!(
            d.get_setting("session_cookie").as_deref(),
            Some("SAPISID=cookie-b")
        );
        assert_eq!(d.get_setting("selected_identity_json"), None);
        assert_eq!(d.get_setting("data_sync_id"), None);
        assert_eq!(d.get_setting("account_json"), None);
        assert_eq!(
            d.get_setting("account_selection_pending").as_deref(),
            Some("true")
        );

        d.set_auth_identity(
            "SAPISID=cookie-b",
            r#"{"data_sync_id":null}"#,
            None,
            r#"{"name":"Single channel"}"#,
        )
        .unwrap();
        assert_eq!(
            d.get_setting("data_sync_id"),
            None,
            "a stale delegated id must be deleted"
        );
        assert_eq!(d.get_setting("account_selection_pending"), None);

        d.clear_auth_identity().unwrap();
        assert_eq!(d.get_setting("selected_identity_json"), None);
        assert_eq!(d.get_setting("data_sync_id"), None);
        assert_eq!(d.get_setting("account_json"), None);
    }
}

// Queue persistence lives in the `settings` KV as a JSON blob (`queue_json`) + `queue_position`,
// so restore round-trips the full SongItem losslessly via serde (context/11 §state).
