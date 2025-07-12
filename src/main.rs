// PixelDrain egui App - Clean Modern UI
use arboard::Clipboard;
use chrono::{DateTime, Utc};
use eframe::{egui, App, NativeOptions};
use egui::IconData;
use rfd::FileDialog;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::env;
use std::time::Instant;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

// Embed the icon as data bytes at compile time for future use
const ICON_DATA: &[u8] = include_bytes!("../assets/icon.png");

fn icon_data_from_png() -> Option<IconData> {
    if let Ok(img) = image::load_from_memory(ICON_DATA) {
        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();
        Some(IconData {
            rgba: rgba.into_raw(),
            width,
            height,
        })
    } else {
        None
    }
}

mod pixeldrain_api;
use pixeldrain_api::{
    FileInfo, PixelDrainConfig, PixelDrainClient,
    UserInfo,
};

#[derive(Default, Serialize, Deserialize)]
struct AppState {
    api_key: String,
    download_location: String,
    upload_history: Vec<UploadHistoryEntry>,
    download_history: Vec<DownloadHistoryEntry>,
    last_error: Option<String>,
    file_list: Vec<FileInfo>,
    user_info: Option<UserInfo>,
    // Debug info
    debug_messages: Vec<String>,
    last_operation_time: Option<DateTime<Utc>>,
}

#[derive(Clone, Serialize, Deserialize)]
struct UploadHistoryEntry {
    id: String,
    url: String,
    filename: String,
    size: u64,
    timestamp: DateTime<Utc>,
}

#[derive(Clone, Serialize, Deserialize)]
struct DownloadHistoryEntry {
    url: String,
    filename: String,
    local_path: String,
    timestamp: DateTime<Utc>,
}

struct PixelDrainApp {
    state: Arc<Mutex<AppState>>,
    tab: Tab,
    // Upload
    upload_progress: Arc<Mutex<f32>>,
    upload_file: Option<PathBuf>,
    upload_thread_running: Arc<Mutex<bool>>,
    // Download
    download_url: String,
    download_progress: Arc<Mutex<f32>>,
    download_thread_running: Arc<Mutex<bool>>,
    download_save_path: Option<PathBuf>,
    // Settings input state
    settings_api_key: String,
    settings_download_location: String,
    // UI State
    show_error: bool,
    error_message: String,
    // Debug
    show_debug: bool,
    debug_log: Vec<String>,
}

#[derive(PartialEq)]
enum Tab {
    Upload,
    Download,
    List,
    Settings,
    About,
}

impl Default for Tab {
    fn default() -> Self {
        Tab::Upload
    }
}

impl Default for PixelDrainApp {
    fn default() -> Self {
        let mut app = Self {
            state: Arc::new(Mutex::new(AppState::default())),
            tab: Tab::default(),
            upload_progress: Arc::new(Mutex::new(0.0)),
            upload_file: None,
            upload_thread_running: Arc::new(Mutex::new(false)),
            download_url: String::new(),
            download_progress: Arc::new(Mutex::new(0.0)),
            download_thread_running: Arc::new(Mutex::new(false)),
            download_save_path: None,
            settings_api_key: String::new(),
            settings_download_location: String::new(),
            show_error: false,
            error_message: String::new(),
            show_debug: false,
            debug_log: Vec::new(),
        };
        
        // Load settings on startup
        app.load_settings();
        
        app
    }
}

impl App for PixelDrainApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            self.render_ui(ui, ctx);
        });
    }
}

impl PixelDrainApp {
    fn add_debug_log(&mut self, message: String) {
        let timestamp = chrono::Utc::now().format("%H:%M:%S").to_string();
        let log_entry = format!("[{}] {}", timestamp, message);
        self.debug_log.push(log_entry.clone());
        
        // Keep only last 50 debug messages
        if self.debug_log.len() > 50 {
            self.debug_log.remove(0);
        }
        
        // Also add to state for persistence
        let mut state = self.state.lock().unwrap();
        state.debug_messages.push(log_entry);
        if state.debug_messages.len() > 50 {
            state.debug_messages.remove(0);
        }
        state.last_operation_time = Some(chrono::Utc::now());
    }

    fn render_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        // Header with title and tabs
        ui.horizontal(|ui| {
            //ui.add_space(20.0);
            
            for (tab, label) in [
                (Tab::Upload, "📤 Upload"),
                (Tab::Download, "📥 Download"),
                (Tab::List, "📋 Files"),
                (Tab::Settings, "⚙ Settings"),
                (Tab::About, "ℹ About"),
            ] {
                if ui.selectable_label(self.tab == tab, label).clicked() {
                    self.tab = tab;
                }
            }
            
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("🔧 Debug").clicked() {
                    self.show_debug = !self.show_debug;
                }
            });
        });
        
        ui.separator();

        // Main content area
        match self.tab {
            Tab::Upload => self.upload_tab(ctx, ui),
            Tab::Download => self.download_tab(ctx, ui),
            Tab::List => self.list_tab(ui),
            Tab::Settings => self.settings_tab(ui),
            Tab::About => self.about_tab(ui),
        }

        // Debug panel
        if self.show_debug {
            self.render_debug_panel(ui);
        }

        // Error popup
        if self.show_error {
            self.render_error_popup(ctx);
        }
    }

    fn render_debug_panel(&mut self, ui: &mut egui::Ui) {
        ui.separator();
        ui.label("🔧 Debug Information");
        
        // Get thread status and last operation time first
        let upload_running = *self.upload_thread_running.lock().unwrap();
        let download_running = *self.download_thread_running.lock().unwrap();
        let last_op = self.state.lock().unwrap().last_operation_time;
        let debug_messages = self.debug_log.clone();
        
        // Show recent debug messages
        egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
            for message in &debug_messages {
                ui.label(message);
            }
        });
        
        // Show thread status
        ui.horizontal(|ui| {
            ui.label(format!("Upload thread: {}", if upload_running { "🔄 Running" } else { "⏸ Idle" }));
            ui.label(format!("Download thread: {}", if download_running { "🔄 Running" } else { "⏸ Idle" }));
        });
        
        // Show last operation time
        if let Some(last_op) = last_op {
            ui.label(format!("Last operation: {}", last_op.format("%H:%M:%S")));
        }
        
        // Clear debug log button
        if ui.button("🗑 Clear Debug Log").clicked() {
            self.debug_log.clear();
            self.state.lock().unwrap().debug_messages.clear();
        }
    }

    fn upload_tab(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        // Check for errors and display them
        let error = {
            let state = self.state.lock().unwrap();
            state.last_error.clone()
        };
        
        if let Some(error_msg) = error {
            ui.colored_label(egui::Color32::RED, format!("❌ Error: {}", error_msg));
            ui.separator();
        }

        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label("File to upload:");
                
                if let Some(path) = &self.upload_file {
                    // Better file path display with proper wrapping
                    ui.add_space(5.0);
                    ui.horizontal_wrapped(|ui| {
                        ui.label("📄");
                        // Constrain width for proper wrapping in narrow windows
                        ui.add(egui::Label::new(path.display().to_string()).wrap());
                    });
                    ui.label(format!("📏 Size: {}", self.format_file_size(path)));
                } else {
                    ui.label("📁 No file selected");
                }
                
                if ui.button("📁 Choose File").clicked() {
                    if let Some(path) = FileDialog::new().pick_file() {
                        self.upload_file = Some(path);
                        // Reset progress when new file is selected
                        *self.upload_progress.lock().unwrap() = 0.0;
                        // Clear any previous errors
                        self.state.lock().unwrap().last_error = None;
                    }
                }

                if let Some(_path) = &self.upload_file {
                    let is_running = *self.upload_thread_running.lock().unwrap();
                    if ui.add_enabled(!is_running, egui::Button::new(if is_running { "⏳ Uploading..." } else { "🚀 Upload" })).clicked() {
                        self.start_upload(self.upload_file.clone().unwrap(), ctx.clone());
                    }
                } else {
                    ui.add_enabled_ui(false, |ui| {
                        let _ = ui.button("🚀 Upload");
                    });
                }
                
                // Show upload progress
                let progress = *self.upload_progress.lock().unwrap();
                if progress > 0.0 && progress < 1.0 {
                    ui.label("📤 Uploading...");
                    ui.add(egui::ProgressBar::new(progress).show_percentage());
                    ui.label(format!("Progress: {:.1}%", progress * 100.0));
                    // Request repaint more frequently for smoother updates
                    ctx.request_repaint_after(std::time::Duration::from_millis(16)); // ~60 FPS
                } else if progress >= 1.0 {
                    ui.label("✅ Upload complete! URL copied to clipboard.");
                }
            });
        });

        // Drag and drop support
        if ctx.input(|i| !i.raw.dropped_files.is_empty()) {
            let dropped = ctx.input(|i| i.raw.dropped_files.clone());
            if let Some(file) = dropped.iter().find_map(|f| f.path.clone()) {
                self.upload_file = Some(file);
                // Reset progress when new file is dropped
                *self.upload_progress.lock().unwrap() = 0.0;
                // Clear any previous errors
                self.state.lock().unwrap().last_error = None;
            }
        }

        ui.separator();

        // Recent uploads with text wrapping for URLs
        ui.label("Recent Uploads");
        
        let state = self.state.lock().unwrap();
        if state.upload_history.is_empty() {
            ui.label("No uploads yet");
        } else {
            egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                for entry in state.upload_history.iter().rev().take(5) {
                    ui.horizontal(|ui| {
                        ui.label(format!("📄 {}", entry.filename));
                        ui.label(format!("({})", self.format_file_size_bytes(entry.size)));
                        if ui.button("📋 Copy").clicked() {
                            let _ = Clipboard::new().and_then(|mut c| c.set_text(entry.url.clone()));
                        }
                    });
                    // Use text wrapping for URLs
                    ui.horizontal_wrapped(|ui| {
                        ui.label("🔗");
                        ui.add(egui::Label::new(&entry.url).wrap());
                    });
                    ui.label(format!("🕐 {}", entry.timestamp.format("%Y-%m-%d %H:%M:%S")));
                    ui.separator();
                }
            });
        }
    }

    fn download_tab(&mut self, _ctx: &egui::Context, ui: &mut egui::Ui) {
        // Check for errors and display them
        let error = {
            let state = self.state.lock().unwrap();
            state.last_error.clone()
        };
        
        if let Some(error_msg) = error {
            ui.colored_label(egui::Color32::RED, format!("❌ Error: {}", error_msg));
            ui.separator();
        }

        ui.vertical(|ui| {
            // URL input
            ui.horizontal(|ui| {
                ui.label("URL:");
                ui.add(egui::TextEdit::singleline(&mut self.download_url).desired_width(120.0));
            });
            
            // Download button
            let can_download = !self.download_url.is_empty() && self.download_save_path.is_some();
            if ui.add_enabled(can_download, egui::Button::new("⬇ Download")).clicked() && !*self.download_thread_running.lock().unwrap() {
                self.start_download();
            }

            // Folder picker and path on same line with better wrapping
            ui.horizontal(|ui| {
                if ui.button("📁").on_hover_text("Choose Folder").clicked() {
                    if let Some(folder) = FileDialog::new().pick_folder() {
                        self.download_save_path = Some(folder);
                    }
                }
                
                // Folder path display with improved wrapping
                let loc = self
                    .download_save_path
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "Choose download folder...".to_string());
                
                // Use horizontal_wrapped for better text wrapping
                ui.horizontal_wrapped(|ui| {
                    ui.add(egui::Label::new(loc).wrap());
                });
            });
            
            // Progress/status
            let progress = *self.download_progress.lock().unwrap();
            if progress > 0.0 && progress < 1.0 {
                ui.add(egui::ProgressBar::new(progress).show_percentage());
            } else if progress >= 1.0 {
                ui.label("✅ Done");
            }
        });

        ui.separator();

        // Recent downloads
        ui.label("Recent Downloads");
        let state = self.state.lock().unwrap();
        if state.download_history.is_empty() {
            ui.label("No downloads yet");
        } else {
            egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                for entry in state.download_history.iter().rev().take(5) {
                    ui.horizontal(|ui| {
                        ui.label(format!("📄 {}", entry.filename));
                    });
                    ui.label(format!("📍 {}", entry.local_path));
                    ui.label(format!("🕐 {}", entry.timestamp.format("%Y-%m-%d %H:%M:%S")));
                    ui.separator();
                }
            });
        }
    }

    fn list_tab(&mut self, ui: &mut egui::Ui) {
        // Check for errors and display them
        let error = {
            let state = self.state.lock().unwrap();
            state.last_error.clone()
        };
        
        if let Some(error_msg) = error {
            ui.colored_label(egui::Color32::RED, format!("❌ Error: {}", error_msg));
            ui.separator();
        }

        let mut refresh_clicked = false;
        ui.horizontal(|ui| {
            ui.label("Your Files");
            if ui.button("🔄 Refresh").clicked() {
                refresh_clicked = true;
            }
        });

        // Check API key status
        let (api_key_set, env_key_set) = {
            let state = self.state.lock().unwrap();
            let api_key_set = !state.api_key.is_empty();
            let env_key_set = env::var("PIXELDRAIN_API_KEY").is_ok();
            (api_key_set, env_key_set)
        };

        let state = self.state.lock().unwrap();
        let file_list = state.file_list.clone();
        drop(state); // Release the lock early
        
        if file_list.is_empty() {
            if !api_key_set && !env_key_set {
                ui.colored_label(egui::Color32::YELLOW, "⚠ No API key configured");
                ui.label("Set your API key in Settings or use PIXELDRAIN_API_KEY environment variable");
                ui.label("Get your API key from https://pixeldrain.com/user/settings");
            } else if api_key_set || env_key_set {
                ui.label("No files found. Click 'Refresh' to load your files.");
            }
        } else {
            let mut copy_clicked = None;
            let mut delete_clicked = None;
            
            egui::ScrollArea::vertical().show(ui, |ui| {
                for file in &file_list {
                    // First line: File name and stats
                    ui.horizontal(|ui| {
                        ui.label(format!("📄 {}", file.name));
                        ui.label(format!("({})", self.format_file_size_bytes(file.size)));
                        ui.label(format!("👁 {} views", file.views));
                        ui.label(format!("⬇ {} downloads", file.downloads));
                    });
                    
                    // Second line: File ID and date
                    ui.horizontal(|ui| {
                        ui.label(format!("🆔 {}", file.id));
                        ui.label(format!("📅 {}", file.date_upload.format("%Y-%m-%d %H:%M:%S")));
                    });
                    
                    // Third line: Action buttons
                    ui.horizontal(|ui| {
                        if ui.button("📋 Copy URL").clicked() {
                            copy_clicked = Some(file.id.clone());
                        }
                        
                        if ui.button("🗑 Delete").clicked() {
                            delete_clicked = Some(file.id.clone());
                        }
                    });
                    
                    ui.separator();
                }
            });
            
            // Handle actions outside the closure to avoid borrowing issues
            if let Some(file_id) = copy_clicked {
                let url = format!("https://pixeldrain.com/u/{}", file_id);
                let _ = Clipboard::new().and_then(|mut c| c.set_text(url));
                self.add_debug_log(format!("Copied URL for file: {}", file_id));
            }
            
            if let Some(file_id) = delete_clicked {
                self.add_debug_log(format!("Starting delete for file: {}", file_id));
                self.delete_file(&file_id);
            }
        }
        
        // Handle refresh action
        if refresh_clicked {
            self.add_debug_log("Refreshing file list".to_string());
            self.refresh_file_list();
        }
    }

    fn settings_tab(&mut self, ui: &mut egui::Ui) {
        // Initialize settings fields if they're empty
        if self.settings_api_key.is_empty() || self.settings_download_location.is_empty() {
            let state = self.state.lock().unwrap();
            if self.settings_api_key.is_empty() {
                self.settings_api_key = state.api_key.clone();
            }
            if self.settings_download_location.is_empty() {
                self.settings_download_location = state.download_location.clone();
            }
        }

        // Get current state for display
        let (user_info, last_error) = {
            let state = self.state.lock().unwrap();
            (state.user_info.clone(), state.last_error.clone())
        };

        let mut settings_saved = false;

        // Show error if any
        if let Some(error_msg) = &last_error {
            ui.colored_label(egui::Color32::RED, format!("❌ Error: {}", error_msg));
            ui.separator();
        }

        ui.label("PixelDrain API Key:");
        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut self.settings_api_key);
            if ui.button("📋 Paste").clicked() {
                if let Ok(mut clipboard) = Clipboard::new() {
                    if let Ok(text) = clipboard.get_text() {
                        self.settings_api_key = text;
                    }
                }
            }
        });
        ui.label("Get your API key from https://pixeldrain.com/user/settings");
        
        // Show if API key is set from environment
        if let Ok(env_key) = env::var("PIXELDRAIN_API_KEY") {
            if !env_key.is_empty() {
                ui.horizontal(|ui| {
                    ui.label(format!("🔑 API Key from environment: {}...", &env_key[..8.min(env_key.len())]));
                    if ui.button("📋 Copy").clicked() {
                        let _ = Clipboard::new().and_then(|mut c| c.set_text(env_key.clone()));
                    }
                });
                if self.settings_api_key.is_empty() {
                    ui.colored_label(egui::Color32::YELLOW, "💡 Environment API key will be used automatically");
                }
            }
        }
        
        ui.separator();

        ui.label("Default Download Location:");
        ui.horizontal(|ui| {
            if ui.button("📁 Choose Folder").clicked() {
                if let Some(folder) = FileDialog::new().pick_folder() {
                    self.settings_download_location = folder.display().to_string();
                }
            }
            ui.text_edit_singleline(&mut self.settings_download_location);
        });

        ui.separator();

        // User info if available
        if let Some(user_info) = &user_info {
            ui.label("Account Information");
            ui.label(format!("👤 Username: {}", user_info.username));
            ui.label(format!("📧 Email: {}", user_info.email));
            ui.label(format!("💾 Storage: {} / {}", 
                self.format_file_size_bytes(user_info.storage_space_used),
                self.format_file_size_bytes(user_info.subscription.storage_space)
            ));
            ui.label(format!("📊 Monthly Transfer: {} / {}", 
                self.format_file_size_bytes(user_info.monthly_transfer_used),
                self.format_file_size_bytes(user_info.monthly_transfer_cap)
            ));
            ui.label(format!("💳 Balance: {} micro EUR", user_info.balance_micro_eur));
        }

        ui.separator();

        if ui.button("💾 Save Settings").clicked() {
            self.save_settings(self.settings_api_key.clone(), self.settings_download_location.clone());
            settings_saved = true;
        }
        
        // Show success message only after actually saving
        if settings_saved {
            ui.colored_label(egui::Color32::GREEN, "✅ Settings saved successfully!");
        }
    }

    fn about_tab(&mut self, ui: &mut egui::Ui) {
        ui.label("PixelDrain Client");
        ui.label("A modern desktop client for PixelDrain file sharing service.");
        ui.label("Built with Rust and egui.");
        
        ui.separator();
        
        ui.label("Features:");
        ui.label("• 📤 Upload files with progress tracking");
        ui.label("• 📥 Download files from PixelDrain URLs");
        ui.label("• 📋 Copy shareable links to clipboard");
        ui.label("• 📁 Manage your uploaded files");
        ui.label("• ⚙ Configure API key and settings");
        ui.label("• 🔑 Environment variable support (PIXELDRAIN_API_KEY)");
        
        ui.separator();
        
        ui.label("PixelDrain: https://pixeldrain.com");
        ui.label("API Documentation: https://pixeldrain.com/api");
        ui.label("Based on go-pd: https://github.com/ManuelReschke/go-pd");
    }

    fn render_error_popup(&mut self, ctx: &egui::Context) {
        let mut show_error = self.show_error;
        egui::Window::new("Error")
            .open(&mut show_error)
            .resizable(false)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.label("❌ An error occurred:");
                ui.label(&self.error_message);
                ui.add_space(10.0);
                if ui.button("OK").clicked() {
                    self.show_error = false;
                }
            });
        self.show_error = show_error;
    }

    fn start_upload(&mut self, path: PathBuf, ctx: egui::Context) {
        // Get API key from settings or environment
        let api_key = {
            let state = self.state.lock().unwrap();
            if !state.api_key.is_empty() {
                Some(state.api_key.clone())
            } else {
                env::var("PIXELDRAIN_API_KEY").ok()
            }
        };
        
        let progress = self.upload_progress.clone();
        let state = self.state.clone();
        let thread_running = self.upload_thread_running.clone();
        let ctx = ctx.clone();
        let last_update = Arc::new(AtomicU64::new(0));
        // Reset progress at start
        *self.upload_progress.lock().unwrap() = 0.0;
        *thread_running.lock().unwrap() = true;
        // Add debug log for upload start
        self.add_debug_log(format!("Starting upload: {}", path.display()));
        thread::spawn(move || {
            let config = if let Some(key) = api_key {
                PixelDrainConfig::default().with_api_key(key)
            } else {
                PixelDrainConfig::default()
            };
            let client = match PixelDrainClient::new(config) {
                Ok(client) => client,
                Err(e) => {
                    let mut state = state.lock().unwrap();
                    state.last_error = Some(format!("Failed to create client: {}", e));
                    state.debug_messages.push(format!("[{}] Upload failed - client creation: {}", 
                        chrono::Utc::now().format("%H:%M:%S"), e));
                    *thread_running.lock().unwrap() = false;
                    return;
                }
            };
            let progress_cb = {
                let progress = progress.clone();
                let ctx = ctx.clone();
                let last_update = last_update.clone();
                Arc::new(Mutex::new(move |p: f32| {
                    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64;
                    let last = last_update.load(Ordering::Relaxed);
                    if now - last >= 16 || p >= 1.0 {
                        last_update.store(now, Ordering::Relaxed);
                        let mut progress = progress.lock().unwrap();
                        *progress = p;
                        ctx.request_repaint();
                    }
                }))
            };
            let result = client.upload_file(&path, Some(progress_cb));
            let mut state = state.lock().unwrap();
            match result {
                Ok(response) => {
                    let url = response.get_file_url();
                    let entry = UploadHistoryEntry {
                        id: response.id,
                        url: url.clone(),
                        filename: path.file_name().unwrap().to_string_lossy().to_string(),
                        size: path.metadata().map(|m| m.len()).unwrap_or(0),
                        timestamp: Utc::now(),
                    };
                    state.upload_history.push(entry);
                    // Copy to clipboard
                    let _ = Clipboard::new().and_then(|mut c| c.set_text(url));
                    state.last_error = None;
                    state.debug_messages.push(format!("[{}] Upload successful: {}", 
                        chrono::Utc::now().format("%H:%M:%S"), path.file_name().unwrap().to_string_lossy()));
                }
                Err(e) => {
                    state.last_error = Some(format!("Upload error: {}", e));
                    state.debug_messages.push(format!("[{}] Upload failed: {} - {}", 
                        chrono::Utc::now().format("%H:%M:%S"), path.file_name().unwrap().to_string_lossy(), e));
                }
            }
            *thread_running.lock().unwrap() = false;
        });
    }

    fn start_download(&mut self) {
        let url = self.download_url.clone();
        let save_dir = self.download_save_path.clone();
        let progress = self.download_progress.clone();
        let state = self.state.clone();
        let thread_running = self.download_thread_running.clone();
        
        // Reset progress at start
        *self.download_progress.lock().unwrap() = 0.0;
        *thread_running.lock().unwrap() = true;
        
        thread::spawn(move || {
            let file_id = match PixelDrainClient::extract_file_id(&url) {
                Ok(id) => id,
                Err(e) => {
                    let mut state = state.lock().unwrap();
                    state.last_error = Some(format!("Invalid URL: {}", e));
                    *thread_running.lock().unwrap() = false;
                    return;
                }
            };
            
            let config = PixelDrainConfig::default();
            let client = match PixelDrainClient::new(config) {
                Ok(client) => client,
                Err(e) => {
                    let mut state = state.lock().unwrap();
                    state.last_error = Some(format!("Failed to create client: {}", e));
                    *thread_running.lock().unwrap() = false;
                    return;
                }
            };
            
            // Get file info first
            let file_info = match client.get_file_info(&file_id) {
                Ok(info) => info,
                Err(e) => {
                    let mut state = state.lock().unwrap();
                    state.last_error = Some(format!("Failed to get file info: {}", e));
                    *thread_running.lock().unwrap() = false;
                    return;
                }
            };
            
            let save_path = save_dir
                .as_ref()
                .map(|d| d.join(&file_info.name))
                .unwrap_or_else(|| PathBuf::from(&file_info.name));
            
            let progress_cb = Arc::new(Mutex::new(move |p: f32| {
                let mut progress = progress.lock().unwrap();
                *progress = p;
            }));
            let result = client.download_file(&file_id, &save_path, Some(progress_cb));
            
            let mut state = state.lock().unwrap();
            match result {
                Ok(_) => {
                    let entry = DownloadHistoryEntry {
                        url,
                        filename: file_info.name,
                        local_path: save_path.display().to_string(),
                        timestamp: Utc::now(),
                    };
                    state.download_history.push(entry);
                    state.last_error = None;
                }
                Err(e) => {
                    state.last_error = Some(format!("Download error: {}", e));
                }
            }
            *thread_running.lock().unwrap() = false;
        });
    }

    fn refresh_file_list(&self) {
        // Get API key from settings or environment
        let api_key = {
            let state = self.state.lock().unwrap();
            if !state.api_key.is_empty() {
                Some(state.api_key.clone())
            } else {
                env::var("PIXELDRAIN_API_KEY").ok()
            }
        };
        
        let state = self.state.clone();
        
        // Clear any previous errors when starting
        state.lock().unwrap().last_error = None;
        
        thread::spawn(move || {
            let config = if let Some(key) = api_key {
                PixelDrainConfig::default().with_api_key(key)
            } else {
                PixelDrainConfig::default()
            };
            
            let client = match PixelDrainClient::new(config) {
                Ok(client) => client,
                Err(e) => {
                    let mut state = state.lock().unwrap();
                    state.last_error = Some(format!("Failed to create client: {}", e));
                    return;
                }
            };
            
            match client.get_user_files() {
                Ok(response) => {
                    let mut state = state.lock().unwrap();
                    state.file_list = response.files;
                    state.last_error = None;
                }
                Err(e) => {
                    let mut state = state.lock().unwrap();
                    state.last_error = Some(format!("Failed to list files: {}", e));
                }
            }
        });
    }

    fn delete_file(&self, file_id: &str) {
        let api_key = {
            let state = self.state.lock().unwrap();
            if !state.api_key.is_empty() {
                Some(state.api_key.clone())
            } else {
                env::var("PIXELDRAIN_API_KEY").ok()
            }
        };

        let state = self.state.clone();
        let file_id = file_id.to_string();

        // Clear any previous errors when starting
        state.lock().unwrap().last_error = None;

        thread::spawn(move || {
            let start_time = Instant::now();
            
            let config = if let Some(key) = api_key.clone() {
                PixelDrainConfig::default().with_api_key(key)
            } else {
                PixelDrainConfig::default()
            };

            let client = match PixelDrainClient::new(config) {
                Ok(client) => client,
                Err(e) => {
                    let mut state = state.lock().unwrap();
                    state.last_error = Some(format!("Failed to create client: {}", e));
                    return;
                }
            };

            match client.delete_file(&file_id) {
                Ok(_) => {
                    let duration = start_time.elapsed();
                    {
                        let mut state = state.lock().unwrap();
                        state.last_error = None;
                        state.debug_messages.push(format!("[{}] Successfully deleted file {} in {:?}", 
                            chrono::Utc::now().format("%H:%M:%S"), file_id, duration));
                        state.last_operation_time = Some(chrono::Utc::now());
                    } // Release lock here
                    
                    // Refresh the file list after successful deletion
                    let state_clone = state.clone();
                    let api_key_clone = api_key.clone();
                    thread::spawn(move || {
                        thread::sleep(std::time::Duration::from_millis(500)); // Small delay
                        
                        let config = if let Some(key) = api_key_clone {
                            PixelDrainConfig::default().with_api_key(key)
                        } else {
                            PixelDrainConfig::default()
                        };
                        
                        if let Ok(client) = PixelDrainClient::new(config) {
                            if let Ok(response) = client.get_user_files() {
                                let mut state = state_clone.lock().unwrap();
                                state.file_list = response.files;
                                state.debug_messages.push(format!("[{}] File list refreshed after deletion", 
                                    chrono::Utc::now().format("%H:%M:%S")));
                            }
                        }
                    });
                }
                Err(e) => {
                    let duration = start_time.elapsed();
                    let mut state = state.lock().unwrap();
                    state.last_error = Some(format!("Failed to delete file: {} (took {:?})", e, duration));
                    state.debug_messages.push(format!("[{}] Delete failed for file {}: {} (took {:?})", 
                        chrono::Utc::now().format("%H:%M:%S"), file_id, e, duration));
                }
            }
        });
    }

    fn save_settings(&self, api_key: String, download_location: String) {
        let mut state = self.state.lock().unwrap();
        state.api_key = api_key;
        state.download_location = download_location;
        state.last_error = None;
        
        // Try to save settings to file
        if let Err(e) = self.persist_settings(&state) {
            state.last_error = Some(format!("Failed to save settings: {}", e));
        } else {
            state.last_error = None;
        }
    }
    
    fn persist_settings(&self, state: &AppState) -> Result<(), Box<dyn std::error::Error>> {
        use std::fs;
        use serde_json;
        
        // Create settings directory if it doesn't exist
        let settings_dir = directories::ProjectDirs::from("com", "pixeldrain", "client")
            .map(|proj_dirs| proj_dirs.config_dir().to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        fs::create_dir_all(&settings_dir)?;
        
        // Save settings to JSON file
        let settings_file = settings_dir.join("settings.json");
        let settings_data = serde_json::to_string_pretty(&state)?;
        fs::write(settings_file, settings_data)?;
        
        Ok(())
    }
    
    fn load_settings(&mut self) {
        use std::fs;
        use serde_json;
        
        let settings_file = directories::ProjectDirs::from("com", "pixeldrain", "client")
            .map(|proj_dirs| proj_dirs.config_dir().join("settings.json"))
            .unwrap_or_else(|| PathBuf::from("settings.json"));
            
        if let Ok(data) = fs::read_to_string(settings_file) {
            if let Ok(loaded_state) = serde_json::from_str::<AppState>(&data) {
                let mut state = self.state.lock().unwrap();
                state.api_key = loaded_state.api_key;
                state.download_location = loaded_state.download_location;
                // Don't overwrite history and other runtime data
            }
        }
    }

    fn format_file_size(&self, path: &PathBuf) -> String {
        if let Ok(metadata) = fs::metadata(path) {
            self.format_file_size_bytes(metadata.len())
        } else {
            "Unknown size".to_string()
        }
    }

    fn format_file_size_bytes(&self, bytes: u64) -> String {
        const KB: f64 = 1024.0;
        const MB: f64 = KB * 1024.0;
        const GB: f64 = MB * 1024.0;
        
        let bytes_f = bytes as f64;
        
        if bytes_f >= GB {
            format!("{:.2} GB", bytes_f / GB)
        } else if bytes_f >= MB {
            format!("{:.2} MB", bytes_f / MB)
        } else if bytes_f >= KB {
            format!("{:.2} KB", bytes_f / KB)
        } else {
            format!("{} B", bytes)
        }
    }
}

fn main() -> Result<(), eframe::Error> {
    env_logger::init();

    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([300.0, 500.0])
        .with_min_inner_size([300.0, 400.0]);
    if let Some(icon) = icon_data_from_png() {
        viewport = viewport.with_icon(icon);
    }
    let options = NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "PixelDrain",
        options,
        Box::new(|_cc| Ok(Box::new(PixelDrainApp::default()))),
    )
}
