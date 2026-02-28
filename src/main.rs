use chrono::Local;
use eframe::egui;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write as IoWrite;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use wmi::{COMLibrary, WMIConnection};

// ── WMI device struct ──────────────────────────────────────────

#[derive(Deserialize, Debug, Clone)]
#[allow(non_snake_case)]
struct UsbDevice {
    Name: Option<String>,
    DeviceID: Option<String>,
    Description: Option<String>,
    Manufacturer: Option<String>,
    PNPClass: Option<String>,
}

impl UsbDevice {
    fn display_name(&self) -> &str {
        self.Name
            .as_deref()
            .or(self.Description.as_deref())
            .unwrap_or("Unknown Device")
    }

    fn vid_pid(&self) -> Option<String> {
        let id = self.DeviceID.as_ref()?;
        let vid_start = id.find("VID_").map(|i| i + 4)?;
        let vid = id.get(vid_start..vid_start + 4)?;
        let pid_start = id.find("PID_").map(|i| i + 4)?;
        let pid = id.get(pid_start..pid_start + 4)?;
        Some(format!("{}:{}", vid, pid))
    }

    fn class(&self) -> &str {
        self.PNPClass.as_deref().unwrap_or("?")
    }
}

fn query_devices(wmi: &WMIConnection) -> Option<HashMap<String, UsbDevice>> {
    let results: Vec<UsbDevice> = wmi
        .raw_query(
            "SELECT Name, DeviceID, Description, Manufacturer, PNPClass \
             FROM Win32_PnPEntity WHERE DeviceID LIKE 'USB%'",
        )
        .ok()?;
    Some(
        results
            .into_iter()
            .filter_map(|d| Some((d.DeviceID.clone()?, d)))
            .collect(),
    )
}

// ── Shared state ───────────────────────────────────────────────

#[derive(Clone)]
struct DeviceEvent {
    timestamp: String,
    kind: EventKind,
    name: String,
    vid_pid: Option<String>,
    manufacturer: Option<String>,
    class: String,
    device_id: String,
}

#[derive(Clone, Copy, PartialEq)]
enum EventKind {
    Connect,
    Disconnect,
}

struct AppState {
    devices: Vec<(String, UsbDevice)>,
    events: Vec<DeviceEvent>,
    error: Option<String>,
}

fn log_to_file(msg: &str) {
    let path = "device-history.log";
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
        let ts = Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
        let _ = writeln!(f, "[{}] {}", ts, msg);
    }
}

// ── Background monitor thread ──────────────────────────────────

fn monitor_loop(state: Arc<Mutex<AppState>>) {
    let com = match COMLibrary::new() {
        Ok(c) => c,
        Err(e) => {
            if let Ok(mut s) = state.lock() {
                s.error = Some(format!("COM init failed: {}", e));
            }
            return;
        }
    };
    let wmi = match WMIConnection::new(com) {
        Ok(w) => w,
        Err(e) => {
            if let Ok(mut s) = state.lock() {
                s.error = Some(format!("WMI connect failed: {}", e));
            }
            return;
        }
    };

    let mut prev = match query_devices(&wmi) {
        Some(d) => d,
        None => {
            if let Ok(mut s) = state.lock() {
                s.error = Some("Failed to query USB devices".into());
            }
            return;
        }
    };

    if let Ok(mut s) = state.lock() {
        let mut sorted: Vec<_> = prev.iter().map(|(id, d)| (id.clone(), d.clone())).collect();
        sorted.sort_by(|a, b| {
            a.1.display_name()
                .to_lowercase()
                .cmp(&b.1.display_name().to_lowercase())
        });
        s.devices = sorted;
    }

    log_to_file(&format!("Started monitoring — {} devices", prev.len()));

    loop {
        thread::sleep(Duration::from_millis(500));

        let Some(current) = query_devices(&wmi) else {
            continue;
        };

        let mut new_events = Vec::new();
        let ts = Local::now().format("%H:%M:%S").to_string();

        for (id, dev) in &prev {
            if !current.contains_key(id) {
                let event = DeviceEvent {
                    timestamp: ts.clone(),
                    kind: EventKind::Disconnect,
                    name: dev.display_name().to_string(),
                    vid_pid: dev.vid_pid(),
                    manufacturer: dev.Manufacturer.clone(),
                    class: dev.class().to_string(),
                    device_id: id.clone(),
                };
                log_to_file(&format!(
                    "DISCONNECT: {} [{}] | {}",
                    event.name,
                    event.vid_pid.as_deref().unwrap_or("?"),
                    id
                ));
                new_events.push(event);
            }
        }

        for (id, dev) in &current {
            if !prev.contains_key(id) {
                let event = DeviceEvent {
                    timestamp: ts.clone(),
                    kind: EventKind::Connect,
                    name: dev.display_name().to_string(),
                    vid_pid: dev.vid_pid(),
                    manufacturer: dev.Manufacturer.clone(),
                    class: dev.class().to_string(),
                    device_id: id.clone(),
                };
                log_to_file(&format!(
                    "CONNECT: {} [{}] | {}",
                    event.name,
                    event.vid_pid.as_deref().unwrap_or("?"),
                    id
                ));
                new_events.push(event);
            }
        }

        if !new_events.is_empty() {
            if let Ok(mut s) = state.lock() {
                s.events.extend(new_events);
                let mut sorted: Vec<_> =
                    current.iter().map(|(id, d)| (id.clone(), d.clone())).collect();
                sorted.sort_by(|a, b| {
                    a.1.display_name()
                        .to_lowercase()
                        .cmp(&b.1.display_name().to_lowercase())
                });
                s.devices = sorted;
            }
        }

        prev = current;
    }
}

// ── Theme system ───────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum Theme {
    Neon,
    Light,
    Mids,
}

impl Theme {
    fn label(self) -> &'static str {
        match self {
            Theme::Neon => "Neon",
            Theme::Light => "Light",
            Theme::Mids => "Mids",
        }
    }

    fn colors(self) -> ThemeColors {
        match self {
            Theme::Neon => ThemeColors {
                bg_deep: c(0x0d, 0x0f, 0x14),
                bg_surface: c(0x1a, 0x1c, 0x23),
                bg_elevated: c(0x22, 0x25, 0x2e),
                border: c(0x2a, 0x2d, 0x3a),
                accent: c(0xa8, 0x55, 0xf7),
                orange: c(0xff, 0x8b, 0x3d),
                teal: c(0x2e, 0xe6, 0xd7),
                green: c(0x50, 0xfa, 0x7b),
                red: c(0xff, 0x55, 0x55),
                yellow: c(0xf1, 0xfa, 0x8c),
                pink: c(0xff, 0x79, 0xc6),
                cyan: c(0x8b, 0xe9, 0xfd),
                text: c(0xe8, 0xe8, 0xf0),
                text_sec: c(0x8c, 0x8c, 0xa0),
                text_muted: c(0x55, 0x57, 0x68),
                dark_mode: true,
            },
            Theme::Light => ThemeColors {
                bg_deep: c(0xf2, 0xf2, 0xf7),
                bg_surface: c(0xff, 0xff, 0xff),
                bg_elevated: c(0xe8, 0xe8, 0xf0),
                border: c(0xd0, 0xd0, 0xdd),
                accent: c(0x7c, 0x3a, 0xed),
                orange: c(0xea, 0x58, 0x0c),
                teal: c(0x0d, 0x94, 0x88),
                green: c(0x16, 0xa3, 0x4a),
                red: c(0xdc, 0x26, 0x26),
                yellow: c(0xa1, 0x6b, 0x07),
                pink: c(0xdb, 0x27, 0x77),
                cyan: c(0x06, 0x7a, 0x99),
                text: c(0x1a, 0x1a, 0x2e),
                text_sec: c(0x64, 0x74, 0x8b),
                text_muted: c(0x94, 0xa3, 0xb8),
                dark_mode: false,
            },
            Theme::Mids => ThemeColors {
                bg_deep: c(0x2a, 0x2c, 0x35),
                bg_surface: c(0x36, 0x38, 0x44),
                bg_elevated: c(0x42, 0x44, 0x52),
                border: c(0x52, 0x54, 0x66),
                accent: c(0x9b, 0x7d, 0xff),
                orange: c(0xff, 0x9f, 0x5a),
                teal: c(0x4d, 0xd8, 0xcc),
                green: c(0x6b, 0xe8, 0x8a),
                red: c(0xff, 0x6e, 0x6e),
                yellow: c(0xe8, 0xd4, 0x6e),
                pink: c(0xff, 0x8f, 0xd0),
                cyan: c(0x7d, 0xcc, 0xe8),
                text: c(0xd8, 0xd8, 0xe4),
                text_sec: c(0x98, 0x98, 0xac),
                text_muted: c(0x6e, 0x70, 0x82),
                dark_mode: true,
            },
        }
    }
}

const fn c(r: u8, g: u8, b: u8) -> egui::Color32 {
    egui::Color32::from_rgb(r, g, b)
}

#[derive(Clone, Copy)]
struct ThemeColors {
    bg_deep: egui::Color32,
    bg_surface: egui::Color32,
    bg_elevated: egui::Color32,
    border: egui::Color32,
    accent: egui::Color32,
    orange: egui::Color32,
    teal: egui::Color32,
    green: egui::Color32,
    red: egui::Color32,
    yellow: egui::Color32,
    pink: egui::Color32,
    cyan: egui::Color32,
    text: egui::Color32,
    text_sec: egui::Color32,
    text_muted: egui::Color32,
    dark_mode: bool,
}

// ── Helpers ────────────────────────────────────────────────────

fn blend(base: egui::Color32, target: egui::Color32, t: f32) -> egui::Color32 {
    let m = |a: u8, b: u8| (a as f32 * (1.0 - t) + b as f32 * t).clamp(0.0, 255.0) as u8;
    egui::Color32::from_rgb(m(base.r(), target.r()), m(base.g(), target.g()), m(base.b(), target.b()))
}

fn load_icon() -> Option<egui::IconData> {
    let png_bytes = include_bytes!("../assets/icon.png");
    let img = image::load_from_memory(png_bytes).ok()?.into_rgba8();
    let (w, h) = img.dimensions();
    Some(egui::IconData {
        rgba: img.into_raw(),
        width: w,
        height: h,
    })
}

fn apply_theme(ctx: &egui::Context, tc: &ThemeColors) {
    ctx.set_visuals({
        let mut v = if tc.dark_mode {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };

        v.panel_fill = tc.bg_deep;
        v.window_fill = tc.bg_surface;
        v.extreme_bg_color = tc.bg_deep;
        v.faint_bg_color = tc.bg_elevated;

        v.widgets.noninteractive.bg_fill = tc.bg_surface;
        v.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, tc.text_sec);
        v.widgets.noninteractive.bg_stroke = egui::Stroke::new(0.5, tc.border);
        v.widgets.noninteractive.rounding = egui::Rounding::same(4.0);

        v.widgets.inactive.bg_fill = tc.bg_elevated;
        v.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, tc.text);
        v.widgets.inactive.bg_stroke = egui::Stroke::new(0.5, tc.border);
        v.widgets.inactive.rounding = egui::Rounding::same(4.0);

        v.widgets.hovered.bg_fill = blend(tc.bg_elevated, tc.accent, 0.15);
        v.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, tc.text);
        v.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, tc.accent);
        v.widgets.hovered.rounding = egui::Rounding::same(4.0);

        v.widgets.active.bg_fill = blend(tc.bg_elevated, tc.accent, 0.25);
        v.widgets.active.fg_stroke = egui::Stroke::new(1.0, tc.text);
        v.widgets.active.bg_stroke = egui::Stroke::new(1.5, tc.accent);
        v.widgets.active.rounding = egui::Rounding::same(4.0);

        v.selection.bg_fill = blend(tc.bg_surface, tc.accent, 0.2);
        v.selection.stroke = egui::Stroke::new(1.0, tc.accent);

        v.window_rounding = egui::Rounding::same(6.0);
        v.window_shadow = egui::Shadow {
            offset: egui::Vec2::new(0.0, 2.0),
            blur: 8.0,
            spread: 0.0,
            color: egui::Color32::from_black_alpha(if tc.dark_mode { 80 } else { 30 }),
        };

        v.collapsing_header_frame = true;
        v.override_text_color = Some(tc.text);

        v
    });
}

fn draw_rainbow_separator(ui: &mut egui::Ui, tc: &ThemeColors) {
    let colors = [tc.red, tc.orange, tc.yellow, tc.green, tc.teal, tc.cyan, tc.accent, tc.pink];
    let w = ui.available_width();
    let h = 2.0;
    let (rect, _) = ui.allocate_exact_size(egui::Vec2::new(w, h), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    let seg_w = w / colors.len() as f32;

    for i in 0..colors.len() {
        let c0 = colors[i];
        let c1 = colors[(i + 1) % colors.len()];
        let subs = 8;
        let sub_w = seg_w / subs as f32;
        for s in 0..subs {
            let t = s as f32 / subs as f32;
            let color = blend(c0, c1, t);
            let x = rect.left() + i as f32 * seg_w + s as f32 * sub_w;
            painter.rect_filled(
                egui::Rect::from_min_size(egui::Pos2::new(x, rect.top()), egui::Vec2::new(sub_w + 0.5, h)),
                0.0,
                color,
            );
        }
    }
}

// ── GUI ────────────────────────────────────────────────────────

struct DeviceHistoryApp {
    state: Arc<Mutex<AppState>>,
    theme: Theme,
    colors: ThemeColors,
    needs_theme_apply: bool,
    show_about: bool,
    update_available: Arc<Mutex<Option<String>>>,
}

impl DeviceHistoryApp {
    fn new(state: Arc<Mutex<AppState>>) -> Self {
        let theme = Theme::Neon;
        let update_available = Arc::new(Mutex::new(None));

        // Background update check
        let update_flag = update_available.clone();
        thread::spawn(move || {
            let current = env!("CARGO_PKG_VERSION");
            let resp = ureq::get("https://api.github.com/repos/TrentSterling/device-history/releases/latest")
                .set("User-Agent", "device-history")
                .call();
            if let Ok(resp) = resp {
                if let Ok(body) = resp.into_string() {
                    // Simple JSON parse for "tag_name":"vX.Y.Z"
                    if let Some(start) = body.find("\"tag_name\"") {
                        let rest = &body[start..];
                        if let Some(colon) = rest.find(':') {
                            let after_colon = rest[colon + 1..].trim_start();
                            if after_colon.starts_with('"') {
                                let val_start = 1;
                                if let Some(val_end) = after_colon[val_start..].find('"') {
                                    let tag = &after_colon[val_start..val_start + val_end];
                                    let latest = tag.trim_start_matches('v');
                                    if latest != current {
                                        if let Ok(mut u) = update_flag.lock() {
                                            *u = Some(latest.to_string());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });

        Self {
            state,
            theme,
            colors: theme.colors(),
            needs_theme_apply: true,
            show_about: false,
            update_available,
        }
    }
}

impl eframe::App for DeviceHistoryApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.needs_theme_apply {
            apply_theme(ctx, &self.colors);
            self.needs_theme_apply = false;
        }

        ctx.request_repaint_after(Duration::from_millis(250));

        let tc = self.colors;
        let state = self.state.lock().unwrap();

        // ── Header ──
        egui::TopBottomPanel::top("header")
            .frame(
                egui::Frame::none()
                    .fill(tc.bg_surface)
                    .inner_margin(egui::Margin::symmetric(14.0, 10.0))
                    .stroke(egui::Stroke::new(1.0, tc.border)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Device History")
                            .strong()
                            .size(20.0)
                            .color(tc.accent),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(format!("v{}", env!("CARGO_PKG_VERSION")))
                            .size(11.0)
                            .color(tc.text_muted),
                    );

                    // Update available banner
                    if let Ok(guard) = self.update_available.lock() {
                        if let Some(ver) = guard.as_ref() {
                            ui.add_space(8.0);
                            let btn = egui::Button::new(
                                egui::RichText::new(format!("Update: v{}", ver))
                                    .size(11.0)
                                    .color(tc.orange),
                            )
                            .fill(egui::Color32::TRANSPARENT)
                            .stroke(egui::Stroke::new(1.0, tc.orange))
                            .rounding(4.0);
                            if ui.add(btn).clicked() {
                                let _ = open::that("https://github.com/TrentSterling/device-history/releases/latest");
                            }
                        }
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Status (rightmost)
                        if let Some(err) = &state.error {
                            ui.label(egui::RichText::new(err).color(tc.pink).size(13.0));
                        } else {
                            ui.label(
                                egui::RichText::new(format!("Monitoring {} devices", state.devices.len()))
                                    .color(tc.teal)
                                    .size(13.0),
                            );
                        }

                        ui.add_space(12.0);
                        ui.separator();
                        ui.add_space(4.0);

                        // Theme picker
                        for t in [Theme::Neon, Theme::Light, Theme::Mids] {
                            let selected = self.theme == t;
                            let label_color = if selected { tc.accent } else { tc.text_sec };
                            let btn = egui::Button::new(
                                egui::RichText::new(t.label()).size(11.0).color(label_color),
                            )
                            .fill(if selected { blend(tc.bg_elevated, tc.accent, 0.12) } else { egui::Color32::TRANSPARENT })
                            .stroke(if selected {
                                egui::Stroke::new(1.0, tc.accent)
                            } else {
                                egui::Stroke::new(0.5, tc.border)
                            })
                            .rounding(3.0);

                            if ui.add(btn).clicked() && !selected {
                                self.theme = t;
                                self.colors = t.colors();
                                self.needs_theme_apply = true;
                            }
                        }
                    });
                });
            });

        // ── Footer ──
        egui::TopBottomPanel::bottom("footer")
            .frame(
                egui::Frame::none()
                    .fill(tc.bg_surface)
                    .inner_margin(egui::Margin::symmetric(14.0, 7.0))
                    .stroke(egui::Stroke::new(1.0, tc.border)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("{} events logged", state.events.len()))
                            .color(tc.text_muted)
                            .size(12.0),
                    );
                    ui.label(egui::RichText::new("\u{2022}").color(tc.accent).size(10.0));
                    ui.label(
                        egui::RichText::new("Log: device-history.log")
                            .color(tc.text_muted)
                            .size(12.0),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(egui::RichText::new("tront.xyz").color(tc.text_muted).size(11.0));
                    });
                });
            });

        // Clone and drop lock before central panel
        let events = state.events.clone();
        let devices = state.devices.clone();
        drop(state);

        // ── Central panel ──
        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(tc.bg_deep)
                    .inner_margin(egui::Margin::symmetric(14.0, 10.0)),
            )
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing.y = 4.0;

                // ── About section ──
                let about_id = ui.make_persistent_id("about_section");
                let about_state = egui::collapsing_header::CollapsingState::load_with_default_open(
                    ctx, about_id, self.show_about,
                );
                about_state
                    .show_header(ui, |ui| {
                        ui.label(
                            egui::RichText::new("About")
                                .size(13.0)
                                .color(tc.text_sec),
                        );
                    })
                    .body(|ui| {
                        let about_frame = egui::Frame::none()
                            .fill(tc.bg_surface)
                            .rounding(6.0)
                            .stroke(egui::Stroke::new(0.5, tc.border))
                            .inner_margin(egui::Margin::same(12.0));

                        about_frame.show(ui, |ui: &mut egui::Ui| {
                            ui.spacing_mut().item_spacing.y = 6.0;

                            ui.label(
                                egui::RichText::new("WTF just disconnected?")
                                    .size(16.0)
                                    .strong()
                                    .color(tc.accent),
                            );
                            ui.add_space(2.0);
                            ui.label(
                                egui::RichText::new(
                                    "Real-time USB device monitor for Windows. Watches for \
                                     connect/disconnect events via WMI polling and logs everything \
                                     to device-history.log with timestamps.",
                                )
                                .size(13.0)
                                .color(tc.text_sec),
                            );
                            ui.add_space(4.0);

                            draw_rainbow_separator(ui, &tc);

                            ui.add_space(4.0);

                            let features = [
                                (tc.green, "Live monitoring", "500ms poll interval, instant detection"),
                                (tc.cyan, "Event log", "Timestamped connect/disconnect history"),
                                (tc.yellow, "Device details", "VID:PID, class, manufacturer for every device"),
                                (tc.orange, "File logging", "Persistent log at device-history.log"),
                                (tc.accent, "CLI mode", "Run with --cli for terminal output"),
                            ];

                            for (color, title, desc) in &features {
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new("\u{2022}").color(*color).size(12.0));
                                    ui.label(egui::RichText::new(*title).color(tc.text).strong().size(12.0));
                                    ui.label(egui::RichText::new(*desc).color(tc.text_sec).size(12.0));
                                });
                            }
                        });
                    });

                ui.add_space(6.0);

                // ── Event Log section ──
                let events_empty = events.is_empty();
                let half_height = (ui.available_height() - 30.0) * 0.45;

                // Event log header
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("Event Log ({})", events.len()))
                            .size(15.0)
                            .color(tc.cyan)
                            .strong(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let clear_btn = egui::Button::new(
                            egui::RichText::new("Clear").color(tc.orange).size(12.0),
                        )
                        .stroke(egui::Stroke::new(1.0, tc.orange))
                        .fill(egui::Color32::TRANSPARENT)
                        .rounding(4.0);

                        if ui.add(clear_btn).clicked() {
                            if let Ok(mut s) = self.state.lock() {
                                s.events.clear();
                            }
                        }
                    });
                });

                ui.add_space(4.0);

                // Event log box
                egui::Frame::none()
                    .fill(tc.bg_surface)
                    .rounding(6.0)
                    .stroke(egui::Stroke::new(1.0, tc.border))
                    .inner_margin(egui::Margin::same(6.0))
                    .show(ui, |ui: &mut egui::Ui| {
                        ui.set_width(ui.available_width());
                        egui::ScrollArea::vertical()
                            .id_salt("event_log")
                            .max_height(if events_empty { 60.0 } else { half_height.max(80.0) })
                            .stick_to_bottom(true)
                            .show(ui, |ui| {
                                ui.spacing_mut().item_spacing.y = 3.0;

                                if events_empty {
                                    ui.add_space(16.0);
                                    ui.vertical_centered(|ui| {
                                        ui.label(
                                            egui::RichText::new("No events yet — waiting for USB changes...")
                                                .color(tc.text_sec)
                                                .italics()
                                                .size(13.0),
                                        );
                                    });
                                } else {
                                    for event in &events {
                                        let (accent, icon, label) = match event.kind {
                                            EventKind::Connect => (tc.green, "\u{25B2}", "CONNECT"),
                                            EventKind::Disconnect => (tc.red, "\u{25BC}", "DISCONNECT"),
                                        };

                                        let card_fill = blend(tc.bg_elevated, accent, 0.05);

                                        egui::Frame::none()
                                            .fill(card_fill)
                                            .rounding(4.0)
                                            .stroke(egui::Stroke::new(0.5, tc.border))
                                            .inner_margin(egui::Margin {
                                                left: 10.0,
                                                right: 8.0,
                                                top: 4.0,
                                                bottom: 4.0,
                                            })
                                            .show(ui, |ui: &mut egui::Ui| {
                                                ui.set_width(ui.available_width());
                                                let rect = ui.max_rect();

                                                // Left accent bar
                                                ui.painter().rect_filled(
                                                    egui::Rect::from_min_size(
                                                        rect.left_top(),
                                                        egui::Vec2::new(3.0, rect.height()),
                                                    ),
                                                    egui::Rounding { nw: 4.0, sw: 4.0, ne: 0.0, se: 0.0 },
                                                    accent,
                                                );

                                                ui.horizontal(|ui| {
                                                    ui.spacing_mut().item_spacing.x = 6.0;

                                                    ui.label(
                                                        egui::RichText::new(&event.timestamp)
                                                            .color(tc.text_sec)
                                                            .monospace()
                                                            .size(12.0),
                                                    );
                                                    ui.label(
                                                        egui::RichText::new(format!("{} {}", icon, label))
                                                            .color(accent)
                                                            .strong()
                                                            .monospace()
                                                            .size(12.0),
                                                    );
                                                    ui.add(
                                                        egui::Label::new(
                                                            egui::RichText::new(&event.name)
                                                                .color(tc.text)
                                                                .size(12.0),
                                                        )
                                                        .truncate(),
                                                    );
                                                    if let Some(vp) = &event.vid_pid {
                                                        ui.label(
                                                            egui::RichText::new(format!("[{}]", vp))
                                                                .color(tc.yellow)
                                                                .monospace()
                                                                .size(11.0),
                                                        );
                                                    }
                                                    ui.label(
                                                        egui::RichText::new(&event.class)
                                                            .color(tc.accent)
                                                            .monospace()
                                                            .size(10.0),
                                                    );
                                                    if let Some(mfr) = &event.manufacturer {
                                                        ui.add(
                                                            egui::Label::new(
                                                                egui::RichText::new(format!("({})", mfr))
                                                                    .color(tc.text_sec)
                                                                    .size(11.0),
                                                            )
                                                            .truncate(),
                                                        );
                                                    }
                                                });
                                            });
                                    }
                                }
                            });
                    });

                ui.add_space(8.0);
                draw_rainbow_separator(ui, &tc);
                ui.add_space(6.0);

                // ── Connected Devices section ──
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("Connected Devices ({})", devices.len()))
                            .size(15.0)
                            .color(tc.cyan)
                            .strong(),
                    );
                });

                ui.add_space(4.0);

                // Device list box — fills remaining space
                egui::Frame::none()
                    .fill(tc.bg_surface)
                    .rounding(6.0)
                    .stroke(egui::Stroke::new(1.0, tc.border))
                    .inner_margin(egui::Margin::same(6.0))
                    .show(ui, |ui: &mut egui::Ui| {
                        ui.set_width(ui.available_width());
                        let remaining = ui.available_height().max(60.0);
                        egui::ScrollArea::vertical()
                            .id_salt("devices_list")
                            .max_height(remaining)
                            .show(ui, |ui| {
                                ui.spacing_mut().item_spacing.y = 2.0;

                                for (_id, dev) in &devices {
                                    egui::Frame::none()
                                        .fill(tc.bg_elevated)
                                        .rounding(4.0)
                                        .stroke(egui::Stroke::new(0.5, tc.border))
                                        .inner_margin(egui::Margin::symmetric(10.0, 4.0))
                                        .show(ui, |ui: &mut egui::Ui| {
                                            ui.set_width(ui.available_width());
                                            ui.horizontal(|ui| {
                                                ui.spacing_mut().item_spacing.x = 6.0;

                                                ui.label(
                                                    egui::RichText::new(dev.class())
                                                        .color(tc.accent)
                                                        .monospace()
                                                        .size(11.0),
                                                );
                                                ui.add(
                                                    egui::Label::new(
                                                        egui::RichText::new(dev.display_name())
                                                            .color(tc.text)
                                                            .size(12.0),
                                                    )
                                                    .truncate(),
                                                );
                                                if let Some(vp) = dev.vid_pid() {
                                                    ui.label(
                                                        egui::RichText::new(format!("[{}]", vp))
                                                            .color(tc.yellow)
                                                            .monospace()
                                                            .size(10.0),
                                                    );
                                                }
                                                if let Some(mfr) = &dev.Manufacturer {
                                                    ui.add(
                                                        egui::Label::new(
                                                            egui::RichText::new(mfr)
                                                                .color(tc.text_sec)
                                                                .size(10.0),
                                                        )
                                                        .truncate(),
                                                    );
                                                }
                                            });
                                        });
                                }
                            });
                    });
            });
    }
}

// ── CLI mode ───────────────────────────────────────────────────

fn run_cli() {
    use colored::*;

    let ver = env!("CARGO_PKG_VERSION");
    let title = format!("Device History v{}", ver);
    let tagline = "WTF just disconnected?";
    let width = 39;
    println!(
        "{}",
        format!("\u{2554}{}\u{2557}", "\u{2550}".repeat(width)).bright_cyan()
    );
    println!(
        "{}",
        format!("\u{2551}{:^w$}\u{2551}", title, w = width).bright_cyan()
    );
    println!(
        "{}",
        format!("\u{2551}{:^w$}\u{2551}", tagline, w = width).bright_cyan()
    );
    println!(
        "{}",
        format!("\u{255a}{}\u{255d}", "\u{2550}".repeat(width)).bright_cyan()
    );
    println!();

    let com = COMLibrary::new().expect("Failed to initialize COM library");
    let wmi = WMIConnection::new(com).expect("Failed to connect to WMI");
    let mut devices = query_devices(&wmi).expect("Failed to query USB devices");

    println!(
        "{} {} USB devices currently connected:\n",
        "\u{25CF}".green(),
        devices.len().to_string().bold()
    );

    let mut sorted: Vec<_> = devices.values().collect();
    sorted.sort_by_key(|d| d.display_name().to_lowercase());
    for dev in &sorted {
        let vid_pid = dev
            .vid_pid()
            .map(|vp| format!(" [{}]", vp))
            .unwrap_or_default();
        let mfr = dev
            .Manufacturer
            .as_deref()
            .map(|m| format!(" ({})", m))
            .unwrap_or_default();
        println!(
            "  {} {} {}{}{}",
            "\u{2022}".dimmed(),
            dev.class().dimmed(),
            dev.display_name(),
            vid_pid.dimmed(),
            mfr.dimmed()
        );
    }

    log_to_file(&format!(
        "Started monitoring (CLI) — {} devices",
        devices.len()
    ));
    println!("\n{}", "Watching for changes... (Ctrl+C to quit)".dimmed());
    println!("{}\n", "\u{2500}".repeat(60).dimmed());

    loop {
        thread::sleep(Duration::from_millis(500));
        let Some(current) = query_devices(&wmi) else {
            continue;
        };

        for (id, dev) in &devices {
            if !current.contains_key(id) {
                let ts = Local::now().format("%H:%M:%S").to_string();
                let vp = dev
                    .vid_pid()
                    .map(|v| format!(" [{}]", v))
                    .unwrap_or_default();
                println!(
                    "{} {} {} {}",
                    format!("[{}]", ts).dimmed(),
                    "\u{25BC} DISCONNECT".red().bold(),
                    dev.display_name().red(),
                    vp.yellow()
                );
                log_to_file(&format!(
                    "DISCONNECT: {} {} | {}",
                    dev.display_name(),
                    vp,
                    id
                ));
            }
        }
        for (id, dev) in &current {
            if !devices.contains_key(id) {
                let ts = Local::now().format("%H:%M:%S").to_string();
                let vp = dev
                    .vid_pid()
                    .map(|v| format!(" [{}]", v))
                    .unwrap_or_default();
                println!(
                    "{} {} {} {}",
                    format!("[{}]", ts).dimmed(),
                    "\u{25B2} CONNECT   ".green().bold(),
                    dev.display_name().green(),
                    vp.yellow()
                );
                log_to_file(&format!(
                    "CONNECT: {} {} | {}",
                    dev.display_name(),
                    vp,
                    id
                ));
            }
        }
        devices = current;
    }
}

// ── Entry point ────────────────────────────────────────────────

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|a| a == "--cli") {
        run_cli();
        return;
    }

    let state = Arc::new(Mutex::new(AppState {
        devices: Vec::new(),
        events: Vec::new(),
        error: None,
    }));

    let state_bg = state.clone();
    thread::spawn(move || monitor_loop(state_bg));

    let icon = load_icon();

    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([720.0, 620.0])
        .with_min_inner_size([420.0, 340.0])
        .with_title("Device History");

    if let Some(icon_data) = icon {
        viewport = viewport.with_icon(std::sync::Arc::new(icon_data));
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    if let Err(e) = eframe::run_native(
        "Device History",
        options,
        Box::new(move |_cc| Ok(Box::new(DeviceHistoryApp::new(state)))),
    ) {
        eprintln!("GUI error: {}", e);
    }
}
