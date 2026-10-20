# TMM flow reference and implemented scope

Reference UI: the user's two screenshots of tinyMediaManager **3.1.16.1**.
Official documentation was checked on 20 October 2026. Current documentation
also covers later TMM versions; this implementation follows the supplied layout
and supported behavior, rather than claiming complete v3 feature parity.

## What the original flow establishes

1. **Update sources** imports local files/NFOs into the library.
2. **Search and scrape** searches the preferred scraper for selected library items.
3. Selecting a candidate loads a preview; it is not the commit step.
4. Scrape options control which metadata is applied and whether existing values
   may be overwritten.
5. Confirmation downloads selected metadata/artwork and writes configured outputs.
6. Renaming is separate, or can be configured as an automatic post-scrape action.
7. TV management uses show/season/episode hierarchy.

Sources:

- https://www.tinymediamanager.org/docs/quickstart
- https://www.tinymediamanager.org/docs/movies/settings
- https://www.tinymediamanager.org/docs/movies/moviepanel
- https://www.tinymediamanager.org/docs/tvshows/tvshowpanel
- https://www.tinymediamanager.org/docs/tvshows/settings

Earlier read-only inspection of the installed JAR identified separate chooser,
scrape, metadata configuration, artwork provider, threshold, and NFO-writing
components. This was static inspection, not an end-to-end UI verification.

## v0.5 implementation

- Light library/detail layout with a dark toolbar, compact table, poster/fanart,
  detail tabs and TV show grouping.
- Separate search results and full-details preview, selectable language,
  cast/crew table and explicit scrape-field selection.
- Selection loads details via TMDB's movie/TV details endpoints with appended
  credits, keywords, videos and certification data. Confirmation merges selected
  fields, stores them in SQLite and writes optional NFO/artwork output.
- Title/original title, tagline, plot, year/date, rating, runtime, certification,
  genres, languages, country, studio, tags, collection, trailer, cast/crew,
  poster and fanart are supported. Some fields can be absent at TMDB.
- Artwork previews use TMDB's CDN; confirmed artwork is downloaded locally.
- Multi-movie review queue, skip/cancel, known-ID re-scrape and unscraped-item scope.

TMDB references:

- https://developer.themoviedb.org/reference/movie-details
- https://developer.themoviedb.org/reference/tv-series-details
- https://developer.themoviedb.org/docs/append-to-response
- https://developer.themoviedb.org/docs/image-basics

Not implemented: additional scraper providers, Top 250, banner/clearart/discart,
manual selection between artwork variants, automatic confidence-based matching,
MediaInfo codec badges, full local NFO import, and individual episode scraping.
No unavailable field is represented as a functioning scrape control.
