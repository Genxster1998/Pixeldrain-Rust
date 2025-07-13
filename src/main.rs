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
use std::process::{Command, Stdio};
use webbrowser;

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

#[derive(Serialize, Deserialize)]
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
    // Theme
    dark_mode: bool,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            download_location: PixelDrainApp::get_default_download_location(),
            upload_history: Vec::new(),
            download_history: Vec::new(),
            last_error: None,
            file_list: Vec::new(),
            user_info: None,
            debug_messages: Vec::new(),
            last_operation_time: None,
            dark_mode: false,
        }
    }
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
    upload_custom_filename: String,
    upload_files: Vec<PathBuf>, // Multiple files for upload
    upload_directory: Option<PathBuf>, // Directory for upload
    upload_directory_name: String, // Custom name for directory archive
    upload_thread_running: Arc<Mutex<bool>>,
    // Download
    download_url: String,
    download_progress: Arc<Mutex<f32>>,
    download_thread_running: Arc<Mutex<bool>>,
    // Settings input state
    settings_api_key: String,
    settings_download_location: String,
    // UI State
    show_error: bool,
    error_message: String,
    // Debug
    show_debug: bool,
    debug_log: Vec<String>,
    lists: Vec<pixeldrain_api::ListInfo>,
    selected_list_id: Option<String>,
    new_list_title: String,
    new_list_files: Vec<pixeldrain_api::ListFile>,
    list_error: Option<String>,
    // Add fields for editing
    edit_list_title: String,
    edit_list_files: Vec<pixeldrain_api::ListFile>,
    // Loading states
    files_loading: Arc<Mutex<bool>>,
    file_delete_loading: Arc<Mutex<bool>>,
    lists_loading: Arc<Mutex<bool>>,
    list_create_loading: Arc<Mutex<bool>>,
    list_update_loading: Arc<Mutex<bool>>,
    list_delete_loading: Arc<Mutex<bool>>,
    user_info_loading: Arc<Mutex<bool>>,
}

#[derive(PartialEq)]
enum Tab {
    Upload,
    Download,
    List,
    Lists, // New Lists tab
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
            upload_custom_filename: String::new(),
            upload_files: Vec::new(),
            upload_directory: None,
            upload_directory_name: String::new(),
            upload_thread_running: Arc::new(Mutex::new(false)),
            download_url: String::new(),
            download_progress: Arc::new(Mutex::new(0.0)),
            download_thread_running: Arc::new(Mutex::new(false)),
            settings_api_key: String::new(),
            settings_download_location: String::new(),
            show_error: false,
            error_message: String::new(),
            show_debug: false,
            debug_log: Vec::new(),
            lists: Vec::new(),
            selected_list_id: None,
            new_list_title: String::new(),
            new_list_files: Vec::new(),
            list_error: None,
            // Add fields for editing
            edit_list_title: String::new(),
            edit_list_files: Vec::new(),
            // Loading states
            files_loading: Arc::new(Mutex::new(false)),
            file_delete_loading: Arc::new(Mutex::new(false)),
            lists_loading: Arc::new(Mutex::new(false)),
            list_create_loading: Arc::new(Mutex::new(false)),
            list_update_loading: Arc::new(Mutex::new(false)),
            list_delete_loading: Arc::new(Mutex::new(false)),
            user_info_loading: Arc::new(Mutex::new(false)),
        };
        
        // Load settings on startup
        app.load_settings();
        
        app
    }
}

impl App for PixelDrainApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Apply theme on first update
        static mut FIRST_UPDATE: bool = true;
        unsafe {
            if FIRST_UPDATE {
                self.apply_theme_on_startup(ctx);
                FIRST_UPDATE = false;
            }
        }
        
        egui::CentralPanel::default().show(ctx, |ui| {
            self.render_ui(ui, ctx);
        });
    }
}

impl PixelDrainApp {
    fn add_debug_log(&mut self, message: String) {
        let timestamp = chrono::Utc::now().format("%H:%M:%S");
        let log_entry = format!("[{}] {}", timestamp, message);
        
        // Add to debug log
        self.debug_log.push(log_entry.clone());
        
        // Keep only last 100 entries
        if self.debug_log.len() > 100 {
            self.debug_log.remove(0);
        }
        
        // Also add to state debug messages
        let mut state = self.state.lock().unwrap();
        state.debug_messages.push(log_entry);
        state.last_operation_time = Some(chrono::Utc::now());
        
        // Keep only last 50 entries in state
        if state.debug_messages.len() > 50 {
            state.debug_messages.remove(0);
        }
    }

    /// Get API key with settings priority
    /// Returns the stored API key if set, otherwise the environment variable
    fn get_api_key(&self) -> Option<String> {
        // First check stored API key
        let state = self.state.lock().unwrap();
        if !state.api_key.is_empty() {
            return Some(state.api_key.clone());
        }
        
        // Fall back to environment variable
        if let Ok(env_key) = env::var("PIXELDRAIN_API_KEY") {
            if !env_key.is_empty() {
                return Some(env_key);
            }
        }
        
        None
    }

    /// Check if API key is available (either from settings or environment)
    fn has_api_key(&self) -> bool {
        self.get_api_key().is_some()
    }

    /// Check if environment variable is set (used as fallback)
    fn has_env_api_key(&self) -> bool {
        env::var("PIXELDRAIN_API_KEY").is_ok()
    }

    fn render_loading_spinner(&self, ui: &mut egui::Ui, text: &str) {
        ui.horizontal(|ui| {
            ui.ctx().request_repaint(); // Keep the spinner animated
            let time = ui.input(|i| i.time);
            let angle = time as f32 * 2.0; // Rotation speed
            
            // Draw a simple rotating circle
            let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(16.0), egui::Sense::hover());
            let painter = ui.painter();
            let center = rect.center();
            let radius = 6.0;
            
            // Draw rotating arc
            for i in 0..8 {
                let a = angle + i as f32 * std::f32::consts::PI / 4.0;
                let alpha = (1.0 - i as f32 / 8.0) * 255.0;
                let color = egui::Color32::from_gray((alpha as u8).max(50));
                let start = center + egui::Vec2::new(radius * a.cos(), radius * a.sin());
                let end = center + egui::Vec2::new((radius - 2.0) * a.cos(), (radius - 2.0) * a.sin());
                painter.line_segment([start, end], egui::Stroke::new(2.0, color));
            }
            
            ui.label(text);
        });
    }

    fn render_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        // Header with title and tabs
        ui.horizontal(|ui| {
            //ui.add_space(20.0);
            
            for (tab, label) in [
                (Tab::Upload, "📤 Upload"),
                (Tab::Download, "📥 Download"),
                (Tab::List, "📋 Files"),
                (Tab::Lists, "📚 Lists"), // New Lists tab
                (Tab::Settings, "⚙ Settings"),
                (Tab::About, "ℹ About"),
            ] {
                if ui.selectable_label(self.tab == tab, label).clicked() {
                    self.tab = tab;
                }
            }
            
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Theme toggle button
                let dark_mode = {
                    let state = self.state.lock().unwrap();
                    state.dark_mode
                };
                
                let theme_button_text = if dark_mode { "☀" } else { "🌙" };
                let tooltip_text = if dark_mode { "Switch to Light Theme" } else { "Switch to Dark Theme" };
                
                if ui.button(theme_button_text).on_hover_text(tooltip_text).clicked() {
                    let new_dark_mode = !dark_mode;
                    
                    // Update the theme state
                    {
                        let mut state = self.state.lock().unwrap();
                        state.dark_mode = new_dark_mode;
                    }
                    
                    // Apply theme to the context
                    if new_dark_mode {
                        ctx.set_visuals(egui::Visuals::dark());
                    } else {
                        ctx.set_visuals(egui::Visuals::light());
                    }
                    
                    // Save settings
                    self.save_theme_settings(new_dark_mode);
                }
                
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
            Tab::Lists => self.lists_tab(ui), // New Lists tab
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
        egui::ScrollArea::vertical().max_height(200.0).id_salt("debug_messages_scroll").show(ui, |ui| {
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
            // Show upload mode (Anonymous vs Authenticated)
            let (api_key_set, env_key_set) = {
                let api_key_set = self.has_api_key();
                let env_key_set = self.has_env_api_key();
                (api_key_set, env_key_set)
            };
            
            if api_key_set || env_key_set {
                ui.colored_label(egui::Color32::GREEN, "🔐 Authenticated Upload");
                if env_key_set {
                    ui.label("Using API key from environment variable");
                } else {
                    ui.label("Using API key from settings");
                }
            } else {
                ui.colored_label(egui::Color32::BLUE, "👤 Anonymous Upload");
                ui.label("No API key configured - upload will be anonymous");
            }
            
            ui.separator();
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
                    
                    // File rename option
                    ui.separator();
                    ui.label("📝 Rename file (optional):");
                    let original_name = path.file_name().unwrap_or_default().to_string_lossy();
                    ui.horizontal(|ui| {
                        if self.upload_custom_filename.is_empty() {
                            ui.text_edit_singleline(&mut self.upload_custom_filename);
                            if ui.button("Use original").clicked() {
                                self.upload_custom_filename = original_name.to_string();
                            }
                        } else {
                            ui.text_edit_singleline(&mut self.upload_custom_filename);
                            if ui.button("Clear").clicked() {
                                self.upload_custom_filename.clear();
                            }
                        }
                    });
                    if !self.upload_custom_filename.is_empty() {
                        ui.label(format!("Will upload as: {}", self.upload_custom_filename));
                    } else {
                        ui.label(format!("Will upload as: {}", original_name));
                    }
                } else if !self.upload_files.is_empty() {
                    // Display multiple files
                    ui.add_space(5.0);
                    ui.label(format!("📁 {} files selected:", self.upload_files.len()));
                    egui::ScrollArea::vertical().max_height(100.0).id_salt("upload_files_scroll").show(ui, |ui| {
                        for (i, path) in self.upload_files.iter().enumerate() {
                            ui.horizontal(|ui| {
                                ui.label(format!("{}. {}", i + 1, path.file_name().unwrap_or_default().to_string_lossy()));
                                ui.label(format!("({})", self.format_file_size(path)));
                            });
                        }
                    });
                } else if let Some(dir_path) = &self.upload_directory {
                    // Display directory
                    ui.add_space(5.0);
                    ui.horizontal_wrapped(|ui| {
                        ui.label("📂");
                        ui.add(egui::Label::new(dir_path.display().to_string()).wrap());
                    });
                    
                    // Directory rename option
                    ui.separator();
                    ui.label("📝 Rename archive (optional):");
                    let original_name = dir_path.file_name().unwrap_or_default().to_string_lossy();
                    ui.horizontal(|ui| {
                        if self.upload_directory_name.is_empty() {
                            ui.text_edit_singleline(&mut self.upload_directory_name);
                            if ui.button("Use original").clicked() {
                                self.upload_directory_name = format!("{}.tar.gz", original_name);
                            }
                        } else {
                            ui.text_edit_singleline(&mut self.upload_directory_name);
                            if ui.button("Clear").clicked() {
                                self.upload_directory_name.clear();
                            }
                        }
                    });
                    if !self.upload_directory_name.is_empty() {
                        ui.label(format!("Will upload as: {}", self.upload_directory_name));
                    } else {
                        ui.label(format!("Will upload as: {}.tar.gz", original_name));
                    }
                } else {
                    ui.label("📁 No file or directory selected");
                }
                
                ui.horizontal(|ui| {
                    if ui.button("📁 Select Files").clicked() {
                        if let Some(paths) = FileDialog::new().pick_files() {
                            if paths.len() == 1 {
                                // Single file selected
                                self.upload_file = Some(paths[0].clone());
                                self.upload_files.clear();
                                self.upload_directory = None;
                            } else {
                                // Multiple files selected
                                self.upload_files = paths;
                                self.upload_file = None;
                                self.upload_directory = None;
                            }
                            self.upload_custom_filename.clear();
                            self.upload_directory_name.clear();
                            // Reset progress
                            *self.upload_progress.lock().unwrap() = 0.0;
                            // Clear any previous errors
                            self.state.lock().unwrap().last_error = None;
                        }
                    }
                    
                    if ui.button("📂 Select Directory").clicked() {
                        if let Some(path) = FileDialog::new().pick_folder() {
                            self.upload_directory = Some(path);
                            self.upload_file = None;
                            self.upload_files.clear();
                            self.upload_custom_filename.clear();
                            self.upload_directory_name.clear();
                            // Reset progress
                            *self.upload_progress.lock().unwrap() = 0.0;
                            // Clear any previous errors
                            self.state.lock().unwrap().last_error = None;
                        }
                    }
                });

                let is_running = *self.upload_thread_running.lock().unwrap();
                if let Some(_path) = &self.upload_file {
                    if ui.add_enabled(!is_running, egui::Button::new(if is_running { "⏳ Uploading..." } else { "🚀 Upload" })).clicked() {
                        self.start_upload(self.upload_file.clone().unwrap(), ctx.clone());
                    }
                } else if !self.upload_files.is_empty() {
                    let button_text = if is_running { "⏳ Uploading..." } else { &format!("🚀 Upload {} Files", self.upload_files.len()) };
                    if ui.add_enabled(!is_running, egui::Button::new(button_text)).clicked() {
                        self.start_multiple_upload(self.upload_files.clone(), ctx.clone());
                    }
                } else if let Some(_dir_path) = &self.upload_directory {
                    let button_text = if is_running { "⏳ Compressing & Uploading..." } else { "🚀 Upload Directory" };
                    if ui.add_enabled(!is_running, egui::Button::new(button_text)).clicked() {
                        self.start_directory_upload(self.upload_directory.clone().unwrap(), ctx.clone());
                    }
                } else {
                    ui.add_enabled_ui(false, |ui| {
                        let _ = ui.button("🚀 Upload");
                    });
                }
                
                // Show upload progress
                let progress = *self.upload_progress.lock().unwrap();
                let is_running = *self.upload_thread_running.lock().unwrap();
                if let Some(_dir_path) = &self.upload_directory {
                    if is_running {
                        ui.horizontal(|ui| {
                            ui.add(egui::Spinner::new());
                            ui.label("Uploading directory...");
                        });
                        ctx.request_repaint_after(std::time::Duration::from_millis(100));
                    }
                } else if progress > 0.0 && progress < 1.0 {
                    ui.label("📤 Uploading...");
                    ui.add(egui::ProgressBar::new(progress).show_percentage());
                    ui.label(format!("Progress: {:.1}%", progress * 100.0));
                    ctx.request_repaint_after(std::time::Duration::from_millis(16));
                } else if progress >= 1.0 {
                    ui.label("✅ Upload complete! URL copied to clipboard.");
                }
            });
        });

        // Drag and drop support
        if ctx.input(|i| !i.raw.dropped_files.is_empty()) {
            let dropped = ctx.input(|i| i.raw.dropped_files.clone());
            let files: Vec<PathBuf> = dropped.iter().filter_map(|f| f.path.clone()).collect();
            
            if files.len() == 1 {
                // Single file dropped
                self.upload_file = Some(files[0].clone());
                self.upload_files.clear();
            } else if files.len() > 1 {
                // Multiple files dropped
                self.upload_files = files;
                self.upload_file = None;
            }
            
            self.upload_custom_filename.clear();
            // Reset progress
            *self.upload_progress.lock().unwrap() = 0.0;
            // Clear any previous errors
            self.state.lock().unwrap().last_error = None;
        }

        ui.separator();

        // Recent uploads with text wrapping for URLs
        ui.label("Recent Uploads");
        
        let state = self.state.lock().unwrap();
        if state.upload_history.is_empty() {
            ui.label("No uploads yet");
        } else {
            egui::ScrollArea::vertical().max_height(200.0).id_salt("upload_history_scroll").show(ui, |ui| {
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
            // Show download mode
            ui.colored_label(egui::Color32::BLUE, "⬇ Public File Download");
            ui.label("Download any public PixelDrain file (no API key required)");
            
            ui.separator();
            
            // URL input
            ui.horizontal(|ui| {
                ui.label("URL:");
                ui.add(egui::TextEdit::singleline(&mut self.download_url).desired_width(120.0));
            });
            
            // Download button
            let can_download = !self.download_url.is_empty();
            if ui.add_enabled(can_download, egui::Button::new("⬇ Download")).clicked() && !*self.download_thread_running.lock().unwrap() {
                self.start_download();
            }

            // Show download location info
            let download_location = {
                let state = self.state.lock().unwrap();
                if !state.download_location.is_empty() {
                    state.download_location.clone()
                } else {
                    "Default download location not set".to_string()
                }
            };
            ui.label(format!("📁 Download location: {}", download_location));
            
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
            egui::ScrollArea::vertical().max_height(200.0).id_salt("download_history_scroll").show(ui, |ui| {
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
        let files_loading = *self.files_loading.lock().unwrap();
        let file_delete_loading = *self.file_delete_loading.lock().unwrap();
        
        ui.horizontal(|ui| {
            ui.label("Your Files");
            if files_loading {
                self.render_loading_spinner(ui, "Loading files...");
            } else {
                if ui.button("🔄 Refresh").clicked() {
                    refresh_clicked = true;
                }
            }
        });

        if file_delete_loading {
            self.render_loading_spinner(ui, "Deleting file...");
        }

        // Check API key status
        let (api_key_set, env_key_set) = {
            let api_key_set = self.has_api_key();
            let env_key_set = self.has_env_api_key();
            (api_key_set, env_key_set)
        };

        let state = self.state.lock().unwrap();
        let file_list = state.file_list.clone();
        drop(state); // Release the lock early
        
        if file_list.is_empty() && !files_loading {
            if !api_key_set && !env_key_set {
                ui.colored_label(egui::Color32::YELLOW, "⚠ No API key configured");
                ui.label("Set your API key in Settings or use PIXELDRAIN_API_KEY environment variable (settings override environment)");
                ui.label("Get your API key from https://pixeldrain.com/user/settings");
            } else if api_key_set || env_key_set {
                ui.label("No files found. Click 'Refresh' to load your files.");
            }
        } else if !file_list.is_empty() {
            let mut copy_clicked = None;
            let mut delete_clicked = None;
            
            egui::ScrollArea::vertical().id_salt("files_list_scroll").show(ui, |ui| {
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
                        
                        if !file_delete_loading && ui.button("🗑 Delete").clicked() {
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

    fn lists_tab(&mut self, ui: &mut egui::Ui) {
        // Collect all actions to perform after UI rendering
        let mut refresh_lists = false;
        let mut create_list = false;
        let mut delete_list_id: Option<String> = None;
        let mut select_list_data: Option<(String, String, Vec<pixeldrain_api::ListFile>)> = None;
        let mut new_list_file_changes: Vec<(String, bool)> = Vec::new(); // (file_id, add_or_remove)
        let mut edit_list_file_changes: Vec<(String, bool)> = Vec::new();
        let mut update_list_id: Option<String> = None;
        let remove_from_existing: Vec<(String, String)> = Vec::new(); // (list_id, file_id)
        
        // Get loading states
        let lists_loading = *self.lists_loading.lock().unwrap();
        let list_create_loading = *self.list_create_loading.lock().unwrap();
        let list_update_loading = *self.list_update_loading.lock().unwrap();
        let list_delete_loading = *self.list_delete_loading.lock().unwrap();
        
        ui.heading("Your Lists");
        
        if lists_loading {
            self.render_loading_spinner(ui, "Loading lists...");
        } else {
            if ui.button("🔄 Refresh Lists").clicked() {
                refresh_lists = true;
            }
        }
        
        if let Some(err) = &self.list_error {
            ui.colored_label(egui::Color32::RED, err);
        }
        
        // Create section
        ui.separator();
        ui.heading("Create New List");
        
        if list_create_loading {
            self.render_loading_spinner(ui, "Creating list...");
        } else {
            ui.horizontal(|ui| {
                ui.label("Title:");
                ui.text_edit_singleline(&mut self.new_list_title);
            });
            ui.label("Select files to add to the list:");
            let file_list = self.state.lock().unwrap().file_list.clone();
            
            egui::ScrollArea::vertical().max_height(100.0).id_salt("new_list_files_scroll").show(ui, |ui| {
                for file in &file_list {
                    let mut selected = self.new_list_files.iter().any(|f| f.id == file.id);
                    if ui.checkbox(&mut selected, &file.name).clicked() {
                        new_list_file_changes.push((file.id.clone(), selected));
                    }
                }
            });
            
            if ui.button("Create List").clicked() {
                create_list = true;
            }
        }
        
        // Lists section
        ui.separator();
        ui.heading("Your Lists");
        
        if list_delete_loading {
            self.render_loading_spinner(ui, "Deleting list...");
        }
        
        if self.lists.is_empty() && !lists_loading {
            ui.label("No lists found. Click 'Refresh Lists' or create a new list.");
        } else if !self.lists.is_empty() {
            egui::ScrollArea::vertical().max_height(200.0).id_salt("user_lists_scroll").show(ui, |ui| {
                for list in &self.lists {
                    let selected = self.selected_list_id.as_ref().map_or(false, |id| id == &list.id);
                    if ui.selectable_label(selected, &list.title).clicked() {
                        select_list_data = Some((list.id.clone(), list.title.clone(), Vec::new())); // Empty files for now
                    }
                    ui.label(format!("Files: {} | Created: {}", list.file_count, list.date_created.format("%Y-%m-%d %H:%M:%S")));
                    if list.can_edit && !list_delete_loading && ui.button("🗑 Delete").clicked() {
                        delete_list_id = Some(list.id.clone());
                    }
                    ui.separator();
                }
            });
        }
        
        // Edit section
        if let Some(selected_id) = &self.selected_list_id {
            if let Some(list) = self.lists.iter().find(|l| &l.id == selected_id) {
                ui.separator();
                ui.heading(format!("Edit List: {}", list.title));
                
                if list_update_loading {
                    self.render_loading_spinner(ui, "Updating list...");
                } else {
                    ui.horizontal(|ui| {
                        ui.label("Title:");
                        ui.text_edit_singleline(&mut self.edit_list_title);
                    });
                    ui.label("Add/remove files:");
                    
                    let file_list = self.state.lock().unwrap().file_list.clone();
                    egui::ScrollArea::vertical().max_height(100.0).id_salt("edit_list_files_scroll").show(ui, |ui| {
                        for file in &file_list {
                            let mut selected = self.edit_list_files.iter().any(|f| f.id == file.id);
                            if ui.checkbox(&mut selected, &file.name).clicked() {
                                edit_list_file_changes.push((file.id.clone(), selected));
                            }
                        }
                    });
                    
                    if ui.button("Save Changes").clicked() {
                        update_list_id = Some(selected_id.clone());
                    }
                }
                
                ui.label(format!("Files in this list: {}", list.file_count));
                ui.label("Note: Edit individual files by fetching the detailed list view.");
                // TODO: Implement detailed list view when needed
            }
        }
        
        // Apply all collected actions
        if refresh_lists {
            self.refresh_lists();
        }
        
        if create_list {
            self.create_list();
        }
        
        if let Some(list_id) = delete_list_id {
            self.delete_list(&list_id);
        }
        
        if let Some((list_id, title, files)) = select_list_data {
            self.selected_list_id = Some(list_id);
            self.edit_list_title = title;
            self.edit_list_files = files;
        }
        
        // Apply file changes to new list
        for (file_id, add) in new_list_file_changes {
            if add {
                if !self.new_list_files.iter().any(|f| f.id == file_id) {
                    self.new_list_files.push(pixeldrain_api::ListFile {
                        id: file_id,
                        description: String::new(), // Default empty description
                    });
                }
            } else {
                self.new_list_files.retain(|f| f.id != file_id);
            }
        }
        
        // Apply file changes to edit list
        for (file_id, add) in edit_list_file_changes {
            if add {
                if !self.edit_list_files.iter().any(|f| f.id == file_id) {
                    self.edit_list_files.push(pixeldrain_api::ListFile {
                        id: file_id,
                        description: String::new(), // Default empty description
                    });
                }
            } else {
                self.edit_list_files.retain(|f| f.id != file_id);
            }
        }
        
        if let Some(list_id) = update_list_id {
            self.update_list(&list_id);
        }
        
        // Handle removing files from existing lists
        for (list_id, file_id) in remove_from_existing {
            self.edit_list_files.retain(|f| f.id != file_id);
            self.update_list(&list_id);
        }
    }
    

    fn refresh_lists(&mut self) {
        // Set loading state
        *self.lists_loading.lock().unwrap() = true;
        
        // Add retry logic similar to upload/download functions
        const MAX_RETRIES: usize = 3;
        const RETRY_DELAY: std::time::Duration = std::time::Duration::from_secs(3);
        
        let client = self.make_api_client();
        let mut last_error = None;
        
        for attempt in 1..=MAX_RETRIES {
            match client.get_user_lists() {
                Ok(resp) => {
                    self.lists = resp.lists;
                    self.list_error = None;
                    *self.lists_loading.lock().unwrap() = false;
                    return;
                }
                Err(e) => {
                    last_error = Some(e);
                    
                    // Check if this is a retryable error
                    let should_retry = match &last_error.as_ref().unwrap() {
                        pixeldrain_api::PixelDrainError::Reqwest(reqwest_err) => {
                            reqwest_err.is_timeout() || 
                            reqwest_err.is_connect() || 
                            reqwest_err.is_request() ||
                            reqwest_err.to_string().contains("request or response body error")
                        }
                        pixeldrain_api::PixelDrainError::Api(api_err) => {
                            api_err.status.is_server_error()
                        }
                        _ => false,
                    };
                    
                    if should_retry && attempt < MAX_RETRIES {
                        std::thread::sleep(RETRY_DELAY);
                        continue;
                    } else {
                        break;
                    }
                }
            }
        }
        
        // If we get here, all retries failed
        self.list_error = Some(format!("Failed to fetch lists after {} attempts: {}", 
            MAX_RETRIES, last_error.unwrap()));
        *self.lists_loading.lock().unwrap() = false;
    }
    fn create_list(&mut self) {
        // Set loading state
        *self.list_create_loading.lock().unwrap() = true;
        
        // Add retry logic similar to other operations
        const MAX_RETRIES: usize = 3;
        const RETRY_DELAY: std::time::Duration = std::time::Duration::from_secs(3);
        
        let client = self.make_api_client();
        let req = pixeldrain_api::CreateListRequest {
            title: self.new_list_title.clone(),
            files: self.new_list_files.clone(),
        };
        let mut last_error = None;
        
        for attempt in 1..=MAX_RETRIES {
            match client.create_list(&req) {
                Ok(list) => {
                    self.lists.push(list);
                    self.new_list_title.clear();
                    self.new_list_files.clear();
                    self.list_error = None;
                    *self.list_create_loading.lock().unwrap() = false;
                    return;
                }
                Err(e) => {
                    last_error = Some(e);
                    
                    // Check if this is a retryable error
                    let should_retry = match &last_error.as_ref().unwrap() {
                        pixeldrain_api::PixelDrainError::Reqwest(reqwest_err) => {
                            reqwest_err.is_timeout() || 
                            reqwest_err.is_connect() || 
                            reqwest_err.is_request() ||
                            reqwest_err.to_string().contains("request or response body error")
                        }
                        pixeldrain_api::PixelDrainError::Api(api_err) => {
                            api_err.status.is_server_error()
                        }
                        _ => false,
                    };
                    
                    if should_retry && attempt < MAX_RETRIES {
                        std::thread::sleep(RETRY_DELAY);
                        continue;
                    } else {
                        break;
                    }
                }
            }
        }
        
        // If we get here, all retries failed
        self.list_error = Some(format!("Failed to create list after {} attempts: {}", 
            MAX_RETRIES, last_error.unwrap()));
        *self.list_create_loading.lock().unwrap() = false;
    }
    fn delete_list(&mut self, list_id: &str) {
        // Set loading state
        *self.list_delete_loading.lock().unwrap() = true;
        
        // Add retry logic similar to other operations
        const MAX_RETRIES: usize = 3;
        const RETRY_DELAY: std::time::Duration = std::time::Duration::from_secs(3);
        
        let client = self.make_api_client();
        let mut last_error = None;
        
        for attempt in 1..=MAX_RETRIES {
            match client.delete_list(list_id) {
                Ok(_) => {
                    self.lists.retain(|l| l.id != list_id);
                    self.selected_list_id = None;
                    self.list_error = None;
                    *self.list_delete_loading.lock().unwrap() = false;
                    return;
                }
                Err(e) => {
                    last_error = Some(e);
                    
                    // Check if this is a retryable error
                    let should_retry = match &last_error.as_ref().unwrap() {
                        pixeldrain_api::PixelDrainError::Reqwest(reqwest_err) => {
                            reqwest_err.is_timeout() || 
                            reqwest_err.is_connect() || 
                            reqwest_err.is_request() ||
                            reqwest_err.to_string().contains("request or response body error")
                        }
                        pixeldrain_api::PixelDrainError::Api(api_err) => {
                            api_err.status.is_server_error()
                        }
                        _ => false,
                    };
                    
                    if should_retry && attempt < MAX_RETRIES {
                        std::thread::sleep(RETRY_DELAY);
                        continue;
                    } else {
                        break;
                    }
                }
            }
        }
        
        // If we get here, all retries failed
        self.list_error = Some(format!("Failed to delete list after {} attempts: {}", 
            MAX_RETRIES, last_error.unwrap()));
        *self.list_delete_loading.lock().unwrap() = false;
    }
    fn update_list(&mut self, list_id: &str) {
        // Set loading state
        *self.list_update_loading.lock().unwrap() = true;
        
        // Add retry logic similar to other operations
        const MAX_RETRIES: usize = 3;
        const RETRY_DELAY: std::time::Duration = std::time::Duration::from_secs(3);
        
        let client = self.make_api_client();
        let req = pixeldrain_api::CreateListRequest {
            title: self.edit_list_title.clone(),
            files: self.edit_list_files.clone(),
        };
        let mut last_error = None;
        
        for attempt in 1..=MAX_RETRIES {
            match client.update_list(list_id, &req) {
                Ok(updated) => {
                    if let Some(list) = self.lists.iter_mut().find(|l| l.id == list_id) {
                        *list = updated;
                    }
                    self.list_error = None;
                    *self.list_update_loading.lock().unwrap() = false;
                    return;
                }
                Err(e) => {
                    last_error = Some(e);
                    
                    // Check if this is a retryable error
                    let should_retry = match &last_error.as_ref().unwrap() {
                        pixeldrain_api::PixelDrainError::Reqwest(reqwest_err) => {
                            reqwest_err.is_timeout() || 
                            reqwest_err.is_connect() || 
                            reqwest_err.is_request() ||
                            reqwest_err.to_string().contains("request or response body error")
                        }
                        pixeldrain_api::PixelDrainError::Api(api_err) => {
                            api_err.status.is_server_error()
                        }
                        _ => false,
                    };
                    
                    if should_retry && attempt < MAX_RETRIES {
                        std::thread::sleep(RETRY_DELAY);
                        continue;
                    } else {
                        break;
                    }
                }
            }
        }
        
        // If we get here, all retries failed
        self.list_error = Some(format!("Failed to update list after {} attempts: {}", 
            MAX_RETRIES, last_error.unwrap()));
        *self.list_update_loading.lock().unwrap() = false;
    }
    fn make_api_client(&self) -> pixeldrain_api::PixelDrainClient {
        let config = if let Some(key) = self.get_api_key() {
            pixeldrain_api::PixelDrainConfig::default().with_api_key(key)
        } else {
            pixeldrain_api::PixelDrainConfig::default()
        };
        
        // Enable debug mode for troubleshooting (set to false for production)
        let config = pixeldrain_api::PixelDrainConfig { debug: false, ..config };
        
        pixeldrain_api::PixelDrainClient::new(config).unwrap()
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
                    ui.colored_label(egui::Color32::from_rgb(255, 140, 0), "💡 Environment API key will be used as fallback");
                } else {
                    ui.colored_label(egui::Color32::GREEN, "✅ Settings API key will be used (overrides environment)");
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

        // User info section with refresh button
        let user_info_loading = *self.user_info_loading.lock().unwrap();
        ui.horizontal(|ui| {
            ui.label("Account Information");
            if user_info_loading {
                self.render_loading_spinner(ui, "Loading user info...");
            } else {
                if ui.button("🔄 Refresh").clicked() {
                    self.fetch_user_info();
                }
            }
        });

        // User info if available
        if let Some(user_info) = &user_info {
            let storage_space_str = if user_info.subscription.storage_space < 0 {
                "Unlimited".to_string()
            } else {
                self.format_file_size_bytes(user_info.subscription.storage_space as u64)
            };
            ui.label(format!("👤 Username: {}", user_info.username));
            ui.label(format!("📧 Email: {}", user_info.email));
            ui.label(format!("📁 Files: {}", user_info.file_count));
            ui.label(format!("💾 Storage: {} / {}", 
                self.format_file_size_bytes(user_info.storage_space_used),
                storage_space_str
            ));
            ui.label(format!("📊 Monthly Transfer: {} / {}", 
                self.format_file_size_bytes(user_info.monthly_transfer_used),
                self.format_file_size_bytes(user_info.monthly_transfer_cap)
            ));
            ui.label(format!("⏰ Files Expiry Days: {}", user_info.subscription.file_expiry_days));
            ui.label(format!("💳 Balance: {} micro EUR", user_info.balance_micro_eur));
        } else {
            ui.colored_label(egui::Color32::GRAY, "No account information available. Set API key in settings or PIXELDRAIN_API_KEY environment variable, then click Refresh.");
        }

        ui.separator();

        if ui.button("💾 Save Settings").clicked() {
            self.save_settings(self.settings_api_key.clone(), self.settings_download_location.clone());
            settings_saved = true;
            // Try to fetch user info after saving settings
            self.fetch_user_info();
        }
        
        // Show success message only after actually saving
        if settings_saved {
            ui.colored_label(egui::Color32::GREEN, "✅ Settings saved successfully!");
        }
    }

    fn about_tab(&mut self, ui: &mut egui::Ui) {
        // Display the app icon at 48x48 size
        if let Some(icon_data) = icon_data_from_png() {
            let texture_id = ui.ctx().load_texture(
                "app_icon",
                egui::ColorImage::from_rgba_unmultiplied(
                    [icon_data.width as usize, icon_data.height as usize],
                    &icon_data.rgba,
                ),
                Default::default(),
            );
            ui.add(egui::Image::new((texture_id.id(), egui::Vec2::new(48.0, 48.0))));
        }
        
        ui.label("PixelDrain Client");
        ui.label("Copyright (c) 2025 Genxster1998");
        ui.label("A modern desktop client for PixelDrain file sharing service.");
        ui.label("Built with Rust and egui.");
        ui.label("Version: 0.1.0");
        if ui.link("🐙 GitHub: https://www.github.com/Genxster1998/Pixeldrain-Rust").clicked() {
            let _ = webbrowser::open("https://www.github.com/Genxster1998/Pixeldrain-Rust");
        }
        
        ui.separator();
        
        ui.label("Features:");
        ui.label("• 📤 Upload files with progress tracking (anonymous or authenticated)");
        ui.label("• 📥 Download files from PixelDrain URLs (no API key required)");
        ui.label("• 📋 Copy shareable links to clipboard");
        ui.label("• 📁 Manage your uploaded files");
        ui.label("• ⚙ Configure API key and settings");
        ui.label("• 🔑 Environment variable support (PIXELDRAIN_API_KEY)");
        ui.label("• 👤 Anonymous upload support (no account required)");
        
        ui.separator();
        
        if ui.link("PixelDrain: https://pixeldrain.com").clicked() {
            let _ = webbrowser::open("https://pixeldrain.com");
        }
        if ui.link("API Documentation: https://pixeldrain.com/api").clicked() {
            let _ = webbrowser::open("https://pixeldrain.com/api");
        }
        if ui.link("Official Go Implementation: https://github.com/Fornaxian/pixeldrain_api_client").clicked() {
            let _ = webbrowser::open("https://github.com/Fornaxian/pixeldrain_api_client");
        }
        if ui.link("Based on go-pd: https://github.com/ManuelReschke/go-pd").clicked() {
            let _ = webbrowser::open("https://github.com/ManuelReschke/go-pd");
        }
        if ui.link("Based on go-pd: https://github.com/jkawamoto/go-pixeldrain").clicked() {
            let _ = webbrowser::open("https://github.com/jkawamoto/go-pixeldrain");
        }
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
        // Get API key with settings priority
        let api_key = self.get_api_key();
        
        let progress = self.upload_progress.clone();
        let state = self.state.clone();
        let thread_running = self.upload_thread_running.clone();
        let ctx = ctx.clone();
        let last_update = Arc::new(AtomicU64::new(0));
        let custom_filename = self.upload_custom_filename.clone();
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
            let result = if !custom_filename.is_empty() {
                client.upload_file_put(&path, &custom_filename, Some(progress_cb))
            } else {
                client.upload_file(&path, Some(progress_cb))
            };
            let mut state = state.lock().unwrap();
            match result {
                Ok(response) => {
                    let url = response.get_file_url();
                    let entry = UploadHistoryEntry {
                        id: response.id,
                        url: url.clone(),
                        filename: if !custom_filename.is_empty() {
                            custom_filename.clone()
                        } else {
                            path.file_name().unwrap().to_string_lossy().to_string()
                        },
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

    fn start_multiple_upload(&mut self, paths: Vec<PathBuf>, ctx: egui::Context) {
        // Get API key with settings priority
        let api_key = self.get_api_key();
        
        let progress = self.upload_progress.clone();
        let state = self.state.clone();
        let thread_running = self.upload_thread_running.clone();
        let ctx = ctx.clone();
        let last_update = Arc::new(AtomicU64::new(0));
        
        // Reset progress at start
        *self.upload_progress.lock().unwrap() = 0.0;
        *thread_running.lock().unwrap() = true;
        
        // Add debug log for upload start
        self.add_debug_log(format!("Starting multiple upload: {} files", paths.len()));
        
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
                    state.debug_messages.push(format!("[{}] Multiple upload failed - client creation: {}", 
                        chrono::Utc::now().format("%H:%M:%S"), e));
                    *thread_running.lock().unwrap() = false;
                    return;
                }
            };
            
            let total_files = paths.len();
            let mut uploaded_count = 0;
            
            for (index, path) in paths.iter().enumerate() {
                let progress_cb = {
                    let progress = progress.clone();
                    let ctx = ctx.clone();
                    let last_update = last_update.clone();
                    let file_index = index;
                    let total = total_files;
                    Arc::new(Mutex::new(move |p: f32| {
                        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64;
                        let last = last_update.load(Ordering::Relaxed);
                        if now - last >= 16 || p >= 1.0 {
                            last_update.store(now, Ordering::Relaxed);
                            let mut progress = progress.lock().unwrap();
                            // Calculate overall progress across all files
                            let file_progress = (file_index as f32 + p) / total as f32;
                            *progress = file_progress;
                            ctx.request_repaint();
                        }
                    }))
                };
                
                let result = client.upload_file(path, Some(progress_cb));
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
                        uploaded_count += 1;
                        
                        state.debug_messages.push(format!("[{}] File {}/{} uploaded successfully: {}", 
                            chrono::Utc::now().format("%H:%M:%S"), uploaded_count, total_files, path.file_name().unwrap().to_string_lossy()));
                    }
                    Err(e) => {
                        state.last_error = Some(format!("Upload error for {}: {}", path.file_name().unwrap().to_string_lossy(), e));
                        state.debug_messages.push(format!("[{}] File upload failed: {} - {}", 
                            chrono::Utc::now().format("%H:%M:%S"), path.file_name().unwrap().to_string_lossy(), e));
                        break;
                    }
                }
            }
            
            // Copy the last uploaded file URL to clipboard
            if uploaded_count > 0 {
                let state = state.lock().unwrap();
                if let Some(last_entry) = state.upload_history.last() {
                    let _ = Clipboard::new().and_then(|mut c| c.set_text(last_entry.url.clone()));
                }
            }
            
            *thread_running.lock().unwrap() = false;
        });
    }

    fn start_directory_upload(&mut self, dir_path: PathBuf, _ctx: egui::Context) {
        let progress = self.upload_progress.clone();
        let state = self.state.clone();
        let thread_running = self.upload_thread_running.clone();
        let directory_name = self.upload_directory_name.clone();
        
        // Reset progress at start
        *self.upload_progress.lock().unwrap() = 0.0;
        *thread_running.lock().unwrap() = true;
        
        // Get API key with settings priority
        let api_key = self.get_api_key();
        
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
                    *thread_running.lock().unwrap() = false;
                    return;
                }
            };
            
            // Determine the archive filename
            let archive_name = if !directory_name.is_empty() {
                directory_name
            } else {
                let dir_name = dir_path.file_name().unwrap_or_default().to_string_lossy();
                format!("{}.tar.gz", dir_name)
            };
            
            // Create tar command that compresses to stdout
            let mut tar_cmd = Command::new("tar");
            tar_cmd
                .arg("czf")
                .arg("-")  // Output to stdout
                .arg("-C")
                .arg(dir_path.parent().unwrap_or(&dir_path))
                .arg(dir_path.file_name().unwrap());
            
            // Set up the command with stdout piped
            tar_cmd.stdout(Stdio::piped());
            tar_cmd.stderr(Stdio::piped());
            
            // Start the tar process
            let mut tar_process = match tar_cmd.spawn() {
                Ok(process) => process,
                Err(e) => {
                    let mut state = state.lock().unwrap();
                    state.last_error = Some(format!("Failed to start tar process: {}", e));
                    *thread_running.lock().unwrap() = false;
                    return;
                }
            };
            
            // Get stdout from tar process
            let tar_stdout = match tar_process.stdout.take() {
                Some(stdout) => stdout,
                None => {
                    let mut state = state.lock().unwrap();
                    state.last_error = Some("Failed to get tar stdout".to_string());
                    *thread_running.lock().unwrap() = false;
                    return;
                }
            };
            
            // Create a progress callback that simulates progress
            let progress_cb = Arc::new(Mutex::new(move |p: f32| {
                let mut progress = progress.lock().unwrap();
                *progress = p;
            }));
            
            // Upload the compressed data directly from tar stdout (streaming)
            eprintln!("[DEBUG] Starting streaming upload of tar.gz to {}", archive_name);
            let result = client.upload_stream_put(tar_stdout, &archive_name, Some(progress_cb));
            
            // Wait for tar process to finish
            let tar_result = tar_process.wait();

            // Print tar stderr if upload fails
            if let Some(mut tar_stderr) = tar_process.stderr {
                let mut stderr_output = String::new();
                use std::io::Read;
                let _ = tar_stderr.read_to_string(&mut stderr_output);
                if !stderr_output.trim().is_empty() {
                    eprintln!("[DEBUG] tar stderr: {}", stderr_output);
                }
            }
            
            let mut state = state.lock().unwrap();
            match result {
                Ok(response) => {
                    let url = response.get_file_url();
                    let entry = UploadHistoryEntry {
                        id: response.id,
                        url: url.clone(),
                        filename: archive_name.clone(),
                        size: 0, // We don't know the exact size since it's streamed
                        timestamp: Utc::now(),
                    };
                    state.upload_history.push(entry);
                    state.last_error = None;
                    
                    // Copy URL to clipboard
                    let _ = Clipboard::new().and_then(|mut c| c.set_text(url));
                    
                    state.debug_messages.push(format!("[{}] Directory uploaded successfully as: {}", 
                        chrono::Utc::now().format("%H:%M:%S"), archive_name));
                }
                Err(e) => {
                    eprintln!("[DEBUG] Directory upload error: {}", e);
                    state.last_error = Some(format!("Directory upload error: {}", e));
                    state.debug_messages.push(format!("[{}] Directory upload failed: {} - {}", 
                        chrono::Utc::now().format("%H:%M:%S"), archive_name, e));
                }
            }
            
            // Check if tar process had any errors
            if let Err(e) = tar_result {
                eprintln!("[DEBUG] Tar process error: {}", e);
                state.debug_messages.push(format!("[{}] Tar process error: {}", 
                    chrono::Utc::now().format("%H:%M:%S"), e));
            }
            
            *thread_running.lock().unwrap() = false;
        });
    }

    fn start_download(&mut self) {
        let url = self.download_url.clone();
        let progress = self.download_progress.clone();
        let state = self.state.clone();
        let thread_running = self.download_thread_running.clone();
        
        // Get download location from settings
        let download_location = {
            let state = self.state.lock().unwrap();
            state.download_location.clone()
        };
        
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
            
            let save_path = if !download_location.is_empty() {
                PathBuf::from(&download_location).join(&file_info.name)
            } else {
                PathBuf::from(&file_info.name)
            };
            
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
        // Set loading state
        *self.files_loading.lock().unwrap() = true;
        
        // Get API key with settings priority
        let api_key = self.get_api_key();
        
        let state = self.state.clone();
        let files_loading = self.files_loading.clone();
        
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
                    *files_loading.lock().unwrap() = false;
                    return;
                }
            };
            
            // Add retry logic similar to Lists tab functions
            const MAX_RETRIES: usize = 3;
            const RETRY_DELAY: std::time::Duration = std::time::Duration::from_secs(3);
            
            let mut last_error = None;
            
            for attempt in 1..=MAX_RETRIES {
                match client.get_user_files() {
                    Ok(response) => {
                        let mut state = state.lock().unwrap();
                        state.file_list = response.files;
                        state.last_error = None;
                        *files_loading.lock().unwrap() = false;
                        return;
                    }
                    Err(e) => {
                        last_error = Some(e);
                        
                        // Check if this is a retryable error
                        let should_retry = match &last_error.as_ref().unwrap() {
                            pixeldrain_api::PixelDrainError::Reqwest(reqwest_err) => {
                                reqwest_err.is_timeout() || 
                                reqwest_err.is_connect() || 
                                reqwest_err.is_request() ||
                                reqwest_err.to_string().contains("request or response body error")
                            }
                            pixeldrain_api::PixelDrainError::Api(api_err) => {
                                api_err.status.is_server_error()
                            }
                            _ => false,
                        };
                        
                        if should_retry && attempt < MAX_RETRIES {
                            std::thread::sleep(RETRY_DELAY);
                            continue;
                        } else {
                            break;
                        }
                    }
                }
            }
            
            // If we get here, all retries failed
            let mut state = state.lock().unwrap();
            state.last_error = Some(format!("Failed to list files after {} attempts: {}", 
                MAX_RETRIES, last_error.unwrap()));
            *files_loading.lock().unwrap() = false;
        });
    }

    fn delete_file(&self, file_id: &str) {
        // Set loading state
        *self.file_delete_loading.lock().unwrap() = true;
        
        // Get API key with settings priority
        let api_key = self.get_api_key();

        let state = self.state.clone();
        let file_id = file_id.to_string();
        let file_delete_loading = self.file_delete_loading.clone();

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
                    *file_delete_loading.lock().unwrap() = false;
                    return;
                }
            };

            // Add retry logic similar to Lists tab functions
            const MAX_RETRIES: usize = 3;
            const RETRY_DELAY: std::time::Duration = std::time::Duration::from_secs(3);
            
            let mut last_error = None;
            
            for attempt in 1..=MAX_RETRIES {
                match client.delete_file(&file_id) {
                    Ok(_) => {
                        let duration = start_time.elapsed();
                        {
                            let mut state = state.lock().unwrap();
                            state.last_error = None;
                            state.debug_messages.push(format!("[{}] Successfully deleted file {} in {:?} (attempt {})", 
                                chrono::Utc::now().format("%H:%M:%S"), file_id, duration, attempt));
                            state.last_operation_time = Some(chrono::Utc::now());
                        } // Release lock here
                        
                        *file_delete_loading.lock().unwrap() = false;
                        
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
                        return;
                    }
                    Err(e) => {
                        last_error = Some(e);
                        
                        // Check if this is a retryable error
                        let should_retry = match &last_error.as_ref().unwrap() {
                            pixeldrain_api::PixelDrainError::Reqwest(reqwest_err) => {
                                reqwest_err.is_timeout() || 
                                reqwest_err.is_connect() || 
                                reqwest_err.is_request() ||
                                reqwest_err.to_string().contains("request or response body error")
                            }
                            pixeldrain_api::PixelDrainError::Api(api_err) => {
                                api_err.status.is_server_error()
                            }
                            _ => false,
                        };
                        
                        if should_retry && attempt < MAX_RETRIES {
                            std::thread::sleep(RETRY_DELAY);
                            continue;
                        } else {
                            break;
                        }
                    }
                }
            }
            
            // If we get here, all retries failed
            let duration = start_time.elapsed();
            let error_msg = last_error.unwrap();
            let mut state = state.lock().unwrap();
            state.last_error = Some(format!("Failed to delete file after {} attempts: {} (took {:?})", 
                MAX_RETRIES, error_msg, duration));
            state.debug_messages.push(format!("[{}] Delete failed for file {} after {} attempts: {} (took {:?})", 
                chrono::Utc::now().format("%H:%M:%S"), file_id, MAX_RETRIES, error_msg, duration));
            *file_delete_loading.lock().unwrap() = false;
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
    
    fn save_theme_settings(&self, dark_mode: bool) {
        let mut state = self.state.lock().unwrap();
        state.dark_mode = dark_mode;
        
        // Try to save settings to file
        if let Err(e) = self.persist_settings(&state) {
            state.last_error = Some(format!("Failed to save theme settings: {}", e));
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
    
    fn get_default_download_location() -> String {
        use std::env;
        
        #[cfg(target_os = "windows")]
        {
            // Windows: %USERPROFILE%\Downloads
            if let Ok(userprofile) = env::var("USERPROFILE") {
                return format!("{}\\Downloads", userprofile);
            }
        }
        
        #[cfg(target_os = "macos")]
        {
            // macOS: /Users/$USER/Downloads
            if let Ok(home) = env::var("HOME") {
                return format!("{}/Downloads", home);
            }
        }
        
        #[cfg(target_os = "linux")]
        {
            // Linux: $HOME/Downloads
            if let Ok(home) = env::var("HOME") {
                return format!("{}/Downloads", home);
            }
        }
        
        // Fallback: current directory
        ".".to_string()
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
                // Use loaded download location if it's not empty, otherwise use default
                if !loaded_state.download_location.is_empty() {
                    state.download_location = loaded_state.download_location;
                } else {
                    state.download_location = Self::get_default_download_location();
                }
                // Load theme preference
                state.dark_mode = loaded_state.dark_mode;
                // Don't overwrite history and other runtime data
            } else {
                // If settings file is corrupted, set default download location
                let mut state = self.state.lock().unwrap();
                state.download_location = Self::get_default_download_location();
            }
        } else {
            // If no settings file exists, set default download location
            let mut state = self.state.lock().unwrap();
            state.download_location = Self::get_default_download_location();
        }
    }
    
    fn apply_theme_on_startup(&self, ctx: &egui::Context) {
        let dark_mode = {
            let state = self.state.lock().unwrap();
            state.dark_mode
        };
        
        if dark_mode {
            ctx.set_visuals(egui::Visuals::dark());
        } else {
            ctx.set_visuals(egui::Visuals::light());
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

    fn fetch_user_info(&mut self) {
        // Set loading state
        *self.user_info_loading.lock().unwrap() = true;
        
        // Get API key with settings priority
        let api_key = self.get_api_key();

        if let Some(_key) = api_key {
            let client = self.make_api_client();
            let state = self.state.clone();
            let user_info_loading = self.user_info_loading.clone();

            thread::spawn(move || {
                let start_time = Instant::now();
                let mut last_error = None;

                for attempt in 1..=3 { // Retry up to 3 times
                    match client.get_user() {
                        Ok(user_info) => {
                            let duration = start_time.elapsed();
                            {
                                let mut state = state.lock().unwrap();
                                state.user_info = Some(user_info);
                                state.last_error = None;
                                state.debug_messages.push(format!("[{}] Successfully fetched user info in {:?} (attempt {})", 
                                    chrono::Utc::now().format("%H:%M:%S"), duration, attempt));
                                state.last_operation_time = Some(chrono::Utc::now());
                            }
                            *user_info_loading.lock().unwrap() = false;
                            break;
                        }
                        Err(e) => {
                            last_error = Some(e);
                            let should_retry = match &last_error.as_ref().unwrap() {
                                pixeldrain_api::PixelDrainError::Reqwest(reqwest_err) => {
                                    reqwest_err.is_timeout() || 
                                    reqwest_err.is_connect() || 
                                    reqwest_err.is_request() ||
                                    reqwest_err.to_string().contains("request or response body error")
                                }
                                pixeldrain_api::PixelDrainError::Api(api_err) => {
                                    api_err.status.is_server_error()
                                }
                                _ => false,
                            };
                            if should_retry && attempt < 3 {
                                std::thread::sleep(std::time::Duration::from_secs(3)); // Retry after 3 seconds
                            } else {
                                break;
                            }
                        }
                    }
                }

                if let Some(error_msg) = last_error {
                    let duration = start_time.elapsed();
                    let mut state = state.lock().unwrap();
                    state.last_error = Some(format!("Failed to fetch user info after {} attempts: {} (took {:?})", 
                        3, error_msg, duration));
                    state.debug_messages.push(format!("[{}] User info fetch failed after {} attempts: {} (took {:?})", 
                        chrono::Utc::now().format("%H:%M:%S"), 3, error_msg, duration));
                }
                *user_info_loading.lock().unwrap() = false;
            });
        } else {
            self.state.lock().unwrap().last_error = Some("No API key available (check settings or environment variable PIXELDRAIN_API_KEY). Cannot fetch user info.".to_string());
        }
    }
}

fn main() -> Result<(), eframe::Error> {
    env_logger::init();

    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([600.0, 400.0])
        .with_min_inner_size([400.0, 300.0]);
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
