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
// Error Response Structure
// ============================================================================

#[derive(Debug, Deserialize)]
struct ApiErrorResponse {
    pub _success: Option<bool>,
    pub value: Option<String>,
    pub message: Option<String>,
    pub _errors: Option<Vec<ApiErrorResponse>>,
}

// ============================================================================
// Configuration and Client
// ============================================================================

#[derive(Debug, Clone)]
pub struct PixelDrainConfig {
    pub api_key: Option<String>,
    pub timeout: Option<Duration>,
    pub user_agent: Option<String>,
    pub real_ip: Option<String>,
    pub real_agent: Option<String>,
    pub debug: bool,
}

impl Default for PixelDrainConfig {
    fn default() -> Self {
        Self {
            api_key: None,
            timeout: Some(Duration::from_secs(3600)), // 1 hour like go-pd
            user_agent: None,
            real_ip: None,
            real_agent: None,
            debug: true, // Enable debug for troubleshooting
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
        // Always require API key for uploads
        if let Some(api_key) = &self.config.api_key {
            let auth_header = format!("Basic {}", base64::engine::general_purpose::STANDARD.encode(format!(":{}", api_key)));
            req = req.header(header::AUTHORIZATION, auth_header);
        }
        if let Some(real_ip) = &self.config.real_ip {
            req = req.header("X-Real-IP", real_ip);
        }
        if let Some(real_agent) = &self.config.real_agent {
            req = req.header("User-Agent", real_agent);
        }
        req
    }

    // Enhanced error handling based on pixeldrain_api_client patterns
    fn parse_json_response<T>(resp: reqwest::blocking::Response) -> Result<T, PixelDrainError>
    where
        T: for<'de> Deserialize<'de>,
    {
        let status = resp.status();
        
        // Get the response body as text first for debugging
        let response_text = resp.text().unwrap_or_default();
        
        // Debug print for user endpoint
        // if response_text.contains("username") || response_text.contains("email") {
        //     eprintln!("=== USER API RESPONSE DEBUG ===");
        //     eprintln!("Status: {}", status);
        //     eprintln!("Response body: {}", response_text);
        //     eprintln!("=== END USER API RESPONSE DEBUG ===");
        // }
        
        // Test for client side and server side errors
        if status.as_u16() >= 400 {
            // Try to parse as structured error first
            if let Ok(api_error) = serde_json::from_str::<ApiErrorResponse>(&response_text) {
                return Err(PixelDrainError::Api(ApiError {
                    status,
                    value: api_error.value.unwrap_or_else(|| "error".to_string()),
                    message: api_error.message.unwrap_or_else(|| "Unknown error".to_string()),
                }));
            }
            
            // Fall back to plain text error
            return Err(PixelDrainError::Api(ApiError {
                status,
                value: "error".to_string(),
                message: response_text,
            }));
        }

        // Parse successful response
        let result: T = serde_json::from_str(&response_text)?;
        Ok(result)
    }

    fn do_request<T>(&self, method: reqwest::Method, endpoint: &str, body: Option<reqwest::blocking::Body>) -> Result<T, PixelDrainError>
    where
        T: for<'de> Deserialize<'de>,
    {
        let method_str = method.as_str();
        // Most API requests require authentication, so default to false for anonymous
        let mut req = self.build_request(method.clone(), endpoint);
        
        if let Some(body) = body {
            req = req.body(body);
        }

        let resp = req.send()?;
        
        if self.config.debug {
            println!("Request: {} {}", method_str, endpoint);
            println!("Response Status: {}", resp.status());
        }

        Self::parse_json_response(resp)
    }

    // Form-based request method for certain endpoints (like pixeldrain_api_client)
    fn do_form_request<T>(&self, method: reqwest::Method, endpoint: &str, form_data: &[(&str, &str)]) -> Result<T, PixelDrainError>
    where
        T: for<'de> Deserialize<'de>,
    {
        let method_str = method.as_str();
        // Form requests typically require authentication, so default to false for anonymous
        let mut req = self.build_request(method.clone(), endpoint);
        
        // Build form data using reqwest's built-in form support
        let mut form_params = Vec::new();
        for (key, value) in form_data.iter() {
            form_params.push((*key, *value));
        }
        
        req = req.form(&form_params);

        let resp = req.send()?;
        
        if self.config.debug {
            println!("Form Request: {} {}", method_str, endpoint);
            println!("Response Status: {}", resp.status());
        }

        Self::parse_json_response(resp)
    }

    fn do_multipart<T>(&self, endpoint: &str, form: multipart::Form) -> Result<T, PixelDrainError>
    where
        T: for<'de> Deserialize<'de>,
    {
        let url = format!("{}/{}", API_URL, endpoint.trim_start_matches('/'));
        let mut req = self.client.request(reqwest::Method::POST, &url);
        // Always require API key for uploads
        if let Some(api_key) = &self.config.api_key {
            let auth_header = format!("Basic {}", base64::engine::general_purpose::STANDARD.encode(format!(":{}", api_key)));
            req = req.header(header::AUTHORIZATION, auth_header);
        }
        if let Some(real_ip) = &self.config.real_ip {
            req = req.header("X-Real-IP", real_ip);
        }
        if let Some(real_agent) = &self.config.real_agent {
            req = req.header("User-Agent", real_agent);
        }
        let resp = req.multipart(form).send()?;
        let status = resp.status();
        if self.config.debug {
            println!("Multipart Request: POST {}", endpoint);
            println!("Response Status: {}", status);
            println!("API Key present: {}", self.config.api_key.is_some());
            if let Some(api_key) = &self.config.api_key {
                println!("API Key (first 8 chars): {}...", &api_key[..8.min(api_key.len())]);
            }
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

        if self.config.api_key.is_none() {
            return Err(PixelDrainError::MissingApiKey);
        }

        let file_name = file_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "file".to_string());

        let file_size = file_path.metadata()?.len();

        // Retry logic with progress reset
        const MAX_RETRIES: usize = 3;
        const RETRY_DELAY: Duration = Duration::from_secs(3);
        
        for attempt in 1..=MAX_RETRIES {
            if self.config.debug {
                println!("Upload attempt {}/{}", attempt, MAX_RETRIES);
            }
            
            // Reset progress at the start of each attempt
            if let Some(progress) = &progress {
                if let Ok(mut progress) = progress.lock() {
                    progress(0.0);
                }
            }

            // Create a progress reader that works for file uploads
            let progress_reader = ProgressReader::new_file(
                File::open(file_path)?,
                file_size,
                progress.clone(),
            );

            let part = multipart::Part::reader(progress_reader)
                .file_name(file_name.clone())
                .mime_str("application/octet-stream")
                .unwrap();

            let form = multipart::Form::new().part("file", part);

            match self.do_multipart("file", form) {
                Ok(result) => {
                    // Reset progress to 100% when complete
                    if let Some(progress) = &progress {
                        if let Ok(mut progress) = progress.lock() {
                            progress(1.0);
                        }
                    }
                    return Ok(result);
                }
                Err(e) => {
                    // Check if this is a retryable error
                    let should_retry = match &e {
                        PixelDrainError::Reqwest(reqwest_err) => {
                            reqwest_err.is_timeout() || 
                            reqwest_err.is_connect() || 
                            reqwest_err.is_request() ||
                            reqwest_err.to_string().contains("request or response body error")
                        }
                        PixelDrainError::Api(api_err) => {
                            api_err.status.is_server_error()
                        }
                        _ => false,
                    };
                    
                    if should_retry && attempt < MAX_RETRIES {
                        if self.config.debug {
                            println!("Upload failed, retrying in {} seconds...", RETRY_DELAY.as_secs());
                        }
                        std::thread::sleep(RETRY_DELAY);
                        continue;
                    } else {
                        return Err(e);
                    }
                }
            }
        }
        
        // This should never be reached, but just in case
        Err(PixelDrainError::Api(ApiError {
            status: reqwest::StatusCode::INTERNAL_SERVER_ERROR,
            value: "error".to_string(),
            message: "Upload failed after all retry attempts".to_string(),
        }))
    }

    /// Download a file using GET /api/file/{id}
    pub fn download_file(
        &self,
        file_id: &str,
        save_path: &Path,
        progress: Option<ProgressCallback>,
    ) -> Result<(), PixelDrainError> {
        let url = format!("{}/file/{}", API_URL, file_id);
        
        // Retry logic similar to go-pd
        const MAX_RETRIES: usize = 5;
        const RETRY_DELAY: Duration = Duration::from_secs(3);
        
        let mut last_error = None;
        
        for attempt in 1..=MAX_RETRIES {
            if self.config.debug {
                println!("Download attempt {}/{}", attempt, MAX_RETRIES);
            }
            
            // Reset progress at the start of each attempt
            if let Some(progress) = &progress {
                let mut progress = progress.lock().unwrap();
                progress(0.0);
            }
            
            // Build request: only add Authorization if API key is set
            let mut req = self.client.get(&url);
            if let Some(api_key) = &self.config.api_key {
                let auth_header = format!("Basic {}", base64::engine::general_purpose::STANDARD.encode(format!(":{}", api_key)));
                req = req.header(header::AUTHORIZATION, auth_header);
            }
            
            let mut resp = match req.send() {
                Ok(resp) => resp,
                Err(e) => {
                    last_error = Some(PixelDrainError::Reqwest(e));
                    if attempt < MAX_RETRIES {
                        if self.config.debug {
                            println!("Download failed, retrying in {} seconds...", RETRY_DELAY.as_secs());
                        }
                        std::thread::sleep(RETRY_DELAY);
                        continue;
                    } else {
                        break;
                    }
                }
            };
            
            let status = resp.status();
            if !status.is_success() {
                let error_text = resp.text().unwrap_or_default();
                let api_error = PixelDrainError::Api(ApiError {
                    status,
                    value: "error".to_string(),
                    message: error_text,
                });
                
                // Retry on server errors
                if status.is_server_error() && attempt < MAX_RETRIES {
                    last_error = Some(api_error);
                    if self.config.debug {
                        println!("Download failed with server error, retrying in {} seconds...", RETRY_DELAY.as_secs());
                    }
                    std::thread::sleep(RETRY_DELAY);
                    continue;
                } else {
                    return Err(api_error);
                }
            }

            let content_length = resp.content_length().unwrap_or(0);
            let mut file = File::create(save_path)?;
            let mut downloaded = 0u64;
            let mut buffer = [0; 8192];

            loop {
                let n = match resp.read(&mut buffer) {
                    Ok(n) => n,
                    Err(e) => {
                        // Retry on read errors
                        if attempt < MAX_RETRIES {
                            if self.config.debug {
                                println!("Download read failed, retrying in {} seconds...", RETRY_DELAY.as_secs());
                            }
                            std::thread::sleep(RETRY_DELAY);
                            break;
                        } else {
                            return Err(PixelDrainError::Io(e));
                        }
                    }
                };
                
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
            
            // If we get here, download was successful
            // Reset progress to 100% when complete
            if let Some(progress) = &progress {
                let mut progress = progress.lock().unwrap();
                progress(1.0);
            }
            
            return Ok(());
        }
        
        // If we get here, all retries failed
        Err(last_error.unwrap_or_else(|| PixelDrainError::Api(ApiError {
            status: reqwest::StatusCode::INTERNAL_SERVER_ERROR,
            value: "error".to_string(),
            message: "Download failed after all retry attempts".to_string(),
        })))
    }

    /// Download a file thumbnail using GET /api/file/{id}/thumbnail?width=x&height=x
    #[allow(dead_code)]
    pub fn download_thumbnail(
        &self,
        file_id: &str,
        width: u32,
        height: u32,
        save_path: &Path,
    ) -> Result<(), PixelDrainError> {
        let url = format!(
            "{}/file/{}/thumbnail?width={}&height={}",
            API_URL, file_id, width, height
        );
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
        let mut file = File::create(save_path)?;
        io::copy(&mut resp, &mut file)?;
        Ok(())
    }

    /// Get file information using GET /api/file/{id}
    pub fn get_file_info(&self, file_id: &str) -> Result<FileInfo, PixelDrainError> {
        self.do_request(reqwest::Method::GET, &format!("file/{}/info", file_id), None)
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

    /// Upload a file using PUT /api/file/{name} (with custom filename)
    #[allow(dead_code)]
    pub fn upload_file_put<P: AsRef<Path>>(
        &self,
        file_path: P,
        custom_filename: &str,
        progress: Option<ProgressCallback>,
    ) -> Result<UploadResponse, PixelDrainError> {
        let file_path = file_path.as_ref();
        
        if !file_path.exists() {
            return Err(PixelDrainError::FileNotFound(file_path.display().to_string()));
        }

        if self.config.api_key.is_none() {
            return Err(PixelDrainError::MissingApiKey);
        }

        let file_size = file_path.metadata()?.len();

        // Retry logic with progress reset
        const MAX_RETRIES: usize = 3;
        const RETRY_DELAY: Duration = Duration::from_secs(3);
        
        for attempt in 1..=MAX_RETRIES {
            if self.config.debug {
                println!("PUT Upload attempt {}/{}", attempt, MAX_RETRIES);
            }
            
            // Reset progress at the start of each attempt
            if let Some(progress) = &progress {
                if let Ok(mut progress) = progress.lock() {
                    progress(0.0);
                }
            }

            // Create a progress reader that works for file uploads
            let progress_reader = ProgressReader::new_file(
                File::open(file_path)?,
                file_size,
                progress.clone(),
            );

            let body = reqwest::blocking::Body::sized(progress_reader, file_size);
            
            match self.do_request::<UploadResponse>(
                reqwest::Method::PUT, 
                &format!("file/{}", custom_filename), 
                Some(body)
            ) {
                Ok(result) => {
                    // Reset progress to 100% when complete
                    if let Some(progress) = &progress {
                        if let Ok(mut progress) = progress.lock() {
                            progress(1.0);
                        }
                    }
                    return Ok(result);
                }
                Err(e) => {
                    // Check if this is a retryable error
                    let should_retry = match &e {
                        PixelDrainError::Reqwest(reqwest_err) => {
                            reqwest_err.is_timeout() || 
                            reqwest_err.is_connect() || 
                            reqwest_err.is_request() ||
                            reqwest_err.to_string().contains("request or response body error")
                        }
                        PixelDrainError::Api(api_err) => {
                            api_err.status.is_server_error()
                        }
                        _ => false,
                    };
                    
                    if should_retry && attempt < MAX_RETRIES {
                        if self.config.debug {
                            println!("PUT Upload failed, retrying in {} seconds...", RETRY_DELAY.as_secs());
                        }
                        std::thread::sleep(RETRY_DELAY);
                        continue;
                    } else {
                        return Err(e);
                    }
                }
            }
        }
        
        // This should never be reached, but just in case
        Err(PixelDrainError::Api(ApiError {
            status: reqwest::StatusCode::INTERNAL_SERVER_ERROR,
            value: "error".to_string(),
            message: "PUT Upload failed after all retry attempts".to_string(),
        }))
    }

    /// Upload a stream using PUT /api/file/{filename} (like Go CLI)
    pub fn upload_stream_put<R: Read + Send + 'static>(
        &self,
        reader: R,
        filename: &str,
        progress: Option<ProgressCallback>,
    ) -> Result<UploadResponse, PixelDrainError> {
        
        if self.config.api_key.is_none() {
            return Err(PixelDrainError::MissingApiKey);
        }

        // Create a progress reader that works for streaming uploads
        let progress_reader = ProgressReader::new_stream(reader, progress);
        
        // Build the PUT request with streaming body
        let mut request = self.build_request(reqwest::Method::PUT, &format!("file/{}", urlencoding::encode(filename)));
        request = request.body(reqwest::blocking::Body::new(progress_reader));
        
        // Send the request
        let resp = request.send()?;
        let status = resp.status();
        
        if !status.is_success() {
            let error_text = resp.text().unwrap_or_default();
            return Err(PixelDrainError::Api(ApiError {
                status,
                value: "error".to_string(),
                message: error_text,
            }));
        }
        
        let response: UploadResponse = resp.json()?;
        Ok(response)
    }

    /// Get rate limits from the server
    #[allow(dead_code)]
    pub fn get_rate_limits(&self) -> Result<RateLimits, PixelDrainError> {
        self.do_request(reqwest::Method::GET, "misc/rate_limits", None)
    }

    /// Get cluster speed information
    #[allow(dead_code)]
    pub fn get_cluster_speed(&self) -> Result<ClusterSpeed, PixelDrainError> {
        self.do_request(reqwest::Method::GET, "misc/cluster_speed", None)
    }

    /// Check if server is overloaded before uploading
    #[allow(dead_code)]
    pub fn check_server_status(&self) -> Result<bool, PixelDrainError> {
        let rate_limits = self.get_rate_limits()?;
        Ok(!rate_limits.server_overload)
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

    /// Get all lists for the user
    pub fn get_user_lists(&self) -> Result<UserListsResponse, PixelDrainError> {
        // User lists require authentication, so pass false for anonymous
        let request = self.build_request(reqwest::Method::GET, "user/lists");
        let resp = request.send()?;
        let status = resp.status();
        
        if self.config.debug {
            println!("Get User Lists Request: GET /user/lists");
            println!("Response Status: {}", status);
        }

        if !status.is_success() {
            let error_text = resp.text().unwrap_or_default();
            if self.config.debug {
                println!("Error response: {}", error_text);
            }
            return Err(PixelDrainError::Api(ApiError {
                status,
                value: "error".to_string(),
                message: error_text,
            }));
        }

        let response_text = resp.text()?;
        if self.config.debug {
            println!("Response body: {}", response_text);
        }

        // Handle empty response
        if response_text.trim().is_empty() {
            return Ok(UserListsResponse { lists: Vec::new() });
        }

        // Try multiple parsing strategies
        // Strategy 1: Try to parse as UserListsResponse with "lists" wrapper
        if let Ok(parsed) = serde_json::from_str::<UserListsResponse>(&response_text) {
            return Ok(parsed);
        }
        
        // Strategy 2: Try to parse as a direct array of ListInfo
        if let Ok(lists) = serde_json::from_str::<Vec<ListInfo>>(&response_text) {
            return Ok(UserListsResponse { lists });
        }
        
        // Strategy 3: Try to parse as a generic JSON value to understand the structure
        if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(&response_text) {
            if self.config.debug {
                println!("Raw JSON structure: {}", serde_json::to_string_pretty(&json_value).unwrap_or_default());
            }
            
            // Check if it's an object with a different key name
            if let Some(obj) = json_value.as_object() {
                // Try common variations
                for key in ["lists", "data", "items", "results"] {
                    if let Some(array_value) = obj.get(key) {
                        if let Ok(lists) = serde_json::from_value::<Vec<ListInfo>>(array_value.clone()) {
                            return Ok(UserListsResponse { lists });
                        }
                    }
                }
            }
        }
        
        // If all parsing attempts fail, return an error with the response text
        Err(PixelDrainError::Serde(serde_json::from_str::<serde_json::Value>("invalid").unwrap_err()))
    }

    /// Get details for a specific list
    #[allow(dead_code)]
    pub fn get_list(&self, list_id: &str) -> Result<DetailedListInfo, PixelDrainError> {
        self.do_request(reqwest::Method::GET, &format!("list/{}", list_id), None)
    }

    /// Create a new list
    pub fn create_list(&self, req: &CreateListRequest) -> Result<ListInfo, PixelDrainError> {
        let body = serde_json::to_vec(req)?;
        let req_body = reqwest::blocking::Body::from(body);
        
        // Creating lists requires authentication, so pass false for anonymous
        let mut request = self.build_request(reqwest::Method::POST, "list");
        request = request.header(header::CONTENT_TYPE, "application/json");
        request = request.body(req_body);
        
        let resp = request.send()?;
        let status = resp.status();
        
        if self.config.debug {
            println!("Create List Request: POST /list");
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

        // Parse the creation response (just contains ID)
        let creation_resp: ListCreationResponse = resp.json()?;
        
        // Now fetch the full list info and convert to simple ListInfo
        let detailed = self.get_list(&creation_resp.id)?;
        Ok(ListInfo {
            id: detailed.id,
            title: detailed.title,
            date_created: detailed.date_created,
            file_count: detailed.file_count as i64,
            files: None, // Don't include files in simple view
            can_edit: detailed.can_edit,
        })
    }

    /// Update a list (change title/files)
    pub fn update_list(&self, list_id: &str, req: &CreateListRequest) -> Result<ListInfo, PixelDrainError> {
        let body = serde_json::to_vec(req)?;
        let req_body = reqwest::blocking::Body::from(body);
        
        // Updating lists requires authentication, so pass false for anonymous
        let mut request = self.build_request(reqwest::Method::PUT, &format!("list/{}", list_id));
        request = request.header(header::CONTENT_TYPE, "application/json");
        request = request.body(req_body);
        
        let resp = request.send()?;
        let status = resp.status();
        
        if self.config.debug {
            println!("Update List Request: PUT /list/{}", list_id);
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

        let detailed: DetailedListInfo = resp.json()?;
        Ok(ListInfo {
            id: detailed.id,
            title: detailed.title,
            date_created: detailed.date_created,
            file_count: detailed.file_count as i64,
            files: None,
            can_edit: detailed.can_edit,
        })
    }

    /// Delete a list
    pub fn delete_list(&self, list_id: &str) -> Result<(), PixelDrainError> {
        let _: serde_json::Value = self.do_request(reqwest::Method::DELETE, &format!("list/{}", list_id), None)?;
        Ok(())
    }

    /// Add a view to a file (based on pixeldrain_api_client)
    #[allow(dead_code)]
    pub fn post_file_view(&self, file_id: &str, view_token: &str) -> Result<(), PixelDrainError> {
        let form_data = [("token", view_token)];
        self.do_form_request::<()>(reqwest::Method::POST, &format!("file/{}/view", file_id), &form_data)
            .map(|_| ())
    }

    /// Get reCaptcha site key (based on pixeldrain_api_client)
    #[allow(dead_code)]
    pub fn get_misc_recaptcha(&self) -> Result<RecaptchaInfo, PixelDrainError> {
        self.do_request(reqwest::Method::GET, "misc/recaptcha", None)
    }

    /// Get Sia cryptocurrency price (based on pixeldrain_api_client)
    #[allow(dead_code)]
    pub fn get_sia_price(&self) -> Result<SiaPrice, PixelDrainError> {
        self.do_request(reqwest::Method::GET, "misc/sia_price", None)
    }

    /// Get user information (enhanced based on pixeldrain_api_client)
    #[allow(dead_code)]
    pub fn get_user(&self) -> Result<UserInfo, PixelDrainError> {
        self.do_request(reqwest::Method::GET, "user", None)
    }

    /// Create a new user session (based on pixeldrain_api_client)
    #[allow(dead_code)]
    pub fn post_user_session(&self, app_name: &str) -> Result<UserSession, PixelDrainError> {
        let form_data = [("app_name", app_name)];
        self.do_form_request(reqwest::Method::POST, "user/session", &form_data)
    }

    /// Get all user sessions (based on pixeldrain_api_client)
    #[allow(dead_code)]
    pub fn get_user_sessions(&self) -> Result<Vec<UserSession>, PixelDrainError> {
        self.do_request(reqwest::Method::GET, "user/session", None)
    }

    /// Delete a user session (based on pixeldrain_api_client)
    #[allow(dead_code)]
    pub fn delete_user_session(&self, session_key: &str) -> Result<(), PixelDrainError> {
        self.do_request::<()>(reqwest::Method::DELETE, &format!("user/session/{}", session_key), None)
            .map(|_| ())
    }

    /// Get user activity log (based on pixeldrain_api_client)
    #[allow(dead_code)]
    pub fn get_user_activity(&self) -> Result<Vec<UserActivity>, PixelDrainError> {
        self.do_request(reqwest::Method::GET, "user/activity", None)
    }

    /// Get user transaction history (based on pixeldrain_api_client)
    #[allow(dead_code)]
    pub fn get_user_transactions(&self) -> Result<Vec<UserTransaction>, PixelDrainError> {
        self.do_request(reqwest::Method::GET, "user/transactions", None)
    }

    /// Get filesystem buckets (based on pixeldrain_api_client)
    #[allow(dead_code)]
    pub fn get_filesystems(&self) -> Result<Vec<FilesystemNode>, PixelDrainError> {
        self.do_request(reqwest::Method::GET, "filesystem", None)
    }

    /// Get filesystem path (based on pixeldrain_api_client)
    #[allow(dead_code)]
    pub fn get_filesystem_path(&self, path: &str) -> Result<FilesystemPath, PixelDrainError> {
        let encoded_path = path.replace("/", "%2F");
        self.do_request(reqwest::Method::GET, &format!("filesystem/{}?stat", encoded_path), None)
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
    #[serde(default)]
    pub file_count: i32,
    pub is_admin: bool,
    pub balance_micro_eur: i64,
    pub hotlinking_enabled: bool,
    pub monthly_transfer_cap: u64,
    pub monthly_transfer_used: u64,
    #[serde(default)]
    pub file_viewer_branding: Option<HashMap<String, String>>,
    pub file_embed_domains: String,
    pub skip_file_viewer: bool,
    pub affiliate_user_name: String,
    pub checkout_country: String,
    pub checkout_name: String,
    pub checkout_provider: String,
    // ResponseDefault fields (embedded in go-pd)
    #[serde(default)]
    pub status_code: Option<i32>,
    #[serde(default)]
    pub success: Option<bool>,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SubscriptionType {
    pub id: String,
    pub name: String,
    pub r#type: String,
    pub file_size_limit: u64,
    pub file_expiry_days: u64,
    pub storage_space: i64, // Can be -1 for unlimited
    pub price_per_tb_storage: u64,
    pub price_per_tb_bandwidth: u64,
    pub monthly_transfer_cap: u64,
    pub file_viewer_branding: bool,
    #[serde(default)]
    pub filesystem_access: bool,
    #[serde(default)]
    pub filesystem_storage_limit: u64,
}

#[derive(Debug, Deserialize)]
pub struct UserFilesResponse {
    pub files: Vec<FileInfo>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RateLimits {
    pub server_overload: bool,
    pub speed_limit: i32,
    pub download_limit: i32,
    pub download_limit_used: i32,
    pub transfer_limit: i32,
    pub transfer_limit_used: i32,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ClusterSpeed {
    pub server_tx: i64,
    pub server_rx: i64,
    pub cache_tx: i64,
    pub cache_rx: i64,
    pub storage_tx: i64,
    pub storage_rx: i64,
}

/// Simple ListInfo for user lists endpoint (matches go-pd ListsGetUser)
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ListInfo {
    pub id: String,
    pub title: String,
    pub date_created: DateTime<Utc>,
    #[serde(default)]
    pub file_count: i64,
    #[serde(default)]
    pub files: Option<serde_json::Value>, // Keep as generic Value for user lists endpoint
    #[serde(default)]
    pub can_edit: bool,
}

/// Detailed ListInfo for single list endpoint
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DetailedListInfo {
    pub id: String,
    pub title: String,
    pub files: Vec<ApiListFile>,
    pub date_created: DateTime<Utc>,
    #[serde(default)]
    pub date_updated: Option<DateTime<Utc>>,
    pub can_edit: bool,
    #[serde(default)]
    pub can_delete: bool,
    #[serde(default)]
    pub file_count: i32,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CreateListRequest {
    pub title: String,
    pub files: Vec<ListFile>, // Changed from Vec<String> to Vec<ListFile>
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct ListFile {
    pub id: String,
    pub description: String,
}

/// ListFile as returned by the API (matches pixeldrain_api_client)
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ApiListFile {
    pub detail_href: String,
    pub description: String,
    #[serde(flatten)]
    pub file_info: FileInfo,
}

/// Response from list creation API
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ListCreationResponse {
    pub id: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct UserListsResponse {
    pub lists: Vec<ListInfo>,
}

// ============================================================================
// Additional API Structures (from pixeldrain_api_client analysis)
// ============================================================================

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RecaptchaInfo {
    pub site_key: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SiaPrice {
    pub price: f64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct UserSession {
    pub auth_key: String,
    pub creation_ip: String,
    pub user_agent: String,
    pub app_name: String,
    pub creation_time: DateTime<Utc>,
    pub last_used_time: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct UserActivity {
    pub time: DateTime<Utc>,
    pub event: String,
    pub file_id: String,
    pub file_name: String,
    pub file_removal_reason: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct UserTransaction {
    pub time: DateTime<Utc>,
    pub new_balance: i64,
    pub deposit_amount: i64,
    pub subscription_charge: i64,
    pub storage_charge: i64,
    pub storage_used: i32,
    pub bandwidth_charge: i64,
    pub bandwidth_used: i32,
    pub affiliate_amount: i64,
    pub affiliate_count: i32,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FilesystemNode {
    pub r#type: String,
    pub path: String,
    pub name: String,
    pub created: DateTime<Utc>,
    pub modified: DateTime<Utc>,
    pub mode_string: String,
    pub mode_octal: String,
    pub created_by: String,
    pub abuse_type: Option<String>,
    pub abuse_report_time: Option<DateTime<Utc>>,
    pub file_size: i32,
    pub file_type: String,
    pub sha256_sum: String,
    pub id: Option<String>,
    pub properties: Option<HashMap<String, String>>,
    pub logging_enabled_at: DateTime<Utc>,
    pub link_permissions: Option<Permissions>,
    pub user_permissions: Option<HashMap<String, Permissions>>,
    pub password_permissions: Option<HashMap<String, Permissions>>,
    pub custom_domain_name: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Permissions {
    pub owner: bool,
    pub read: bool,
    pub write: bool,
    pub delete: bool,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FilesystemPath {
    pub path: Vec<FilesystemNode>,
    pub base_index: i32,
    pub children: Vec<FilesystemNode>,
    pub permissions: Permissions,
    pub context: FilesystemContext,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FilesystemContext {
    pub premium_transfer: bool,
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
// Progress Tracking
// ============================================================================

pub type ProgressCallback = Arc<Mutex<dyn FnMut(f32) + Send>>;

/// Generic progress reader that works for both file-based and streaming uploads
struct ProgressReader<R: Read> {
    inner: R,
    total: Option<u64>, // None for streaming uploads
    read: u64,
    cb: Option<ProgressCallback>,
}

impl<R: Read> ProgressReader<R> {
    fn new_file(inner: R, total: u64, cb: Option<ProgressCallback>) -> Self {
        Self {
            inner,
            total: Some(total),
            read: 0,
            cb,
        }
    }
    
    fn new_stream(inner: R, cb: Option<ProgressCallback>) -> Self {
        Self {
            inner,
            total: None,
            read: 0,
            cb,
        }
    }
    
    fn call_progress(&mut self, progress: f32) {
        if let Some(cb) = &mut self.cb {
            if let Ok(mut callback) = cb.lock() {
                callback(progress);
            }
        }
    }
}

impl<R: Read> Read for ProgressReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let bytes_read = self.inner.read(buf)?;
        self.read += bytes_read as u64;
        
        // Calculate progress
        if let Some(total) = self.total {
            if total > 0 {
                let progress = (self.read as f32 / total as f32).min(1.0);
                self.call_progress(progress);
            }
        } else {
            // For streaming, estimate progress based on bytes read
            // This is a rough estimate - could be improved with better heuristics
            let estimated_progress = (self.read as f32 / 1024.0 / 1024.0).min(0.95); // Cap at 95% for streaming
            self.call_progress(estimated_progress);
        }
        
        Ok(bytes_read)
    }
}
