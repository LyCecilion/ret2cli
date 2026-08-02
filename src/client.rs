use std::path::Path;

use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::{
    Client as HttpClient, Method, Response, Url,
    header::{AUTHORIZATION, HeaderMap, HeaderValue},
    multipart::Form,
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use tokio::io::AsyncWriteExt;

use crate::{
    config::ClientConfig,
    error::{CliError, CliResult},
};

pub struct Client {
    pub base_url: String,
    pub token: Option<String>,
    persist_token: bool,
    http: HttpClient,
}

impl Client {
    /// Create a new API client.
    ///
    /// # Errors
    ///
    /// Returns `CliError::Config` if `base_url` is empty or consists only of slashes.
    pub fn new(base_url: &str, token: Option<String>) -> CliResult<Self> {
        let base_url = base_url.trim_end_matches('/').to_owned();
        if base_url.is_empty() {
            return Err(CliError::Config("base URL cannot be empty".to_owned()));
        }
        Ok(Self { base_url, token, persist_token: true, http: HttpClient::new() })
    }

    pub fn set_token_persistence(&mut self, persist: bool) {
        self.persist_token = persist;
    }

    #[must_use]
    pub fn persists_token(&self) -> bool {
        self.persist_token
    }

    /// Build a full API URL: `{base_url}/api/{path}`
    fn url(&self, path: &str, query: &[(&str, &str)]) -> CliResult<Url> {
        let mut url =
            Url::parse(&format!("{}/api/{}", self.base_url, path.trim_start_matches('/')))
                .map_err(|e| CliError::Config(format!("invalid URL: {e}")))?;
        if !query.is_empty() {
            url.query_pairs_mut().extend_pairs(query.iter().copied());
        }
        Ok(url)
    }

    fn auth_headers(&self) -> CliResult<HeaderMap> {
        let mut headers = HeaderMap::new();
        if let Some(token) = &self.token {
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {token}"))
                    .map_err(|_| CliError::Config("invalid bearer token".to_owned()))?,
            );
        }
        Ok(headers)
    }

    fn request(
        &self,
        method: Method,
        path: &str,
        query: &[(&str, &str)],
    ) -> CliResult<reqwest::RequestBuilder> {
        Ok(self.http.request(method, self.url(path, query)?).headers(self.auth_headers()?))
    }

    async fn check_response(&self, response: Response) -> CliResult<(Response, Option<String>)> {
        // Extract Set-Token header before consuming the response
        let new_token =
            response.headers().get("Set-Token").and_then(|v| v.to_str().ok()).map(str::to_owned);

        if response.status().is_success() {
            return Ok((response, new_token));
        }

        let status = response.status();
        // Try to parse JSON error body
        let text = response.text().await.unwrap_or_else(|_| status.to_string());
        let message = serde_json::from_str::<Value>(&text)
            .ok()
            .and_then(|v| v.get("error").cloned())
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_else(|| if text.is_empty() { status.to_string() } else { text });

        Err(CliError::Api { status, message })
    }

    async fn json_response(&self, response: Response) -> CliResult<(Value, Option<String>)> {
        let (response, new_token) = self.check_response(response).await?;
        let text = response.text().await?;
        let value = if text.is_empty() {
            Value::Null
        } else {
            serde_json::from_str(&text).unwrap_or(Value::String(text))
        };
        Ok((value, new_token))
    }

    async fn typed_response<T: DeserializeOwned>(
        &self,
        response: Response,
    ) -> CliResult<(T, Option<String>)> {
        let (response, new_token) = self.check_response(response).await?;
        let text = response.text().await?;
        if text.is_empty() {
            return Err(CliError::Config("empty response body".to_owned()));
        }
        let value = serde_json::from_str(&text)?;
        Ok((value, new_token))
    }

    /// Handle token refresh: if Set-Token returned and differs from current, update config.
    fn handle_token(
        &mut self,
        new_token: Option<&str>,
        config: &mut ClientConfig,
        profile_name: Option<&str>,
    ) -> CliResult<()> {
        if let Some(new) = new_token
            && self.token.as_deref() != Some(new)
        {
            self.token = Some(new.to_owned());
            if self.persist_token {
                let profile = config.active_profile_mut(profile_name)?;
                if let Some(account) = profile.active_account.clone()
                    && let Some(session) = profile.accounts.get_mut(&account)
                {
                    session.token = String::from(new);
                    config.save()?;
                }
            }
        }
        Ok(())
    }

    // --- Public HTTP methods ---

    /// Send a GET request and deserialize the JSON response.
    ///
    /// # Errors
    ///
    /// Returns `CliError` on HTTP, serialization, or config errors.
    pub async fn get<T: DeserializeOwned>(
        &mut self,
        path: &str,
        query: &[(&str, &str)],
        config: &mut ClientConfig,
        profile_name: Option<&str>,
    ) -> CliResult<T> {
        let response = self.request(Method::GET, path, query)?.send().await?;
        let (value, new_token) = self.typed_response::<T>(response).await?;
        self.handle_token(new_token.as_deref(), config, profile_name)?;
        Ok(value)
    }

    /// Send a GET request and return the raw JSON value.
    ///
    /// # Errors
    ///
    /// Returns `CliError` on HTTP or config errors.
    pub async fn get_value(
        &mut self,
        path: &str,
        query: &[(&str, &str)],
        config: &mut ClientConfig,
        profile_name: Option<&str>,
    ) -> CliResult<Value> {
        let response = self.request(Method::GET, path, query)?.send().await?;
        let (value, new_token) = self.json_response(response).await?;
        self.handle_token(new_token.as_deref(), config, profile_name)?;
        Ok(value)
    }

    /// Send a POST request with a JSON body and deserialize the response.
    ///
    /// # Errors
    ///
    /// Returns `CliError` on HTTP, serialization, or config errors.
    pub async fn post<T: DeserializeOwned, B: Serialize + ?Sized>(
        &mut self,
        path: &str,
        body: &B,
        config: &mut ClientConfig,
        profile_name: Option<&str>,
    ) -> CliResult<T> {
        let response = self.request(Method::POST, path, &[])?.json(body).send().await?;
        let (value, new_token) = self.typed_response::<T>(response).await?;
        self.handle_token(new_token.as_deref(), config, profile_name)?;
        Ok(value)
    }

    /// Send a POST request with a JSON body and return the raw JSON value.
    ///
    /// # Errors
    ///
    /// Returns `CliError` on HTTP or config errors.
    pub async fn post_value<B: Serialize + ?Sized>(
        &mut self,
        path: &str,
        body: &B,
        config: &mut ClientConfig,
        profile_name: Option<&str>,
    ) -> CliResult<Value> {
        let response = self.request(Method::POST, path, &[])?.json(body).send().await?;
        let (value, new_token) = self.json_response(response).await?;
        self.handle_token(new_token.as_deref(), config, profile_name)?;
        Ok(value)
    }

    /// POST JSON body, expect empty/no response body. Returns the new token if Set-Token header present.
    ///
    /// # Errors
    ///
    /// Returns `CliError` on HTTP or config errors.
    pub async fn post_no_body<B: Serialize + ?Sized>(
        &mut self,
        path: &str,
        body: &B,
        config: &mut ClientConfig,
        profile_name: Option<&str>,
    ) -> CliResult<Option<String>> {
        let response = self.request(Method::POST, path, &[])?.json(body).send().await?;
        let (_, new_token) = self.check_response(response).await?;
        self.handle_token(new_token.as_deref(), config, profile_name)?;
        Ok(new_token)
    }

    /// Upload multipart form data and deserialize the JSON response.
    ///
    /// # Errors
    ///
    /// Returns `CliError` on HTTP, serialization, or config errors.
    pub async fn post_multipart<T: DeserializeOwned>(
        &mut self,
        path: &str,
        form: Form,
        config: &mut ClientConfig,
        profile_name: Option<&str>,
    ) -> CliResult<T> {
        let response = self.request(Method::POST, path, &[])?.multipart(form).send().await?;
        let (value, new_token) = self.typed_response::<T>(response).await?;
        self.handle_token(new_token.as_deref(), config, profile_name)?;
        Ok(value)
    }

    /// Send a PATCH request with a JSON body and deserialize the response.
    ///
    /// # Errors
    ///
    /// Returns `CliError` on HTTP, serialization, or config errors.
    pub async fn patch<T: DeserializeOwned, B: Serialize + ?Sized>(
        &mut self,
        path: &str,
        body: &B,
        config: &mut ClientConfig,
        profile_name: Option<&str>,
    ) -> CliResult<T> {
        let response = self.request(Method::PATCH, path, &[])?.json(body).send().await?;
        let (value, new_token) = self.typed_response::<T>(response).await?;
        self.handle_token(new_token.as_deref(), config, profile_name)?;
        Ok(value)
    }

    /// Send a PATCH request with a JSON body and expect no response body.
    ///
    /// # Errors
    ///
    /// Returns `CliError` on HTTP or config errors.
    pub async fn patch_no_body<B: Serialize + ?Sized>(
        &mut self,
        path: &str,
        body: &B,
        config: &mut ClientConfig,
        profile_name: Option<&str>,
    ) -> CliResult<()> {
        let response = self.request(Method::PATCH, path, &[])?.json(body).send().await?;
        let (_, new_token) = self.check_response(response).await?;
        self.handle_token(new_token.as_deref(), config, profile_name)
    }

    /// Send a DELETE request.
    ///
    /// # Errors
    ///
    /// Returns `CliError` on HTTP or config errors.
    pub async fn delete(
        &mut self,
        path: &str,
        config: &mut ClientConfig,
        profile_name: Option<&str>,
    ) -> CliResult<()> {
        let response = self.request(Method::DELETE, path, &[])?.send().await?;
        let (_, new_token) = self.check_response(response).await?;
        self.handle_token(new_token.as_deref(), config, profile_name)?;
        Ok(())
    }

    /// Send a DELETE request and return the raw JSON value.
    ///
    /// # Errors
    ///
    /// Returns `CliError` on HTTP or config errors.
    pub async fn delete_value(
        &mut self,
        path: &str,
        config: &mut ClientConfig,
        profile_name: Option<&str>,
    ) -> CliResult<Value> {
        let response = self.request(Method::DELETE, path, &[])?.send().await?;
        let (value, new_token) = self.json_response(response).await?;
        self.handle_token(new_token.as_deref(), config, profile_name)?;
        Ok(value)
    }

    /// Fetch a raw response body with its content type, e.g. for inline images.
    ///
    /// # Errors
    ///
    /// Returns `CliError` on HTTP, serialization, or config errors.
    pub async fn download_bytes(
        &mut self,
        path: &str,
        query: &[(&str, &str)],
        config: &mut ClientConfig,
        profile_name: Option<&str>,
    ) -> CliResult<(Vec<u8>, String)> {
        let response = self.request(Method::GET, path, query)?.send().await?;
        let (response, new_token) = self.check_response(response).await?;
        self.handle_token(new_token.as_deref(), config, profile_name)?;
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("application/octet-stream")
            .to_owned();
        let bytes = response.bytes().await?;
        Ok((bytes.to_vec(), content_type))
    }

    /// Download a file from a GET endpoint with streaming and optional progress bar.
    ///
    /// # Errors
    ///
    /// Returns `CliError` on HTTP, I/O, or config errors.
    pub async fn download_query(
        &mut self,
        path: &str,
        query: &[(&str, &str)],
        output: &Path,
        config: &mut ClientConfig,
        profile_name: Option<&str>,
        show_progress: bool,
    ) -> CliResult<()> {
        let response = self.request(Method::GET, path, query)?.send().await?;
        let (response, new_token) = self.check_response(response).await?;
        self.handle_token(new_token.as_deref(), config, profile_name)?;

        let pb = if show_progress {
            let template = ProgressStyle::default_bar()
                .template("{spinner:.green} [{bar:40.cyan/blue}] {bytes}/{total_bytes}")
                .map_err(|_| CliError::Config("invalid progress template".to_owned()))?;
            response.content_length().map(|size| {
                let pb = ProgressBar::new(size);
                pb.set_style(template.progress_chars("#>-"));
                pb
            })
        } else {
            None
        };
        let mut file = tokio::fs::File::create(output).await?;
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            if let Some(pb) = &pb {
                pb.inc(chunk.len() as u64);
            }
            file.write_all(&chunk).await?;
        }
        file.flush().await?;
        if let Some(pb) = pb {
            pb.finish_and_clear();
        }
        Ok(())
    }
}
