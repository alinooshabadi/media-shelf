//! Full candidate previews, selected-field merging, artwork cache and NFO export.
use crate::{Category, Source, Video, storage::database_path, tmdb::TmdbMatch};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{Duration, UNIX_EPOCH},
};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Person {
    pub name: String,
    pub role: String,
    #[serde(default)]
    pub tmdb_id: Option<u64>,
    #[serde(default)]
    pub profile_path: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Metadata {
    pub identity: TmdbMatch,
    pub year: Option<u16>,
    pub tagline: Option<String>,
    pub rating: Option<f64>,
    pub votes: Option<u64>,
    pub runtime: Option<u32>,
    pub certification: Option<String>,
    pub imdb_id: Option<String>,
    pub genres: Vec<String>,
    pub languages: Vec<String>,
    pub countries: Vec<String>,
    pub studios: Vec<String>,
    pub tags: Vec<String>,
    pub collection: Option<String>,
    pub trailer: Option<String>,
    pub cast: Vec<Person>,
    pub crew: Vec<Person>,
    pub poster: Option<String>,
    pub fanart: Option<String>,
    pub poster_file: Option<PathBuf>,
    pub fanart_file: Option<PathBuf>,
    #[serde(default)]
    pub nfo_written: bool,
    #[serde(default)]
    pub original_filename: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Field {
    Title,
    OriginalTitle,
    Tagline,
    Plot,
    Year,
    ReleaseDate,
    Rating,
    Runtime,
    Certification,
    Genres,
    Languages,
    Country,
    Studio,
    Tags,
    MovieSet,
    Trailer,
    Actors,
    Producers,
    Directors,
    Writers,
    Poster,
    Fanart,
}
impl Field {
    pub const ALL: &[Self] = &[
        Self::Title,
        Self::OriginalTitle,
        Self::Tagline,
        Self::Plot,
        Self::Year,
        Self::ReleaseDate,
        Self::Rating,
        Self::Runtime,
        Self::Certification,
        Self::Genres,
        Self::Languages,
        Self::Country,
        Self::Studio,
        Self::Tags,
        Self::MovieSet,
        Self::Trailer,
        Self::Actors,
        Self::Producers,
        Self::Directors,
        Self::Writers,
        Self::Poster,
        Self::Fanart,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Title => "Title",
            Self::OriginalTitle => "Original title",
            Self::Tagline => "Tagline",
            Self::Plot => "Plot",
            Self::Year => "Year",
            Self::ReleaseDate => "Release date",
            Self::Rating => "Rating",
            Self::Runtime => "Runtime",
            Self::Certification => "Certification",
            Self::Genres => "Genres",
            Self::Languages => "Language",
            Self::Country => "Country",
            Self::Studio => "Studio",
            Self::Tags => "Tags",
            Self::MovieSet => "Movie set",
            Self::Trailer => "Trailer",
            Self::Actors => "Actors",
            Self::Producers => "Producers",
            Self::Directors => "Directors",
            Self::Writers => "Writers",
            Self::Poster => "Poster",
            Self::Fanart => "Fanart",
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScrapeOptions {
    pub fields: Vec<Field>,
    pub only_missing: bool,
    pub write_nfo: bool,
    pub replace_nfo: bool,
}
impl Default for ScrapeOptions {
    fn default() -> Self {
        Self {
            fields: Field::ALL.to_vec(),
            only_missing: false,
            write_nfo: true,
            replace_nfo: false,
        }
    }
}

impl Metadata {
    pub fn empty(identity: TmdbMatch) -> Self {
        Self {
            identity,
            year: None,
            tagline: None,
            rating: None,
            votes: None,
            runtime: None,
            certification: None,
            imdb_id: None,
            genres: vec![],
            languages: vec![],
            countries: vec![],
            studios: vec![],
            tags: vec![],
            collection: None,
            trailer: None,
            cast: vec![],
            crew: vec![],
            poster: None,
            fanart: None,
            poster_file: None,
            fanart_file: None,
            nfo_written: false,
            original_filename: None,
        }
    }
}

pub fn merge(
    existing: Option<&Metadata>,
    baseline: TmdbMatch,
    new: &Metadata,
    options: &ScrapeOptions,
) -> Metadata {
    let mut out = existing
        .cloned()
        .unwrap_or_else(|| Metadata::empty(baseline));
    out.identity.id = new.identity.id;
    out.identity.category = new.identity.category;
    out.imdb_id = new.imdb_id.clone();
    macro_rules! set {
        ($field:ident,$target:expr,$source:expr,$empty:expr) => {
            if options.fields.contains(&Field::$field) && (!options.only_missing || $empty) {
                $target = $source;
            }
        };
    }
    set!(
        Title,
        out.identity.title,
        new.identity.title.clone(),
        out.identity.title.is_empty()
    );
    set!(
        OriginalTitle,
        out.identity.original_title,
        new.identity.original_title.clone(),
        out.identity.original_title.is_empty()
    );
    set!(
        Plot,
        out.identity.overview,
        new.identity.overview.clone(),
        out.identity.overview.is_empty()
    );
    set!(
        ReleaseDate,
        out.identity.release_date,
        new.identity.release_date.clone(),
        out.identity.release_date.is_none()
    );
    set!(Year, out.year, new.year, out.year.is_none());
    set!(
        Tagline,
        out.tagline,
        new.tagline.clone(),
        out.tagline.is_none()
    );
    if options.fields.contains(&Field::Rating) && (!options.only_missing || out.rating.is_none()) {
        out.rating = new.rating;
        out.votes = new.votes;
    }
    set!(Runtime, out.runtime, new.runtime, out.runtime.is_none());
    set!(
        Certification,
        out.certification,
        new.certification.clone(),
        out.certification.is_none()
    );
    set!(
        Genres,
        out.genres,
        new.genres.clone(),
        out.genres.is_empty()
    );
    set!(
        Languages,
        out.languages,
        new.languages.clone(),
        out.languages.is_empty()
    );
    set!(
        Country,
        out.countries,
        new.countries.clone(),
        out.countries.is_empty()
    );
    set!(
        Studio,
        out.studios,
        new.studios.clone(),
        out.studios.is_empty()
    );
    set!(Tags, out.tags, new.tags.clone(), out.tags.is_empty());
    set!(
        MovieSet,
        out.collection,
        new.collection.clone(),
        out.collection.is_none()
    );
    set!(
        Trailer,
        out.trailer,
        new.trailer.clone(),
        out.trailer.is_none()
    );
    set!(Actors, out.cast, new.cast.clone(), out.cast.is_empty());
    for (field, jobs) in [
        (Field::Directors, &["Director", "Creator"][..]),
        (Field::Writers, &["Writer", "Screenplay", "Story"][..]),
        (Field::Producers, &["Producer", "Executive Producer"][..]),
    ] {
        if options.fields.contains(&field)
            && (!options.only_missing || !out.crew.iter().any(|p| jobs.contains(&p.role.as_str())))
        {
            out.crew.retain(|p| !jobs.contains(&p.role.as_str()));
            out.crew.extend(
                new.crew
                    .iter()
                    .filter(|p| jobs.contains(&p.role.as_str()))
                    .cloned(),
            );
        }
    }
    if options.fields.contains(&Field::Poster) && (!options.only_missing || out.poster.is_none()) {
        out.poster = new.poster.clone();
        out.poster_file = None;
    }
    if options.fields.contains(&Field::Fanart) && (!options.only_missing || out.fanart.is_none()) {
        out.fanart = new.fanart.clone();
        out.fanart_file = None;
    }
    out
}

pub fn details(
    category: Category,
    id: i64,
    token: &str,
    language: &str,
) -> Result<Metadata, String> {
    details_at(
        "https://api.themoviedb.org/3",
        category,
        id,
        token,
        language,
    )
}
fn details_at(
    base: &str,
    category: Category,
    id: i64,
    token: &str,
    language: &str,
) -> Result<Metadata, String> {
    if id <= 0 {
        return Err("Invalid TMDB ID".into());
    }
    let endpoint = if category == Category::Movies {
        "movie"
    } else {
        "tv"
    };
    let append = if category == Category::Movies {
        "credits,keywords,videos,release_dates,external_ids"
    } else {
        "credits,keywords,videos,content_ratings,external_ids"
    };
    let data = request_json(
        &format!("{base}/{endpoint}/{id}"),
        token,
        &[("language", language), ("append_to_response", append)],
    )?;
    parse_details(&data, category, id, language)
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct EpisodeDetails {
    pub title: String,
    pub aired: Option<String>,
    pub rating: Option<f64>,
}

/// Episode details used by season browsing; callers retain local NFO values.
pub fn season_episodes(
    id: i64,
    season: u32,
    token: &str,
    language: &str,
) -> Result<std::collections::BTreeMap<u32, EpisodeDetails>, String> {
    season_episodes_at("https://api.themoviedb.org/3", id, season, token, language)
}

pub fn episode_titles(
    id: i64,
    season: u32,
    token: &str,
    language: &str,
) -> Result<std::collections::BTreeMap<u32, String>, String> {
    episode_titles_at("https://api.themoviedb.org/3", id, season, token, language)
}

fn episode_titles_at(
    base: &str,
    id: i64,
    season: u32,
    token: &str,
    language: &str,
) -> Result<std::collections::BTreeMap<u32, String>, String> {
    Ok(season_episodes_at(base, id, season, token, language)?
        .into_iter()
        .filter(|(_, e)| !e.title.trim().is_empty())
        .map(|(n, e)| (n, e.title))
        .collect())
}

fn season_episodes_at(
    base: &str,
    id: i64,
    season: u32,
    token: &str,
    language: &str,
) -> Result<std::collections::BTreeMap<u32, EpisodeDetails>, String> {
    if id <= 0 || season > 999 {
        return Err("Invalid show ID or season number".into());
    }
    #[derive(Deserialize)]
    struct Episode {
        episode_number: u32,
        season_number: Option<u32>,
        name: String,
        air_date: Option<String>,
        vote_average: Option<f64>,
    }
    #[derive(Deserialize)]
    struct Season {
        season_number: u32,
        episodes: Vec<Episode>,
    }
    let data: Season = serde_json::from_value(request_json(
        &format!("{base}/tv/{id}/season/{season}"),
        token,
        &[("language", language)],
    )?)
    .map_err(|_| "Invalid TMDB season response")?;
    if data.season_number != season {
        return Err("TMDB returned a different season".into());
    }
    let mut episodes = std::collections::BTreeMap::new();
    for e in data.episodes {
        if e.episode_number == 0 || e.season_number.is_some_and(|n| n != season) {
            return Err("Invalid TMDB episode numbering".into());
        }
        let info = EpisodeDetails {
            title: e.name,
            aired: e.air_date.filter(|d| valid_episode_date(d)),
            rating: e
                .vote_average
                .filter(|r| r.is_finite() && (0.0..=10.0).contains(r)),
        };
        if episodes.insert(e.episode_number, info).is_some() {
            return Err("TMDB returned duplicate episode numbers".into());
        }
    }
    Ok(episodes)
}

pub(crate) fn valid_episode_date(d: &str) -> bool {
    d.len() == 10
        && d.bytes().enumerate().all(|(i, b)| {
            if i == 4 || i == 7 {
                b == b'-'
            } else {
                b.is_ascii_digit()
            }
        })
        && ("01"..="12").contains(&&d[5..7])
        && ("01"..="31").contains(&&d[8..10])
}

/// Aired TMDB episodes for optional missing-file display. No library or media writes.
pub fn episode_catalog(
    id: i64,
    token: &str,
    language: &str,
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<std::collections::BTreeMap<u32, std::collections::BTreeMap<u32, String>>, String> {
    let today = utc_date(
        std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| "System clock is before 1970")?
            .as_secs(),
    );
    episode_catalog_at(
        "https://api.themoviedb.org/3",
        id,
        token,
        language,
        &today,
        cancel,
    )
}

pub fn episode_catalog_details(
    id: i64,
    token: &str,
    language: &str,
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<std::collections::BTreeMap<u32, std::collections::BTreeMap<u32, EpisodeDetails>>, String>
{
    let today = utc_date(
        std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| "System clock is before 1970")?
            .as_secs(),
    );
    episode_catalog_details_at(
        "https://api.themoviedb.org/3",
        id,
        token,
        language,
        &today,
        cancel,
    )
}

fn utc_date(seconds: u64) -> String {
    let z = (seconds / 86400) as i64 + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

fn episode_catalog_at(
    base: &str,
    id: i64,
    token: &str,
    language: &str,
    today: &str,
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<std::collections::BTreeMap<u32, std::collections::BTreeMap<u32, String>>, String> {
    Ok(
        episode_catalog_details_at(base, id, token, language, today, cancel)?
            .into_iter()
            .map(|(s, episodes)| (s, episodes.into_iter().map(|(n, e)| (n, e.title)).collect()))
            .collect(),
    )
}

fn episode_catalog_details_at(
    base: &str,
    id: i64,
    token: &str,
    language: &str,
    today: &str,
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<std::collections::BTreeMap<u32, std::collections::BTreeMap<u32, EpisodeDetails>>, String>
{
    use std::collections::{BTreeMap, BTreeSet};
    if id <= 0 {
        return Err("Invalid show ID".into());
    }
    #[derive(Deserialize)]
    struct SeasonRef {
        season_number: u32,
    }
    #[derive(Deserialize)]
    struct Show {
        id: i64,
        seasons: Vec<SeasonRef>,
    }
    let check_cancel = || {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            Err("Episode check cancelled".to_string())
        } else {
            Ok(())
        }
    };
    check_cancel()?;
    let show: Show = serde_json::from_value(request_json(
        &format!("{base}/tv/{id}"),
        token,
        &[("language", language)],
    )?)
    .map_err(|_| "Invalid TMDB show response")?;
    if show.id != id || show.seasons.len() > 1000 {
        return Err("Invalid TMDB show response".into());
    }
    let mut seen = BTreeSet::new();
    let mut result = BTreeMap::new();
    for season in show.seasons {
        check_cancel()?;
        let n = season.season_number;
        if n > 999 || !seen.insert(n) {
            return Err("Invalid TMDB season list".into());
        }
        let mut episodes = season_episodes_at(base, id, n, token, language)?;
        episodes.retain(|_, e| e.aired.as_deref().is_some_and(|d| d <= today));
        for (number, e) in &mut episodes {
            if e.title.trim().is_empty() {
                e.title = format!("Episode {number}");
            }
        }
        if !episodes.is_empty() {
            result.insert(n, episodes);
        }
    }
    Ok(result)
}

fn request_json(url: &str, token: &str, query: &[(&str, &str)]) -> Result<Value, String> {
    let token = token.trim().strip_prefix("Bearer ").unwrap_or(token.trim());
    if token.is_empty() {
        return Err("Enter a TMDB Read Access Token in Settings.".into());
    }
    let response = client()?
        .get(url)
        .bearer_auth(token)
        .query(query)
        .send()
        .map_err(|_| {
            "Could not load TMDB details. Check your connection and try again.".to_string()
        })?;
    if !response.status().is_success() {
        return Err(match response.status().as_u16() {
            401 | 403 => "TMDB rejected the Read Access Token.",
            429 => "TMDB request limit reached. Wait and try again.",
            _ => "TMDB could not load this candidate. Try again.",
        }
        .into());
    }
    let mut bytes = vec![];
    response
        .take(4_000_001)
        .read_to_end(&mut bytes)
        .map_err(|_| "Could not read TMDB details".to_string())?;
    if bytes.len() > 4_000_000 {
        return Err("TMDB response is too large".into());
    }
    serde_json::from_slice(&bytes).map_err(|_| "Invalid TMDB details response".to_string())
}
fn client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(8))
        .timeout(Duration::from_secs(25))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent("MediaShelf/0.5")
        .build()
        .map_err(|_| "Could not initialize network client".into())
}
fn string(v: &Value, key: &str) -> Option<String> {
    v.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}
fn array<'a>(v: &'a Value, key: &str) -> &'a [Value] {
    v.get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}
fn names(v: &Value, key: &str) -> Vec<String> {
    array(v, key)
        .iter()
        .filter_map(|x| string(x, "name"))
        .collect()
}

pub fn parse_details(
    v: &Value,
    category: Category,
    id: i64,
    language: &str,
) -> Result<Metadata, String> {
    if v.get("id").and_then(Value::as_i64) != Some(id) {
        return Err("TMDB returned details for a different item.".into());
    }
    let movie = category == Category::Movies;
    let title = string(v, if movie { "title" } else { "name" }).ok_or("Candidate has no title")?;
    let date = string(
        v,
        if movie {
            "release_date"
        } else {
            "first_air_date"
        },
    );
    let year = date
        .as_deref()
        .and_then(|d| d.get(..4))
        .and_then(|y| y.parse().ok());
    let mut out = Metadata::empty(TmdbMatch {
        category,
        id,
        title: title.clone(),
        original_title: string(
            v,
            if movie {
                "original_title"
            } else {
                "original_name"
            },
        )
        .unwrap_or(title),
        release_date: date,
        overview: string(v, "overview").unwrap_or_default(),
    });
    out.year = year;
    out.tagline = string(v, "tagline");
    out.rating = v.get("vote_average").and_then(Value::as_f64);
    out.votes = v.get("vote_count").and_then(Value::as_u64);
    out.runtime = v
        .get("runtime")
        .and_then(Value::as_u64)
        .or_else(|| array(v, "episode_run_time").first().and_then(Value::as_u64))
        .and_then(|x| u32::try_from(x).ok());
    out.genres = names(v, "genres");
    out.languages = array(v, "spoken_languages")
        .iter()
        .filter_map(|l| string(l, "english_name").or_else(|| string(l, "name")))
        .collect();
    out.countries = names(v, "production_countries");
    out.studios = names(v, "production_companies");
    out.tags = v
        .get("keywords")
        .map(|k| names(k, if movie { "keywords" } else { "results" }))
        .unwrap_or_default();
    out.collection = v
        .get("belongs_to_collection")
        .and_then(|c| string(c, "name"));
    out.imdb_id =
        string(v, "imdb_id").or_else(|| v.get("external_ids").and_then(|x| string(x, "imdb_id")));
    if let Some(credits) = v.get("credits") {
        out.cast = array(credits, "cast")
            .iter()
            .take(100)
            .filter_map(|p| {
                Some(Person {
                    name: string(p, "name")?,
                    role: string(p, "character").unwrap_or_default(),
                    tmdb_id: p.get("id").and_then(Value::as_u64),
                    profile_path: string(p, "profile_path"),
                })
            })
            .collect();
        out.crew = array(credits, "crew")
            .iter()
            .filter_map(|p| {
                Some(Person {
                    name: string(p, "name")?,
                    role: string(p, "job").unwrap_or_default(),
                    tmdb_id: p.get("id").and_then(Value::as_u64),
                    profile_path: string(p, "profile_path"),
                })
            })
            .collect();
    }
    out.crew
        .extend(array(v, "created_by").iter().filter_map(|p| {
            Some(Person {
                name: string(p, "name")?,
                role: "Creator".into(),
                tmdb_id: p.get("id").and_then(Value::as_u64),
                profile_path: string(p, "profile_path"),
            })
        }));
    out.trailer = v
        .get("videos")
        .and_then(|x| {
            array(x, "results")
                .iter()
                .find(|p| p["site"] == "YouTube" && p["type"] == "Trailer")
        })
        .and_then(|p| string(p, "key"))
        .filter(|k| {
            k.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        })
        .map(|k| format!("https://www.youtube.com/watch?v={k}"));
    let region = language.split('-').nth(1).unwrap_or("US");
    let ratings = v.get(if movie {
        "release_dates"
    } else {
        "content_ratings"
    });
    if let Some(ratings) = ratings {
        let rows = array(ratings, "results");
        let selected = rows
            .iter()
            .find(|r| r["iso_3166_1"] == region)
            .or_else(|| rows.iter().find(|r| r["iso_3166_1"] == "US"));
        out.certification = selected.and_then(|r| {
            if movie {
                array(r, "release_dates")
                    .iter()
                    .find_map(|d| string(d, "certification"))
            } else {
                string(r, "rating")
            }
        });
    }
    out.poster = string(v, "poster_path").filter(|s| image_url(s, false).is_some());
    out.fanart = string(v, "backdrop_path").filter(|s| image_url(s, true).is_some());
    Ok(out)
}

pub fn image_url(path: &str, fanart: bool) -> Option<String> {
    let name = path.strip_prefix('/')?;
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
        || name.contains("..")
        || !matches!(
            Path::new(name).extension()?.to_str()?,
            "jpg" | "png" | "webp"
        )
    {
        return None;
    }
    Some(format!(
        "https://image.tmdb.org/t/p/{}/{name}",
        if fanart { "w1280" } else { "w500" }
    ))
}

pub fn cache_artwork(data: &mut Metadata, fields: &[Field]) -> Vec<String> {
    let mut warnings = vec![];
    for fanart in [false, true] {
        if !fields.contains(&if fanart { Field::Fanart } else { Field::Poster }) {
            continue;
        }
        let (remote, local) = if fanart {
            (&data.fanart, &mut data.fanart_file)
        } else {
            (&data.poster, &mut data.poster_file)
        };
        if local.is_some() {
            continue;
        }
        let Some(remote) = remote else {
            continue;
        };
        let Some(url) = image_url(remote, fanart) else {
            continue;
        };
        let result = (|| -> Result<PathBuf, String> {
            let folder = database_path().parent().unwrap().join("artwork");
            let target = folder.join(format!(
                "{}-{}-{}",
                if data.identity.category == Category::Movies {
                    "movie"
                } else {
                    "tv"
                },
                data.identity.id,
                remote.trim_start_matches('/')
            ));
            if target.is_file() {
                return Ok(target);
            }
            let response = client()?
                .get(url)
                .send()
                .map_err(|_| "Artwork download failed".to_string())?;
            if !response.status().is_success() {
                return Err("Artwork unavailable".into());
            }
            let mut bytes = vec![];
            response
                .take(12_000_001)
                .read_to_end(&mut bytes)
                .map_err(|_| "Artwork read failed".to_string())?;
            if bytes.len() > 12_000_000 {
                return Err("Artwork is too large".into());
            }
            let mut reader = image::ImageReader::new(std::io::Cursor::new(&bytes))
                .with_guessed_format()
                .map_err(|_| "Unsupported artwork".to_string())?;
            let mut limits = image::Limits::default();
            limits.max_image_width = Some(8192);
            limits.max_image_height = Some(8192);
            limits.max_alloc = Some(96_000_000);
            reader.limits(limits);
            reader
                .decode()
                .map_err(|_| "Invalid artwork image".to_string())?;
            fs::create_dir_all(&folder).map_err(|_| "Cannot create artwork cache".to_string())?;
            if !target.exists() {
                let mut f = tempfile::NamedTempFile::new_in(&folder)
                    .map_err(|_| "Cannot save artwork".to_string())?;
                f.write_all(&bytes)
                    .map_err(|_| "Cannot save artwork".to_string())?;
                sync_output(f.as_file()).map_err(|_| "Cannot save artwork".to_string())?;
                if let Err(e) = f.persist_noclobber(&target) {
                    if e.error.kind() != std::io::ErrorKind::AlreadyExists {
                        return Err("Cannot save artwork".into());
                    }
                }
            }
            Ok(target)
        })();
        match result {
            Ok(path) => *local = Some(path),
            Err(e) => warnings.push(format!("{}: {e}", if fanart { "Fanart" } else { "Poster" })),
        }
    }
    warnings
}

pub fn series_folder(video: &Video, sources: &[Source]) -> Option<PathBuf> {
    if video.category != Category::Series {
        return None;
    }
    if video.is_folder {
        return Some(video.path.clone());
    }
    let source = sources
        .iter()
        .filter(|s| s.category == Category::Series && video.path.starts_with(&s.path))
        .max_by_key(|s| s.path.components().count())?;
    let relative = video.path.strip_prefix(&source.path).ok()?;
    let parts: Vec<_> = relative.components().collect();
    if parts.len() < 2 {
        return None;
    }
    let first = parts[0].as_os_str().to_string_lossy();
    if first.to_ascii_lowercase().starts_with("season") || first.eq_ignore_ascii_case("specials") {
        Some(source.path.clone())
    } else {
        Some(source.path.join(parts[0]))
    }
}

fn movie_xml(data: &Metadata) -> String {
    fn esc(value: &str) -> String {
        value
            .chars()
            .filter(|c| {
                matches!(*c, '\t' | '\n' | '\r')
                    || (*c >= '\u{20}' && *c != '\u{fffe}' && *c != '\u{ffff}')
            })
            .collect::<String>()
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&apos;")
    }
    fn tag(out: &mut String, key: &str, value: &str) {
        if value.is_empty() {
            out.push_str(&format!("  <{key}/>\n"));
        } else {
            out.push_str(&format!("  <{key}>{}</{key}>\n", esc(value)));
        }
    }
    fn person(out: &mut String, key: &str, p: &Person, profile: bool) {
        out.push_str(&format!("  <{key}>\n"));
        out.push_str(&format!(
            "    <name>{}</name>\n    <role>{}</role>\n",
            esc(&p.name),
            esc(&p.role)
        ));
        let photo = p
            .profile_path
            .as_deref()
            .and_then(|path| image_url(path, false))
            .map(|url| url.replace("/w500/", "/h632/"));
        match photo {
            Some(url) => out.push_str(&format!("    <thumb>{}</thumb>\n", esc(&url))),
            None => out.push_str("    <thumb/>\n"),
        }
        if profile {
            if let Some(id) = p.tmdb_id.filter(|id| *id > 0) {
                out.push_str(&format!(
                    "    <profile>https://www.themoviedb.org/person/{id}</profile>\n"
                ));
            } else {
                out.push_str("    <profile/>\n");
            }
        }
        out.push_str(&format!("  </{key}>\n"));
    }
    let mut out = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<!--created by Media Shelf {}-->\n<movie>\n",
        env!("CARGO_PKG_VERSION")
    );
    tag(&mut out, "title", &data.identity.title);
    tag(&mut out, "originaltitle", &data.identity.original_title);
    tag(&mut out, "sorttitle", "");
    tag(&mut out, "epbookmark", "");
    tag(
        &mut out,
        "year",
        &data.year.map(|n| n.to_string()).unwrap_or_default(),
    );
    if let Some(rating) = data.rating {
        out.push_str(&format!("  <ratings>\n    <rating default=\"true\" max=\"10\" name=\"themoviedb\">\n      <value>{rating}</value>\n      <votes>{}</votes>\n    </rating>\n  </ratings>\n", data.votes.unwrap_or(0)));
    } else {
        tag(&mut out, "ratings", "");
    }
    for key in ["userrating", "top250"] {
        tag(&mut out, key, "");
    }
    tag(
        &mut out,
        "set",
        data.collection.as_deref().unwrap_or_default(),
    );
    tag(&mut out, "plot", &data.identity.overview);
    tag(&mut out, "outline", &data.identity.overview);
    tag(
        &mut out,
        "tagline",
        data.tagline.as_deref().unwrap_or_default(),
    );
    tag(
        &mut out,
        "runtime",
        &data.runtime.map(|n| n.to_string()).unwrap_or_default(),
    );
    if let Some(url) = data.poster.as_deref().and_then(|p| image_url(p, false)) {
        out.push_str(&format!(
            "  <thumb aspect=\"poster\">{}</thumb>\n",
            esc(&url)
        ));
    } else {
        out.push_str("  <thumb aspect=\"poster\"/>\n");
    }
    if let Some(url) = data.fanart.as_deref().and_then(|p| image_url(p, true)) {
        out.push_str(&format!(
            "  <fanart>\n    <thumb>{}</thumb>\n  </fanart>\n",
            esc(&url.replace("/w1280/", "/original/"))
        ));
    } else {
        out.push_str("  <fanart/>\n");
    }
    for key in ["mpaa", "certification"] {
        tag(
            &mut out,
            key,
            data.certification.as_deref().unwrap_or_default(),
        );
    }
    tag(&mut out, "id", data.imdb_id.as_deref().unwrap_or_default());
    tag(&mut out, "tmdbid", &data.identity.id.to_string());
    tag(&mut out, "tmdbCollectionId", "");
    let imdb = data.imdb_id.as_deref().filter(|id| !id.is_empty());
    out.push_str(&format!(
        "  <uniqueid default=\"{}\" type=\"tmdb\">{}</uniqueid>\n",
        imdb.is_none(),
        data.identity.id
    ));
    if let Some(id) = imdb {
        out.push_str(&format!(
            "  <uniqueid default=\"true\" type=\"imdb\">{}</uniqueid>\n",
            esc(id)
        ));
    }
    for country in &data.countries {
        tag(&mut out, "country", country);
    }
    for key in ["status", "code"] {
        tag(&mut out, key, "");
    }
    tag(
        &mut out,
        "premiered",
        data.identity.release_date.as_deref().unwrap_or_default(),
    );
    for key in ["watched", "playcount", "lastplayed"] {
        tag(&mut out, key, "");
    }
    for genre in &data.genres {
        tag(&mut out, "genre", genre);
    }
    for studio in &data.studios {
        tag(&mut out, "studio", studio);
    }
    for p in data
        .crew
        .iter()
        .filter(|p| matches!(p.role.as_str(), "Writer" | "Screenplay" | "Story"))
    {
        tag(&mut out, "credits", &p.name);
    }
    for p in data
        .crew
        .iter()
        .filter(|p| matches!(p.role.as_str(), "Director" | "Creator"))
    {
        tag(&mut out, "director", &p.name);
    }
    for t in &data.tags {
        tag(&mut out, "tag", t);
    }
    for actor in &data.cast {
        person(&mut out, "actor", actor, true);
    }
    for producer in data
        .crew
        .iter()
        .filter(|p| matches!(p.role.as_str(), "Producer" | "Executive Producer"))
    {
        person(&mut out, "producer", producer, false);
    }
    let trailer = data.trailer.as_deref().unwrap_or_default();
    let trailer = trailer
        .strip_prefix("https://www.youtube.com/watch?v=")
        .filter(|key| {
            !key.is_empty()
                && key
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        })
        .map(|key| format!("plugin://plugin.video.youtube/?action=play_video&videoid={key}"))
        .unwrap_or_else(|| trailer.to_string());
    tag(&mut out, "trailer", &trailer);
    tag(&mut out, "languages", &data.languages.join(", "));
    tag(&mut out, "dateadded", "");
    out.push_str("  <fileinfo>\n    <streamdetails>\n      <video>\n        <codec/>\n        <aspect/>\n        <width/>\n        <height/>\n        <durationinseconds/>\n        <stereomode/>\n      </video>\n      <audio>\n        <codec/>\n        <language/>\n        <channels/>\n      </audio>\n    </streamdetails>\n  </fileinfo>\n  <!--Media Shelf metadata-->\n");
    tag(&mut out, "source", "");
    tag(&mut out, "edition", "");
    tag(
        &mut out,
        "original_filename",
        data.original_filename.as_deref().unwrap_or_default(),
    );
    tag(&mut out, "user_note", "");
    out.push_str("</movie>\n");
    out
}

pub fn xml(data: &Metadata) -> String {
    if data.identity.category == Category::Movies {
        return movie_xml(data);
    }
    fn escape(s: &str) -> String {
        s.chars()
            .filter(|c| {
                matches!(*c, '\t' | '\n' | '\r')
                    || (*c >= '\u{20}' && *c != '\u{fffe}' && *c != '\u{ffff}')
            })
            .collect::<String>()
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&apos;")
    }
    fn tag(out: &mut String, name: &str, value: &str) {
        out.push_str(&format!("  <{name}>{}</{name}>\n", escape(value)));
    }
    let root = if data.identity.category == Category::Movies {
        "movie"
    } else {
        "tvshow"
    };
    let mut out = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<{root}>\n");
    tag(&mut out, "title", &data.identity.title);
    tag(&mut out, "originaltitle", &data.identity.original_title);
    tag(&mut out, "plot", &data.identity.overview);
    for (key, value) in [
        ("tagline", data.tagline.as_deref()),
        ("premiered", data.identity.release_date.as_deref()),
        ("mpaa", data.certification.as_deref()),
        ("trailer", data.trailer.as_deref()),
    ] {
        if let Some(value) = value {
            tag(&mut out, key, value);
        }
    }
    if let Some(y) = data.year {
        tag(&mut out, "year", &y.to_string());
    }
    if let Some(r) = data.runtime {
        tag(&mut out, "runtime", &r.to_string());
    }
    out.push_str(&format!(
        "  <uniqueid type=\"tmdb\" default=\"true\">{}</uniqueid>\n",
        data.identity.id
    ));
    if let Some(id) = &data.imdb_id {
        out.push_str(&format!(
            "  <uniqueid type=\"imdb\">{}</uniqueid>\n",
            escape(id)
        ));
    }
    if let Some(r) = data.rating {
        out.push_str(&format!("  <ratings><rating name=\"tmdb\" max=\"10\" default=\"true\"><value>{r}</value><votes>{}</votes></rating></ratings>\n",data.votes.unwrap_or(0)));
    }
    for (name, values) in [
        ("genre", &data.genres),
        ("country", &data.countries),
        ("studio", &data.studios),
        ("tag", &data.tags),
    ] {
        for v in values {
            tag(&mut out, name, v);
        }
    }
    if let Some(set) = &data.collection {
        out.push_str(&format!("  <set><name>{}</name></set>\n", escape(set)));
    }
    for actor in &data.cast {
        out.push_str(&format!(
            "  <actor><name>{}</name><role>{}</role></actor>\n",
            escape(&actor.name),
            escape(&actor.role)
        ));
    }
    for person in &data.crew {
        match person.role.as_str() {
            "Director" | "Creator" => tag(&mut out, "director", &person.name),
            "Writer" | "Screenplay" | "Story" => tag(&mut out, "credits", &person.name),
            _ => {}
        }
    }
    if let Some(p) = data.poster.as_deref().and_then(|p| image_url(p, false)) {
        out.push_str(&format!(
            "  <thumb aspect=\"poster\">{}</thumb>\n",
            escape(&p)
        ));
    }
    if let Some(p) = data.fanart.as_deref().and_then(|p| image_url(p, true)) {
        out.push_str(&format!(
            "  <fanart><thumb>{}</thumb></fanart>\n",
            escape(&p)
        ));
    }
    out.push_str(&format!("</{root}>\n"));
    out
}

pub fn write_nfo(path: &Path, data: &Metadata, replace: bool) -> Result<(), String> {
    write_output(
        path,
        xml(data).as_bytes(),
        replace,
        &database_path().parent().unwrap().join("output-backups"),
    )
}

pub fn write_output(path: &Path, bytes: &[u8], replace: bool, backup: &Path) -> Result<(), String> {
    let mut output =
        tempfile::NamedTempFile::new_in(path.parent().ok_or("NFO has no parent folder")?)
            .map_err(|e| e.to_string())?;
    output.write_all(bytes).map_err(|e| e.to_string())?;
    sync_output(output.as_file()).map_err(|e| format!("Cannot flush output: {e}"))?;
    if path.exists() {
        if !replace {
            return Err(
                "Existing output kept. Enable Replace existing NFO/artwork to update it.".into(),
            );
        }
        if !fs::symlink_metadata(path)
            .map_err(|e| e.to_string())?
            .is_file()
        {
            return Err("NFO is not a regular file".into());
        }
        fs::create_dir_all(backup).map_err(|e| e.to_string())?;
        let name = format!(
            "{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            path.file_name().unwrap_or_default().to_string_lossy()
        );
        fs::copy(path, backup.join(name)).map_err(|e| format!("Cannot back up NFO: {e}"))?;
        output
            .persist(path)
            .map(|_| ())
            .map_err(|e| format!("Cannot write NFO: {}", e.error))
    } else {
        match output.persist_noclobber(path) {
            Ok(_) => Ok(()),
            #[cfg(target_os = "macos")]
            Err(error) if unsupported_macos_operation(&error.error) => {
                // shortcut: NAS without exclusive rename exposes new outputs while writing; use server-side atomic creation when available.
                create_output(path, bytes, sync_output)
                    .map_err(|e| format!("Cannot create output: {e}"))
            }
            Err(error) => Err(format!("Cannot create output: {}", error.error)),
        }
    }
}

#[cfg(target_os = "macos")]
fn unsupported_macos_operation(error: &std::io::Error) -> bool {
    error.raw_os_error() == Some(45) || error.kind() == std::io::ErrorKind::Unsupported
}

#[cfg(target_os = "macos")]
fn create_output(
    path: &Path,
    bytes: &[u8],
    sync: impl FnOnce(&fs::File) -> std::io::Result<()>,
) -> std::io::Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    if let Err(error) = file.write_all(bytes).and_then(|_| sync(&file)) {
        drop(file);
        if let Err(cleanup) = fs::remove_file(path)
            && cleanup.kind() != std::io::ErrorKind::NotFound
        {
            return Err(std::io::Error::other(format!(
                "{error}; could not remove partial output {}: {cleanup}",
                path.display()
            )));
        }
        return Err(error);
    }
    Ok(())
}

fn sync_output(file: &fs::File) -> std::io::Result<()> {
    sync_output_with(file, fs::File::sync_all)
}

fn sync_output_with(
    file: &fs::File,
    sync: impl FnOnce(&fs::File) -> std::io::Result<()>,
) -> std::io::Result<()> {
    let result = sync(file);
    #[cfg(target_os = "macos")]
    if let Err(error) = &result
        && unsupported_macos_operation(error)
    {
        use std::os::fd::AsRawFd;
        unsafe extern "C" {
            fn fsync(fd: std::ffi::c_int) -> std::ffi::c_int;
        }
        // macOS ENOTSUP (45): SMB may support fsync but not F_FULLFSYNC.
        loop {
            if unsafe { fsync(file.as_raw_fd()) } == 0 {
                return Ok(());
            }
            let error = std::io::Error::last_os_error();
            if error.kind() != std::io::ErrorKind::Interrupted {
                return Err(error);
            }
        }
    }
    result
}

#[cfg(all(test, target_os = "macos"))]
mod output_sync_tests {
    use super::*;

    #[test]
    fn guarded_creation_refuses_overwrites_and_removes_failed_partial_output() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("tvshow.nfo");
        create_output(&path, b"original", sync_output).unwrap();
        assert_eq!(
            create_output(&path, b"replacement", sync_output)
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::AlreadyExists
        );
        assert_eq!(fs::read(&path).unwrap(), b"original");
        let partial = temp.path().join("partial.nfo");
        assert!(
            create_output(&partial, b"partial", |_| Err(std::io::Error::from(
                std::io::ErrorKind::PermissionDenied
            )))
            .is_err()
        );
        assert!(!partial.exists());
    }

    #[test]
    fn unsupported_full_sync_falls_back_but_other_errors_are_not_ignored() {
        let file = tempfile::tempfile().unwrap();
        sync_output_with(&file, |_| Err(std::io::Error::from_raw_os_error(45))).unwrap();
        let error = sync_output_with(&file, |_| {
            Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
        })
        .unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
        sync_output(&file).unwrap();
    }
}

pub fn commit_scrape(
    db: &mut crate::storage::LibraryDb,
    target: &Video,
    new: &Metadata,
    options: &ScrapeOptions,
    whole_show: bool,
) -> crate::storage::DbResult<(usize, Vec<String>)> {
    if target.category != new.identity.category {
        return Err("Wrong scrape category".into());
    }
    let sources = db.sources()?;
    let videos = db.videos()?;
    let folder = if whole_show {
        series_folder(target, &sources)
    } else {
        None
    };
    let targets: Vec<_> = videos
        .into_iter()
        .filter(|v| {
            v.category == target.category
                && (v.path == target.path
                    || folder
                        .as_ref()
                        .is_some_and(|f| series_folder(v, &sources).as_ref() == Some(f)))
        })
        .collect();
    if !targets.iter().any(|v| v.path == target.path) {
        return Err("Library selection changed. Select the item again.".into());
    }
    let mut entries = vec![];
    let mut warnings = vec![];
    for video in &targets {
        let baseline = video.tmdb.clone().unwrap_or(TmdbMatch {
            category: video.category,
            id: new.identity.id,
            title: series_folder(video, &sources)
                .as_ref()
                .and_then(|p| p.file_name())
                .unwrap_or_else(|| video.path.file_stem().unwrap_or_default())
                .to_string_lossy()
                .into_owned(),
            original_title: String::new(),
            release_date: None,
            overview: String::new(),
        });
        let mut data = merge(video.metadata.as_ref(), baseline, new, options);
        if data.original_filename.is_none() {
            data.original_filename = video
                .path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned());
        }
        warnings.extend(cache_artwork(&mut data, &options.fields));
        entries.push((video.category, video.path.clone(), data));
    }
    // Commit library data first; optional external outputs report independent failures.
    db.save_scraped(&entries)?;
    let mut written = std::collections::HashSet::new();
    let mut nfo_results = std::collections::HashMap::new();
    for ((_, path, data), video) in entries.iter_mut().zip(&targets) {
        let show_folder = series_folder(video, &sources);
        let standard_movie = crate::file_ops::standard_movie_folder(video);
        let nfo = if video.category == Category::Movies {
            Some(if standard_movie {
                path.with_file_name("movie.nfo")
            } else {
                path.with_extension("nfo")
            })
        } else {
            show_folder.as_ref().map(|p| p.join("tvshow.nfo"))
        };
        if options.write_nfo {
            if let Some(nfo) = nfo {
                if written.insert(nfo.clone()) {
                    match write_nfo(&nfo, data, options.replace_nfo) {
                        Ok(()) => data.nfo_written = true,
                        Err(e) => {
                            data.nfo_written = false;
                            warnings.push(format!("{}: {e}", nfo.display()));
                        }
                    }
                    nfo_results.insert(nfo.clone(), data.nfo_written);
                } else {
                    data.nfo_written = nfo_results.get(&nfo).copied().unwrap_or(false);
                }
            } else {
                warnings.push(
                    "TV show folder could not be inferred; no NFO was written for this file."
                        .into(),
                );
            }
        }
        for (field, cache, label) in [
            (Field::Poster, data.poster_file.as_ref(), "poster"),
            (Field::Fanart, data.fanart_file.as_ref(), "fanart"),
        ] {
            if !options.fields.contains(&field) {
                continue;
            }
            if let Some(cache) = cache {
                let ext = cache.extension().unwrap_or_default().to_string_lossy();
                let output = if video.category == Category::Movies {
                    Some(if standard_movie {
                        path.with_file_name(format!("{label}.{ext}"))
                    } else {
                        path.with_file_name(format!(
                            "{}-{label}.{ext}",
                            path.file_stem().unwrap_or_default().to_string_lossy()
                        ))
                    })
                } else {
                    show_folder
                        .as_ref()
                        .map(|f| f.join(format!("{label}.{ext}")))
                };
                if let Some(output) = output
                    && written.insert(output.clone())
                {
                    let result = fs::read(cache)
                        .map_err(|e| e.to_string())
                        .and_then(|bytes| {
                            write_output(
                                &output,
                                &bytes,
                                options.replace_nfo,
                                &database_path().parent().unwrap().join("output-backups"),
                            )
                        });
                    if let Err(e) = result {
                        warnings.push(format!("{}: {e}", output.display()));
                    }
                }
            }
        }
    }
    db.save_scraped(&entries)?;
    warnings.sort();
    warnings.dedup();
    Ok((entries.len(), warnings))
}

#[cfg(test)]
mod request_tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn catalog_uses_explicit_seasons_and_only_aired_episodes_with_safe_header_auth() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let mut requests = vec![];
            for body in [
                r#"{"id":42,"seasons":[{"season_number":1},{"season_number":2}]}"#,
                r#"{"season_number":1,"episodes":[{"season_number":1,"episode_number":1,"name":"Pilot","air_date":"2020-01-01"},{"season_number":1,"episode_number":2,"name":"Second","air_date":"2020-01-02"},{"season_number":1,"episode_number":3,"name":"Third","air_date":"2020-01-03"}]}"#,
                r#"{"season_number":2,"episodes":[{"season_number":2,"episode_number":1,"name":"Return","air_date":"2021-01-01"},{"season_number":2,"episode_number":2,"name":"Future","air_date":"2099-01-01"},{"season_number":2,"episode_number":3,"name":"Unknown","air_date":null},{"season_number":2,"episode_number":4,"name":"Invalid date","air_date":"2020-99-99"}]}"#,
            ] {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let mut request = vec![];
                while !request.ends_with(b"\r\n\r\n") {
                    let mut b = [0];
                    stream.read_exact(&mut b).unwrap();
                    request.push(b[0]);
                }
                requests.push(String::from_utf8(request).unwrap());
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .unwrap();
            }
            requests
        });
        let cancel = std::sync::atomic::AtomicBool::new(false);
        let catalog =
            episode_catalog_at(&base, 42, "private-token", "en-US", "2026-10-22", &cancel).unwrap();
        assert_eq!(catalog[&1].len(), 3);
        assert_eq!(
            catalog[&2],
            std::collections::BTreeMap::from([(1, "Return".into())])
        );
        let requests = server.join().unwrap();
        for (request, path) in
            requests
                .iter()
                .zip(["/tv/42?", "/tv/42/season/1?", "/tv/42/season/2?"])
        {
            assert!(request.starts_with(&format!("GET {path}language=en-US ")));
            assert!(!request.lines().next().unwrap().contains("private-token"));
            assert!(
                request
                    .to_lowercase()
                    .contains("authorization: bearer private-token")
            );
        }
        cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        assert_eq!(
            episode_catalog_at(&base, 42, "private-token", "en-US", "2026-10-22", &cancel)
                .unwrap_err(),
            "Episode check cancelled"
        );
        for (seconds, date) in [
            (0, "1970-01-01"),
            (951782400, "2000-02-29"),
            (1709164800, "2024-02-29"),
            (1709251200, "2024-03-01"),
        ] {
            assert_eq!(utc_date(seconds), date);
        }
    }

    #[test]
    fn season_titles_use_header_auth_and_reject_invalid_responses() {
        for (status, body, valid) in [
            (
                200,
                r#"{"season_number":1,"episodes":[{"episode_number":1,"name":"Pilot","air_date":"2022-07-15","vote_average":7.8}]}"#,
                true,
            ),
            (200, r#"{"season_number":2,"episodes":[]}"#, false),
            (
                200,
                r#"{"season_number":1,"episodes":[{"episode_number":1,"name":"A"},{"episode_number":1,"name":"B"}]}"#,
                false,
            ),
            (401, "private-token sensitive server response", false),
        ] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let server = std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let mut request = vec![];
                while !request.ends_with(b"\r\n\r\n") {
                    let mut byte = [0];
                    stream.read_exact(&mut byte).unwrap();
                    request.push(byte[0]);
                }
                write!(stream, "HTTP/1.1 {status} Result\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
                String::from_utf8(request).unwrap()
            });
            let result = season_episodes_at(
                &format!("http://{address}"),
                42,
                1,
                "private-token",
                "en-US",
            );
            let request = server.join().unwrap();
            assert!(request.starts_with("GET /tv/42/season/1?language=en-US "));
            assert!(!request.lines().next().unwrap().contains("private-token"));
            assert!(
                request
                    .to_lowercase()
                    .contains("authorization: bearer private-token")
            );
            assert_eq!(result.is_ok(), valid);
            match result {
                Ok(details) => {
                    let episode = &details[&1];
                    assert_eq!(episode.title, "Pilot");
                    assert_eq!(episode.aired.as_deref(), Some("2022-07-15"));
                    assert_eq!(episode.rating, Some(7.8));
                }
                Err(error) => assert!(
                    !error.contains("private-token")
                        && !error.contains("sensitive server response")
                ),
            }
        }
    }
    #[test]
    fn preview_requests_full_details_in_selected_language_without_query_credentials() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut request = vec![];
            while !request.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                stream.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
            }
            let body = r#"{"id":42,"title":"French title","release_date":"2020-01-01","credits":{"cast":[{"name":"Actor","character":"Role"}]}}"#;
            write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
            String::from_utf8(request).unwrap()
        });
        let data = details_at(
            &format!("http://{address}"),
            Category::Movies,
            42,
            "private-token",
            "fr-FR",
        )
        .unwrap();
        let request = server.join().unwrap();
        assert_eq!(data.identity.title, "French title");
        assert_eq!(data.cast.len(), 1);
        assert!(request.starts_with("GET /movie/42?"));
        assert!(request.contains("language=fr-FR"));
        assert!(request.contains("append_to_response=credits"));
        assert!(
            request
                .to_lowercase()
                .contains("authorization: bearer private-token")
        );
        assert!(!request.lines().next().unwrap().contains("private-token"));
    }
}
