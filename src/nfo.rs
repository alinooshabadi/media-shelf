//! Read local movie and show NFOs without changing them or contacting a provider.
use crate::{
    Category, Video,
    metadata::{Metadata, Person},
    tmdb::TmdbMatch,
};
use std::{fs, io::Read};

pub struct LocalMovie {
    pub title: String,
    pub year: Option<u16>,
    pub metadata: Option<Metadata>,
}

pub fn read(video: &Video) -> Result<Option<LocalMovie>, String> {
    if video.category == Category::Series {
        if !video.is_folder {
            return Ok(None);
        }
        return read_file(&video.path.join("tvshow.nfo"), Category::Series);
    }
    let parent = video.path.parent().ok_or("Missing movie directory")?;
    let entries: Vec<_> = match fs::read_dir(parent) {
        Ok(e) => e.filter_map(Result::ok).collect(),
        Err(_) => return Ok(None),
    };
    let single = entries
        .iter()
        .filter(|e| crate::is_video(&e.path()) && !crate::identification::sample_file(&e.path()))
        .count()
        == 1;
    let stem = video.path.file_stem().unwrap_or_default().to_string_lossy();
    let mut names = vec![format!("{stem}.nfo")];
    if single {
        names.insert(0, "movie.nfo".into());
    }
    for name in names {
        if let Some(entry) = entries
            .iter()
            .find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(&name))
        {
            return read_file(&entry.path(), Category::Movies);
        }
    }
    Ok(None)
}

fn read_file(path: &std::path::Path, category: Category) -> Result<Option<LocalMovie>, String> {
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("Could not read local NFO".into()),
    };
    let mut bytes = Vec::new();
    file.take(4 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Could not read local NFO")?;
    if bytes.len() > 4 * 1024 * 1024 {
        return Err("Local NFO exceeds 4 MiB".into());
    }
    let text = std::str::from_utf8(&bytes).map_err(|_| "Local NFO must use UTF-8")?;
    parse_category(text.trim_start_matches('\u{feff}'), category).map(Some)
}

pub fn parse(text: &str) -> Result<LocalMovie, String> {
    parse_category(text, Category::Movies)
}

fn parse_category(text: &str, category: Category) -> Result<LocalMovie, String> {
    let doc = roxmltree::Document::parse_with_options(
        text,
        roxmltree::ParsingOptions {
            allow_dtd: false,
            nodes_limit: 100_000,
            ..Default::default()
        },
    )
    .map_err(|_| "Invalid local NFO XML")?;
    let root = doc.root_element();
    let tag = if category == Category::Movies {
        "movie"
    } else {
        "tvshow"
    };
    if !root.has_tag_name(tag) {
        return Err(format!("Expected a {tag} NFO"));
    }
    let get = |key| {
        root.children()
            .find(|n| n.has_tag_name(key))
            .and_then(|n| n.text())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };
    let title = get("title").ok_or("NFO title is missing")?;
    let year = get("year")
        .and_then(|v| v.parse::<u16>().ok())
        .filter(|y| (1800..=2199).contains(y));
    let id = root
        .children()
        .find(|n| n.has_tag_name("uniqueid") && n.attribute("type") == Some("tmdb"))
        .and_then(|n| n.text())
        .and_then(|v| v.trim().parse::<i64>().ok())
        .or_else(|| get("tmdbid").and_then(|v| v.parse().ok()))
        .filter(|id| *id > 0);
    if id.is_none() && category == Category::Movies {
        return Ok(LocalMovie {
            title,
            year,
            metadata: None,
        });
    }
    // Local-only series metadata has no provider match; storage never saves ID 0 as one.
    let id = id.unwrap_or(0);
    let identity = TmdbMatch {
        category,
        id,
        title: title.clone(),
        original_title: get("originaltitle").unwrap_or_else(|| title.clone()),
        release_date: get("premiered").or_else(|| year.map(|y| y.to_string())),
        overview: get("plot").or_else(|| get("outline")).unwrap_or_default(),
    };
    let mut data = Metadata::empty(identity);
    data.year = year.or_else(|| data.identity.year().parse().ok());
    data.tagline = get("tagline");
    data.runtime = get("runtime").and_then(|v| v.parse().ok());
    data.certification = get("certification").or_else(|| get("mpaa"));
    data.imdb_id = root
        .children()
        .find(|n| n.has_tag_name("uniqueid") && n.attribute("type") == Some("imdb"))
        .and_then(|n| n.text())
        .map(str::to_string)
        .or_else(|| get("id").filter(|id| id.starts_with("tt")));
    for (target, key) in [
        (&mut data.genres, "genre"),
        (&mut data.countries, "country"),
        (&mut data.studios, "studio"),
        (&mut data.tags, "tag"),
    ] {
        *target = root
            .children()
            .filter(|n| n.has_tag_name(key))
            .filter_map(|n| n.text())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect();
    }
    data.languages = get("languages")
        .map(|v| {
            v.split(',')
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    data.collection = root
        .children()
        .find(|n| n.has_tag_name("set"))
        .and_then(|n| {
            n.children()
                .find(|c| c.has_tag_name("name"))
                .and_then(|c| c.text())
                .or_else(|| n.text())
        })
        .map(str::to_string);
    let ratings: Vec<_> = root
        .descendants()
        .filter(|n| n.has_tag_name("rating"))
        .collect();
    if let Some(r) = ratings
        .iter()
        .find(|n| matches!(n.attribute("name"), Some("themoviedb" | "tmdb")))
        .or_else(|| ratings.first())
    {
        data.rating = r
            .children()
            .find(|n| n.has_tag_name("value"))
            .and_then(|n| n.text())
            .or_else(|| r.text())
            .and_then(|v| v.trim().parse::<f64>().ok())
            .and_then(|v| {
                let max = r.attribute("max").unwrap_or("10").parse::<f64>().ok()?;
                (v.is_finite() && max.is_finite() && max > 0.0 && (0.0..=max).contains(&v))
                    .then_some(v * (10.0 / max))
            });
        data.votes = r
            .children()
            .find(|n| n.has_tag_name("votes"))
            .and_then(|n| n.text())
            .and_then(|v| v.parse().ok());
    }
    for node in root
        .children()
        .filter(|n| n.has_tag_name("actor") || n.has_tag_name("producer"))
    {
        let field = |key| {
            node.children()
                .find(|n| n.has_tag_name(key))
                .and_then(|n| n.text())
                .map(str::to_string)
        };
        if let Some(name) = field("name") {
            let p = Person {
                name,
                role: field("role").unwrap_or_default(),
                tmdb_id: field("profile")
                    .and_then(|s| s.rsplit('/').next().and_then(|id| id.parse().ok())),
                profile_path: field("thumb").and_then(|url| asset(&url)),
            };
            if node.has_tag_name("actor") {
                data.cast.push(p);
            } else {
                data.crew.push(p);
            }
        }
    }
    for (tag, role) in [("director", "Director"), ("credits", "Writer")] {
        for node in root.children().filter(|n| n.has_tag_name(tag)) {
            if let Some(name) = node.text() {
                data.crew.push(Person {
                    name: name.into(),
                    role: role.into(),
                    ..Default::default()
                });
            }
        }
    }
    data.poster = root
        .children()
        .find(|n| n.has_tag_name("thumb") && n.attribute("aspect") == Some("poster"))
        .and_then(|n| n.text())
        .and_then(asset);
    data.fanart = root
        .children()
        .find(|n| n.has_tag_name("fanart"))
        .and_then(|n| n.children().find(|n| n.has_tag_name("thumb")))
        .and_then(|n| n.text())
        .and_then(asset);
    data.trailer = get("trailer").and_then(|s| {
        if let Some(key) =
            s.strip_prefix("plugin://plugin.video.youtube/?action=play_video&videoid=")
        {
            if key
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            {
                return Some(format!("https://www.youtube.com/watch?v={key}"));
            }
        }
        if s.starts_with("https://www.youtube.com/") {
            Some(s)
        } else {
            None
        }
    });
    data.original_filename = get("original_filename");
    data.nfo_written = true;
    Ok(LocalMovie {
        title,
        year: data.year,
        metadata: Some(data),
    })
}

fn asset(url: &str) -> Option<String> {
    let url = reqwest::Url::parse(url).ok()?;
    if url.host_str()? != "image.tmdb.org" {
        return None;
    }
    let path = format!("/{}", url.path_segments()?.next_back()?);
    crate::metadata::image_url(&path, false)?;
    Some(path)
}
