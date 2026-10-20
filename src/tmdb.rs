//! TMDB searches send only an editable title query, category, and page number.
use crate::Category;
use reqwest::{StatusCode, blocking::Client};
use serde::{Deserialize, Serialize};
use std::{io::Read, time::Duration};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TmdbMatch {
    pub category: Category,
    pub id: i64,
    pub title: String,
    pub original_title: String,
    pub release_date: Option<String>,
    pub overview: String,
}

impl TmdbMatch {
    pub fn year(&self) -> &str {
        self.release_date
            .as_deref()
            .and_then(|s| s.get(..4))
            .filter(|s| s.chars().all(|c| c.is_ascii_digit()))
            .unwrap_or("Unknown year")
    }
    pub fn url(&self) -> String {
        format!(
            "https://www.themoviedb.org/{}/{}",
            match self.category {
                Category::Movies => "movie",
                Category::Series => "tv",
            },
            self.id
        )
    }
}

#[derive(Debug)]
pub struct SearchPage {
    pub page: u32,
    pub total_pages: u32,
    pub results: Vec<TmdbMatch>,
}

#[derive(Deserialize)]
struct ApiPage {
    page: u32,
    total_pages: u32,
    results: Vec<ApiResult>,
}
#[derive(Deserialize)]
struct ApiResult {
    id: i64,
    title: Option<String>,
    name: Option<String>,
    original_title: Option<String>,
    original_name: Option<String>,
    release_date: Option<String>,
    first_air_date: Option<String>,
    overview: Option<String>,
}

pub fn search(
    category: Category,
    query: &str,
    token: &str,
    page: u32,
) -> Result<SearchPage, String> {
    search_at("https://api.themoviedb.org/3", category, query, token, page)
}

fn search_at(
    base: &str,
    category: Category,
    query: &str,
    token: &str,
    page: u32,
) -> Result<SearchPage, String> {
    search_at_language(base, category, query, token, page, "en-US")
}

pub fn search_language(
    category: Category,
    query: &str,
    token: &str,
    page: u32,
    language: &str,
) -> Result<SearchPage, String> {
    search_at_language(
        "https://api.themoviedb.org/3",
        category,
        query,
        token,
        page,
        language,
    )
}

fn search_at_language(
    base: &str,
    category: Category,
    query: &str,
    token: &str,
    page: u32,
    language: &str,
) -> Result<SearchPage, String> {
    let query = query.trim();
    if query.is_empty() {
        return Err("Enter a movie or series title.".into());
    }
    if query.chars().count() > 200 {
        return Err("Keep the search title under 200 characters.".into());
    }
    let token = token.trim().strip_prefix("Bearer ").unwrap_or(token.trim());
    if token.is_empty() {
        return Err("Enter your TMDB API Read Access Token in TMDB settings.".into());
    }
    if page == 0 || page > 500 {
        return Err("Choose a page between 1 and 500.".into());
    }
    let client = Client::builder()
        .connect_timeout(Duration::from_secs(8))
        .timeout(Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent("MediaShelf/0.5")
        .build()
        .map_err(|_| "Could not initialize the TMDB connection.".to_string())?;
    let endpoint = match category {
        Category::Movies => "movie",
        Category::Series => "tv",
    };
    let response = client
        .get(format!("{base}/search/{endpoint}"))
        .bearer_auth(token)
        .query(&[
            ("query", query),
            ("page", &page.to_string()),
            ("language", language),
            ("include_adult", "false"),
        ])
        .send()
        .map_err(|e| {
            if e.is_timeout() {
                "TMDB timed out. Try again.".to_string()
            } else {
                "Could not reach TMDB. Check your connection and try again.".to_string()
            }
        })?;
    match response.status() {
        StatusCode::OK => {},
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => return Err("TMDB rejected the token. Use the API Read Access Token from your TMDB account settings.".into()),
        StatusCode::TOO_MANY_REQUESTS => return Err("TMDB is limiting requests. Wait a moment and try again.".into()),
        status if status.is_server_error() => return Err("TMDB is temporarily unavailable. Try again later.".into()),
        _ => return Err("TMDB could not complete this search. Check the title and try again.".into()),
    }
    // Limit unexpected responses without logging bodies, credentials, or request URLs.
    let mut bytes = Vec::new();
    response
        .take(2_000_001)
        .read_to_end(&mut bytes)
        .map_err(|_| "Could not read the TMDB response.".to_string())?;
    if bytes.len() > 2_000_000 {
        return Err("TMDB returned an oversized response.".into());
    }
    parse_page(&bytes, category)
}

fn parse_page(bytes: &[u8], category: Category) -> Result<SearchPage, String> {
    let page: ApiPage = serde_json::from_slice(bytes)
        .map_err(|_| "TMDB returned an unreadable response. Try again.".to_string())?;
    let mut results = Vec::new();
    for row in page.results {
        let (title, original_title, date) = match category {
            Category::Movies => (row.title, row.original_title, row.release_date),
            Category::Series => (row.name, row.original_name, row.first_air_date),
        };
        let Some(title) = title.filter(|t| !t.trim().is_empty()) else {
            continue;
        };
        if row.id <= 0 {
            continue;
        }
        results.push(TmdbMatch {
            category,
            id: row.id,
            original_title: original_title.unwrap_or_else(|| title.clone()),
            title,
            release_date: date.filter(|d| !d.is_empty()),
            overview: row.overview.unwrap_or_default(),
        });
    }
    Ok(SearchPage {
        page: page.page,
        total_pages: page.total_pages.min(500),
        results,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
    };

    fn mock_search(
        category: Category,
        body: &'static str,
        status: u16,
    ) -> (Result<SearchPage, String>, String) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                stream.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
            }
            write!(stream,"HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
            String::from_utf8(request).unwrap()
        });
        let result = search_at(
            &format!("http://{address}"),
            category,
            "Amélie & Co",
            "test-token",
            2,
        );
        (result, server.join().unwrap())
    }

    #[test]
    fn movies_use_encoded_queries_and_header_authentication() {
        let (result, request) = mock_search(
            Category::Movies,
            r#"{"page":2,"total_pages":3,"results":[{"id":42,"title":"Amélie","original_title":"Le Fabuleux Destin","release_date":"2001-04-25","overview":"A story"}]}"#,
            200,
        );
        let page = result.unwrap();
        assert_eq!(page.page, 2);
        assert_eq!(page.total_pages, 3);
        assert_eq!(page.results[0].year(), "2001");
        assert!(request.starts_with("GET /search/movie?"));
        assert!(request.contains("query=Am%C3%A9lie+%26+Co"));
        assert!(
            request
                .to_lowercase()
                .contains("authorization: bearer test-token")
        );
        assert!(!request.lines().next().unwrap().contains("test-token"));
    }

    #[test]
    fn series_map_names_and_first_air_dates() {
        let (result, request) = mock_search(
            Category::Series,
            r#"{"page":2,"total_pages":2,"results":[{"id":9,"name":"A Show","original_name":"Original Show","first_air_date":"2020-01-01","overview":null},{"id":10,"name":"Unknown date","first_air_date":null}]}"#,
            200,
        );
        let page = result.unwrap();
        assert!(request.starts_with("GET /search/tv?"));
        assert_eq!(page.results[0].title, "A Show");
        assert_eq!(page.results[0].year(), "2020");
        assert_eq!(page.results[1].year(), "Unknown year");
        assert_eq!(page.results[0].url(), "https://www.themoviedb.org/tv/9");
    }

    #[test]
    fn failures_do_not_expose_credentials_or_server_bodies() {
        for status in [401, 403, 429, 503] {
            let (result, _) = mock_search(Category::Movies, "sensitive response body", status);
            let error = result.unwrap_err();
            assert!(!error.contains("test-token"));
            assert!(!error.contains("sensitive"));
        }
        assert!(search(Category::Movies, " ", "", 1).is_err());
        assert!(search(Category::Movies, "Film", "", 1).is_err());
        assert!(search(Category::Movies, "Film", "token", 0).is_err());
        assert!(parse_page(b"broken", Category::Movies).is_err());
        let empty = parse_page(
            br#"{"page":1,"total_pages":0,"results":[]}"#,
            Category::Movies,
        )
        .unwrap();
        assert!(empty.results.is_empty());
    }
}
