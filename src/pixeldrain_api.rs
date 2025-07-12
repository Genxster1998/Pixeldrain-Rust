// pixeldrain_api.rs - Robust PixelDrain API implementation
// Based on actual API responses and patterns from go-pd and pixeldrain_api_client
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine;
use chrono::{DateTime, Utc};
use reqwest::{blocking::multipart, blocking::Client, header, StatusCode};
use serde::{Deserialize, Serialize};
use url::Url;

pub const BASE_URL: &str = "https://pixeldrain.com";
pub const API_URL: &str = "https://pixeldrain.com/api";
pub const DEFAULT_USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/79.0.3945.117 Safari/537.36";

// ============================================================================
// Configuration and Client
// ============================================================================

#[derive(Debug, Clone)]
pub struct PixelDrainConfig {
    pub api_key: Option<String>,
    pub timeout: Option<Duration>,
    pub user_agent: Option<String>,
    pub real_ip: Option<String>,
    pub debug: bool,
}

impl Default for PixelDrainConfig {
    fn default() -> Self {
        Self {
            api_key: None,
            timeout: Some(Duration::from_secs(300)), // 5 minutes
            user_agent: None,
            real_ip: None,
            debug: false,
        }
    }
}

impl PixelDrainConfig {
    pub fn with_api_key(mut self, api_key: String) -> Self {
        self.api_key = Some(api_key);
        self
    }
}

pub struct PixelDrainClient {
    config: PixelDrainConfig,
    client: Client,
}

impl PixelDrainClient {
    pub fn new(config: PixelDrainConfig) -> Result<Self, PixelDrainError> {
        let mut client_builder = Client::builder()
            .user_agent(config.user_agent.as_deref().unwrap_or(DEFAULT_USER_AGENT))
            // Optimize for upload performance
            .pool_max_idle_per_host(10)
            .pool_idle_timeout(Some(Duration::from_secs(30)))
            .tcp_keepalive(Some(Duration::from_secs(60)));
        
        if let Some(timeout) = config.timeout {
            client_builder = client_builder.timeout(timeout);
        }

        let client = client_builder.build()?;
        
        Ok(Self { config, client })
    }

    // ============================================================================
    // Core Request Methods
    // ============================================================================

    fn build_request(&self, method: reqwest::Method, endpoint: &str) -> reqwest::blocking::RequestBuilder {
        let url = format!("{}/{}", API_URL, endpoint.trim_start_matches('/'));
        let mut req = self.client.request(method, &url);
        
        // Add Basic Auth header if API key is available
        if let Some(api_key) = &self.config.api_key {
            let auth_header = format!("Basic {}", base64::engine::general_purpose::STANDARD.encode(format!(":{}", api_key)));
            req = req.header(header::AUTHORIZATION, auth_header);
        }

        // Add custom headers
        if let Some(real_ip) = &self.config.real_ip {
            req = req.header("X-Real-IP", real_ip);
        }
        
        req
    }

    fn do_request<T>(&self, method: reqwest::Method, endpoint: &str, body: Option<reqwest::blocking::Body>) -> Result<T, PixelDrainError>
    where
        T: for<'de> Deserialize<'de>,
    {
        let method_str = method.as_str();
        let mut req = self.build_request(method.clone(), endpoint);
        
        if let Some(body) = body {
            req = req.body(body);
        }

        let resp = req.send()?;
        let status = resp.status();
        
        if self.config.debug {
            println!("Request: {} {}", method_str, endpoint);
            println!("Response Status: {}", status);
        }

        if !status.is_success() {
            let error_text = resp.text().unwrap_or_default();
            return Err(PixelDrainError::Api(ApiError {
                status,
                value: "error".to_string(),
                message: error_text,
            }));
        }

        let result: T = resp.json()?;
        Ok(result)
    }

    fn do_multipart<T>(&self, endpoint: &str, form: multipart::Form) -> Result<T, PixelDrainError>
    where
        T: for<'de> Deserialize<'de>,
    {
        let req = self.build_request(reqwest::Method::POST, endpoint);
        let resp = req.multipart(form).send()?;
        let status = resp.status();
        
        if self.config.debug {
            println!("Multipart Request: POST {}", endpoint);
            println!("Response Status: {}", status);
        }

        if !status.is_success() {
            let error_text = resp.text().unwrap_or_default();
            return Err(PixelDrainError::Api(ApiError {
                status,
                value: "error".to_string(),
                message: error_text,
            }));
        }

        let result: T = resp.json()?;
        Ok(result)
    }

    // ============================================================================
    // File Operations
    // ============================================================================

    /// Upload a file using POST /api/file
    pub fn upload_file<P: AsRef<Path>>(
        &self,
        file_path: P,
        progress: Option<ProgressCallback>,
    ) -> Result<UploadResponse, PixelDrainError> {
        let file_path = file_path.as_ref();
        
        if !file_path.exists() {
            return Err(PixelDrainError::FileNotFound(file_path.display().to_string()));
        }

        let file_name = file_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "file".to_string());

        let file_size = file_path.metadata()?.len();

        // Custom reader to report progress with optimized buffering
        struct ProgressReader<R: Read> {
            inner: R,
            total: u64,
            read: u64,
            cb: Option<ProgressCallback>,
            buffer: Vec<u8>,
            buffer_pos: usize,
            buffer_len: usize,
        }
        
        impl<R: Read> ProgressReader<R> {
            fn new(inner: R, total: u64, cb: Option<ProgressCallback>) -> Self {
                Self {
                    inner,
                    total,
                    read: 0,
                    cb,
                    buffer: vec![0; 64 * 1024], // 64KB buffer
                    buffer_pos: 0,
                    buffer_len: 0,
                }
            }
        }
        
        impl<R: Read> Read for ProgressReader<R> {
            fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
                // If buffer is empty, fill it
                if self.buffer_pos >= self.buffer_len {
                    self.buffer_len = self.inner.read(&mut self.buffer)?;
                    self.buffer_pos = 0;
                    if self.buffer_len == 0 {
                        return Ok(0);
                    }
                }
                
                // Copy from buffer to output
                let to_copy = std::cmp::min(buf.len(), self.buffer_len - self.buffer_pos);
                buf[..to_copy].copy_from_slice(&self.buffer[self.buffer_pos..self.buffer_pos + to_copy]);
                self.buffer_pos += to_copy;
                self.read += to_copy as u64;
                
                // Report progress
                if let Some(cb) = &self.cb {
                    let mut cb = cb.lock().unwrap();
                    let progress = if self.total > 0 {
                        self.read as f32 / self.total as f32
                    } else {
                        0.0
                    };
                    cb(progress.min(1.0));
                }
                
                Ok(to_copy)
            }
        }

        let reader = ProgressReader::new(
            File::open(file_path)?,
            file_size,
            progress.clone(),
        );

        let part = multipart::Part::reader(reader)
            .file_name(file_name)
            .mime_str("application/octet-stream")
            .unwrap();

        let form = multipart::Form::new().part("file", part);

        let result = self.do_multipart("file", form)?;
        
        // Reset progress to 100% when complete
        if let Some(progress) = progress {
            let mut progress = progress.lock().unwrap();
            progress(1.0);
        }
        
        Ok(result)
    }

    /// Download a file using GET /api/file/{id}
    pub fn download_file(
        &self,
        file_id: &str,
        save_path: &Path,
        progress: Option<ProgressCallback>,
    ) -> Result<(), PixelDrainError> {
        let url = format!("{}/file/{}", API_URL, file_id);
        let mut resp = self.client.get(&url).send()?;
        
        let status = resp.status();
        if !status.is_success() {
            let error_text = resp.text().unwrap_or_default();
            return Err(PixelDrainError::Api(ApiError {
                status,
                value: "error".to_string(),
                message: error_text,
            }));
        }

        let content_length = resp.content_length().unwrap_or(0);
        let mut file = File::create(save_path)?;
        let mut downloaded = 0u64;
        let mut buffer = [0; 8192];

        loop {
            let n = resp.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            
            file.write_all(&buffer[..n])?;
            downloaded += n as u64;
            
            if let Some(progress) = &progress {
                let mut progress = progress.lock().unwrap();
                let progress_value = if content_length > 0 {
                    downloaded as f32 / content_length as f32
                } else {
                    0.0
                };
                progress(progress_value.min(1.0));
            }
        }

        // Reset progress to 100% when complete
        if let Some(progress) = progress {
            let mut progress = progress.lock().unwrap();
            progress(1.0);
        }

        Ok(())
    }

    /// Get file information using GET /api/file/{id}
    pub fn get_file_info(&self, file_id: &str) -> Result<FileInfo, PixelDrainError> {
        self.do_request(reqwest::Method::GET, &format!("file/{}", file_id), None)
    }

    /// Get user files using GET /api/user/files
    pub fn get_user_files(&self) -> Result<UserFilesResponse, PixelDrainError> {
        self.do_request(reqwest::Method::GET, "user/files", None)
    }

    /// Delete a file using DELETE /api/file/{id}
    pub fn delete_file(&self, file_id: &str) -> Result<(), PixelDrainError> {
        if self.config.api_key.is_none() {
            return Err(PixelDrainError::MissingApiKey);
        }

        let _: serde_json::Value = self.do_request(reqwest::Method::DELETE, &format!("file/{}", file_id), None)?;
        Ok(())
    }

    /// Extract file ID from PixelDrain URL
    pub fn extract_file_id(url: &str) -> Result<String, PixelDrainError> {
        let url = Url::parse(url)?;
        let path = url.path();
        
        // Handle different URL formats:
        // - https://pixeldrain.com/u/{id}
        // - https://pixeldrain.com/u/{id}?download
        // - https://pixeldrain.com/api/file/{id}
        
        if path.starts_with("/u/") {
            let id = path.strip_prefix("/u/").unwrap_or("");
            if !id.is_empty() {
                return Ok(id.to_string());
            }
        } else if path.starts_with("/api/file/") {
            let id = path.strip_prefix("/api/file/").unwrap_or("");
            if !id.is_empty() {
                return Ok(id.to_string());
            }
        }
        
        Err(PixelDrainError::InvalidUrl("Could not extract file ID from URL".to_string()))
    }
}

// ============================================================================
// Response Types
// ============================================================================

#[derive(Debug, Clone)]
pub struct ApiError {
    pub status: StatusCode,
    pub value: String,
    pub message: String,
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "API Error {}: {} - {}", self.status, self.value, self.message)
    }
}

#[derive(Debug, Deserialize)]
pub struct UploadResponse {
    pub id: String,
}

impl UploadResponse {
    pub fn get_file_url(&self) -> String {
        format!("{}/u/{}", BASE_URL, self.id)
    }
}



#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FileInfo {
    pub id: String,
    pub name: String,
    pub size: u64,
    pub views: u64,
    pub bandwidth_used: u64,
    pub bandwidth_used_paid: u64,
    pub downloads: u64,
    pub date_upload: DateTime<Utc>,
    pub date_last_view: DateTime<Utc>,
    pub mime_type: String,
    pub thumbnail_href: String,
    pub hash_sha256: String,
    pub delete_after_date: DateTime<Utc>,
    pub delete_after_downloads: u64,
    pub availability: String,
    pub availability_message: String,
    pub abuse_type: String,
    pub abuse_reporter_name: String,
    pub can_edit: bool,
    pub can_download: bool,
    pub show_ads: bool,
    pub allow_video_player: bool,
    pub download_speed_limit: u64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct UserInfo {
    pub username: String,
    pub email: String,
    pub email_verified: bool,
    pub otp_enabled: bool,
    pub subscription: SubscriptionType,
    pub storage_space_used: u64,
    pub filesystem_storage_used: u64,
    pub is_admin: bool,
    pub balance_micro_eur: i64,
    pub hotlinking_enabled: bool,
    pub monthly_transfer_cap: u64,
    pub monthly_transfer_used: u64,
    pub file_viewer_branding: HashMap<String, String>,
    pub file_embed_domains: String,
    pub skip_file_viewer: bool,
    pub affiliate_user_name: String,
    pub checkout_country: String,
    pub checkout_name: String,
    pub checkout_provider: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SubscriptionType {
    pub id: String,
    pub name: String,
    pub r#type: String,
    pub file_size_limit: u64,
    pub file_expiry_days: u64,
    pub storage_space: u64,
    pub price_per_tb_storage: u64,
    pub price_per_tb_bandwidth: u64,
    pub monthly_transfer_cap: u64,
    pub file_viewer_branding: bool,
}

#[derive(Debug, Deserialize)]
pub struct UserFilesResponse {
    pub files: Vec<FileInfo>,
}

// ============================================================================
// Error Types
// ============================================================================

#[derive(Debug)]
pub enum PixelDrainError {
    Io(io::Error),
    Reqwest(reqwest::Error),
    Api(ApiError),
    Serde(serde_json::Error),
    InvalidUrl(String),
    FileNotFound(String),
    MissingApiKey,
}

impl std::fmt::Display for PixelDrainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PixelDrainError::Io(e) => write!(f, "IO error: {}", e),
            PixelDrainError::Reqwest(e) => write!(f, "Request error: {}", e),
            PixelDrainError::Api(e) => write!(f, "API error: {}", e),
            PixelDrainError::Serde(e) => write!(f, "Serialization error: {}", e),
            PixelDrainError::InvalidUrl(msg) => write!(f, "Invalid URL: {}", msg),
            PixelDrainError::FileNotFound(path) => write!(f, "File not found: {}", path),
            PixelDrainError::MissingApiKey => write!(f, "Missing API key"),
        }
    }
}

impl From<io::Error> for PixelDrainError {
    fn from(e: io::Error) -> Self {
        PixelDrainError::Io(e)
    }
}

impl From<reqwest::Error> for PixelDrainError {
    fn from(e: reqwest::Error) -> Self {
        PixelDrainError::Reqwest(e)
    }
}

impl From<serde_json::Error> for PixelDrainError {
    fn from(e: serde_json::Error) -> Self {
        PixelDrainError::Serde(e)
    }
}

impl From<url::ParseError> for PixelDrainError {
    fn from(e: url::ParseError) -> Self {
        PixelDrainError::InvalidUrl(e.to_string())
    }
}

// ============================================================================
// Progress Callback Type
// ============================================================================

pub type ProgressCallback = Arc<Mutex<dyn FnMut(f32) + Send>>;
