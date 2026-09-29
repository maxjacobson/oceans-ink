use serde::Deserialize;

const BASE_URL: &str = "https://www.instapaper.com/api/2";

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
    pub url: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub liked: bool,
    pub archived: bool,
}

impl Bookmark {
    pub fn display_title(&self) -> String {
        self.title
            .clone()
            .filter(|t| !t.is_empty())
            .or_else(|| self.url.clone())
            .unwrap_or_else(|| "Untitled".to_string())
    }
}

#[derive(Debug, Deserialize)]
struct BookmarkList {
    bookmarks: Vec<Bookmark>,
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

    pub fn bookmarks(&self, section: Section) -> Result<Vec<Bookmark>, Error> {
        let url = format!("{BASE_URL}/bookmarks");
        let response = self
            .http
            .get(url)
            .query(&[("section", section.query_value()), ("limit", "500")])
            .bearer_auth(&self.token)
            .send()?;
        let list: BookmarkList = self.check(response)?.json()?;
        Ok(list.bookmarks)
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

        let first = &list.bookmarks[0];
        assert_eq!(first.id, 123);
        assert_eq!(first.display_title(), "An Article");
        assert!(first.liked);
        assert!(!first.archived);

        let second = &list.bookmarks[1];
        assert_eq!(second.display_title(), "Untitled");
    }

    #[test]
    fn parses_api_error_body() {
        let json = r#"{ "error": { "code": 401, "message": "Authentication failed" } }"#;
        let body: ApiErrorBody = serde_json::from_str(json).expect("should parse");
        assert_eq!(body.error.message, "Authentication failed");
    }

    #[test]
    fn section_query_values_match_api() {
        assert_eq!(Section::Home.query_value(), "home");
        assert_eq!(Section::Liked.query_value(), "liked");
        assert_eq!(Section::Archive.query_value(), "archive");
    }
}
