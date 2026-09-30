use serde::Deserialize;

const BASE_URL: &str = "https://www.instapaper.com/api/2";
pub const BOOKMARKS_LIMIT: u32 = 25;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Home,
    Liked,
    Archive,
}

impl Section {
    pub fn query_value(self) -> &'static str {
        match self {
            Section::Home => "home",
            Section::Liked => "liked",
            Section::Archive => "archive",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Bookmark {
    pub id: i64,
    #[serde(default)]
    pub image: Option<String>,
    pub url: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub liked: bool,
    pub archived: bool,
    #[serde(default)]
    pub time: Option<i64>,
}

impl Bookmark {
    pub fn reader_url(&self) -> String {
        format!("https://instapaper.com/read/{}", self.id)
    }

    pub fn display_title(&self) -> String {
        self.title
            .clone()
            .filter(|t| !t.is_empty())
            .or_else(|| self.url.clone())
            .unwrap_or_else(|| "Untitled".to_string())
    }

    pub fn added_date(&self) -> Option<String> {
        let date = glib::DateTime::from_unix_local(self.time?).ok()?;
        date.format("%B %-d, %Y").ok().map(|text| text.to_string())
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ArticleAuthor {
    pub name: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ArticleMetadata {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub author: Option<ArticleAuthor>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ArticleContent {
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub words: Option<i64>,
    #[serde(default)]
    pub paywalled: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ParsedArticle {
    #[serde(default)]
    pub metadata: ArticleMetadata,
    #[serde(default)]
    pub content: ArticleContent,
}

#[derive(Debug, Deserialize)]
struct BookmarkList {
    bookmarks: Vec<Bookmark>,
    #[serde(default)]
    total: u64,
}

#[derive(Debug, Clone)]
pub struct BookmarkPage {
    pub bookmarks: Vec<Bookmark>,
    pub total: u64,
}

pub fn page_count(total: u64) -> u64 {
    total.div_ceil(BOOKMARKS_LIMIT as u64).max(1)
}

pub fn offset_for_page(page: u64) -> u32 {
    page.saturating_mul(BOOKMARKS_LIMIT as u64)
        .min(u32::MAX as u64) as u32
}

#[derive(Debug, Deserialize)]
struct ApiErrorBody {
    error: ApiErrorDetail,
}

#[derive(Debug, Deserialize)]
struct ApiErrorDetail {
    message: String,
}

#[derive(Debug)]
pub enum Error {
    Request(reqwest::Error),
    Api { code: u32, message: String },
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Request(e) => write!(f, "{e}"),
            Error::Api { code, message } => write!(f, "Instapaper error {code}: {message}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Self {
        Error::Request(e)
    }
}

#[derive(Clone)]
pub struct Client {
    token: String,
    http: reqwest::blocking::Client,
}

impl Client {
    pub fn new(token: String) -> Self {
        Self {
            token,
            http: reqwest::blocking::Client::new(),
        }
    }

    pub fn bookmarks(&self, section: Section, offset: u32) -> Result<BookmarkPage, Error> {
        let url = format!("{BASE_URL}/bookmarks");
        let response = self
            .http
            .get(url)
            .query(&[
                ("section", section.query_value()),
                ("limit", BOOKMARKS_LIMIT.to_string().as_str()),
                ("offset", offset.to_string().as_str()),
            ])
            .bearer_auth(&self.token)
            .send()?;
        let list: BookmarkList = Self::decode_json(response)?;
        Ok(BookmarkPage {
            bookmarks: list.bookmarks,
            total: list.total,
        })
    }

    pub fn image_bytes(&self, url: &str) -> Result<Vec<u8>, Error> {
        let response = self.http.get(url).send()?;
        let response = self.check(response)?;
        let bytes = response.bytes()?;
        Ok(bytes.to_vec())
    }

    pub fn count(&self, section: Section) -> Result<u64, Error> {
        let url = format!("{BASE_URL}/bookmarks");
        let response = self
            .http
            .get(url)
            .query(&[("section", section.query_value()), ("limit", "1")])
            .bearer_auth(&self.token)
            .send()?;
        let list: BookmarkList = Self::decode_json(response)?;
        Ok(list.total)
    }

    fn decode_json<T: serde::de::DeserializeOwned>(
        response: reqwest::blocking::Response,
    ) -> Result<T, Error> {
        let text = response.text()?;
        match serde_json::from_str(&text) {
            Ok(parsed) => Ok(parsed),
            Err(_) => {
                let snippet: String = text.chars().take(300).collect();
                Err(Error::Api {
                    code: 0,
                    message: format!("unexpected response: {snippet}"),
                })
            }
        }
    }

    pub fn article(&self, id: i64) -> Result<ParsedArticle, Error> {
        let url = format!("{BASE_URL}/bookmarks/{id}/parse");
        let response = self.http.get(url).bearer_auth(&self.token).send()?;
        let parsed: ParsedArticle = self.check(response)?.json()?;
        Ok(parsed)
    }

    pub fn archive(&self, id: i64) -> Result<(), Error> {
        self.move_to(id, "archive")
    }

    pub fn unarchive(&self, id: i64) -> Result<(), Error> {
        self.move_to(id, "home")
    }

    fn move_to(&self, id: i64, section: &str) -> Result<(), Error> {
        let url = format!("{BASE_URL}/bookmarks/{id}/move");
        let response = self
            .http
            .post(url)
            .json(&serde_json::json!({ "section": section }))
            .bearer_auth(&self.token)
            .send()?;
        self.check(response)?;
        Ok(())
    }

    pub fn delete(&self, id: i64) -> Result<(), Error> {
        let url = format!("{BASE_URL}/bookmarks/{id}");
        let response = self.http.delete(url).bearer_auth(&self.token).send()?;
        self.check(response)?;
        Ok(())
    }

    pub fn set_liked(&self, id: i64, liked: bool) -> Result<(), Error> {
        let url = format!("{BASE_URL}/bookmarks/{id}/like");
        let request = if liked {
            self.http.post(&url)
        } else {
            self.http.delete(&url)
        };
        let response = request.bearer_auth(&self.token).send()?;
        self.check(response)?;
        Ok(())
    }

    fn check(
        &self,
        response: reqwest::blocking::Response,
    ) -> Result<reqwest::blocking::Response, Error> {
        if response.status().is_success() {
            return Ok(response);
        }
        let code = response.status().as_u16();
        let message = response
            .json::<ApiErrorBody>()
            .ok()
            .map(|body| body.error.message)
            .unwrap_or_else(|| format!("HTTP {code}"));
        Err(Error::Api {
            code: code.into(),
            message,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bookmark_list() {
        let json = r#"{
            "bookmarks": [
                {
                    "id": 123,
                    "url": "https://example.com/a",
                    "image": "https://example.com/a/thumb.jpg",
                    "title": "An Article",
                    "description": "first line of text",
                    "progress": { "percentage": 0.5, "timestamp": 1700000000 },
                    "liked": true,
                    "archived": false,
                    "time": 1700000000,
                    "tags": [],
                    "category": 0
                },
                {
                    "id": 456,
                    "url": null,
                    "image": null,
                    "title": null,
                    "description": null,
                    "progress": { "percentage": 0.0, "timestamp": 1700000001 },
                    "liked": false,
                    "archived": false,
                    "time": 1700000001,
                    "tags": [],
                    "category": 0
                }
            ],
            "total": 2
        }"#;

        let list: BookmarkList = serde_json::from_str(json).expect("should parse");
        assert_eq!(list.bookmarks.len(), 2);
        assert_eq!(list.total, 2);

        let first = &list.bookmarks[0];
        assert_eq!(first.id, 123);
        assert_eq!(
            first.image.as_deref(),
            Some("https://example.com/a/thumb.jpg")
        );
        assert_eq!(first.display_title(), "An Article");
        assert!(first.liked);
        assert!(!first.archived);

        let second = &list.bookmarks[1];
        assert_eq!(second.image, None);
        assert_eq!(second.display_title(), "Untitled");
    }

    #[test]
    fn parses_api_error_body() {
        let json = r#"{ "error": { "code": 401, "message": "Authentication failed" } }"#;
        let body: ApiErrorBody = serde_json::from_str(json).expect("should parse");
        assert_eq!(body.error.message, "Authentication failed");
    }

    #[test]
    fn reader_url_uses_bookmark_id() {
        let bookmark = Bookmark {
            id: 2045166304,
            image: None,
            url: Some("https://example.com/a".to_string()),
            title: Some("An Article".to_string()),
            description: None,
            liked: false,
            archived: false,
            time: Some(1700000000),
        };
        assert_eq!(
            bookmark.reader_url(),
            "https://instapaper.com/read/2045166304"
        );
    }

    #[test]
    fn parses_parsed_article() {
        let json = r#"{
            "metadata": {
                "title": "An Article",
                "author": { "name": "A. Writer", "url": "https://example.com/author" },
                "category": 0
            },
            "content": {
                "body": "<p>Hello <b>world</b></p>",
                "images": [],
                "words": 3,
                "paywalled": false,
                "direction": "ltr"
            }
        }"#;

        let article: ParsedArticle = serde_json::from_str(json).expect("should parse");
        assert_eq!(article.metadata.title.as_deref(), Some("An Article"));
        assert_eq!(article.metadata.author.unwrap().name, "A. Writer");
        assert_eq!(
            article.content.body.as_deref(),
            Some("<p>Hello <b>world</b></p>")
        );
        assert_eq!(article.content.words, Some(3));
        assert!(!article.content.paywalled);
    }

    #[test]
    fn parses_parsed_article_with_nulls() {
        let json = r#"{
            "metadata": { "title": null, "author": null, "category": 0 },
            "content": { "body": null, "images": [], "words": null, "paywalled": true }
        }"#;

        let article: ParsedArticle = serde_json::from_str(json).expect("should parse");
        assert_eq!(article.metadata.title, None);
        assert_eq!(article.content.body, None);
        assert!(article.content.paywalled);
    }

    #[test]
    fn added_date_formats_month_day_year() {
        let bookmark = Bookmark {
            id: 1,
            image: None,
            url: None,
            title: None,
            description: None,
            liked: false,
            archived: false,
            time: Some(1700000000),
        };
        let text = bookmark.added_date().expect("should format");
        let parts: Vec<&str> = text.split(' ').collect();
        assert_eq!(parts.len(), 3, "expected 'Month D, YYYY', got {text}");
        assert!(
            parts[0].starts_with(char::is_uppercase),
            "expected a month name, got {text}"
        );
        let day = parts[1].trim_end_matches(',');
        assert!(
            day.parse::<u32>().is_ok() && !day.starts_with('0'),
            "expected an unpadded day, got {text}"
        );
        assert_eq!(parts[2].len(), 4, "expected a year, got {text}");

        let bookmark = Bookmark {
            time: None,
            ..bookmark
        };
        assert_eq!(bookmark.added_date(), None);
    }

    #[test]
    fn section_query_values_match_api() {
        assert_eq!(Section::Home.query_value(), "home");
        assert_eq!(Section::Liked.query_value(), "liked");
        assert_eq!(Section::Archive.query_value(), "archive");
    }

    #[test]
    fn page_math() {
        let limit = u64::from(BOOKMARKS_LIMIT);
        assert_eq!(page_count(0), 1);
        assert_eq!(page_count(1), 1);
        assert_eq!(page_count(limit), 1);
        assert_eq!(page_count(limit + 1), 2);
        assert_eq!(page_count(10 * limit - 1), 10);
        assert_eq!(page_count(10 * limit), 10);

        assert_eq!(offset_for_page(0), 0);
        assert_eq!(offset_for_page(1), BOOKMARKS_LIMIT);
        assert_eq!(offset_for_page(3), 3 * BOOKMARKS_LIMIT);
    }

    #[test]
    #[ignore = "hits the live Instapaper API with the keyring token"]
    fn live_api_offset_paginates() {
        let Some(token) = crate::secret::get_token() else {
            panic!("no token in the keyring");
        };
        let client = Client::new(token);
        let first = client.bookmarks(Section::Home, 0).expect("first page");
        let second = client
            .bookmarks(Section::Home, BOOKMARKS_LIMIT)
            .expect("second page");
        if first.total <= BOOKMARKS_LIMIT as u64 {
            eprintln!(
                "home has {} bookmarks; add more to exercise pagination",
                first.total
            );
            return;
        }
        assert_eq!(first.total, second.total);
        assert_ne!(
            first.bookmarks[0].id, second.bookmarks[0].id,
            "offset did not move to the next page"
        );
    }
}
