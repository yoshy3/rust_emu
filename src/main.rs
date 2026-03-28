use anyhow::{Error, Result};
use chrono::{Local, TimeZone};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use egui::{self, ColorImage, RichText, TextureHandle};
use egui_wgpu::{renderer::ScreenDescriptor, Renderer as EguiRenderer};
use egui_winit::State as EguiWinitState;
use gilrs::{Axis, Button, EventType, GamepadId, Gilrs};
use log::error;
use pixels::{Pixels, SurfaceTexture};
use rust_emu::joypad::JoypadButton;
use rust_emu::NesSnapshot;
use std::collections::{HashMap, HashSet, VecDeque};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use winit::dpi::LogicalSize;
use winit::event::{Event, VirtualKeyCode};
use winit::event_loop::EventLoop;
use winit::window::WindowBuilder;
use winit_input_helper::WinitInputHelper;

/// Write a stereo WAV file (IEEE float 32-bit)
/// Both channels carry the emulator output sample for easy comparison.
fn write_wav_file(path: &str, sample_rate: u32, samples: &[(f32, f32)]) -> std::io::Result<()> {
    let mut f = std::fs::File::create(path)?;
    let num_channels: u16 = 2;
    let bits_per_sample: u16 = 32;
    let byte_rate = sample_rate * num_channels as u32 * bits_per_sample as u32 / 8;
    let block_align = num_channels * bits_per_sample / 8;
    let data_size = samples.len() as u32 * block_align as u32;
    let file_size = 36 + data_size;

    // RIFF header
    f.write_all(b"RIFF")?;
    f.write_all(&file_size.to_le_bytes())?;
    f.write_all(b"WAVE")?;

    // fmt chunk (IEEE float = format tag 3)
    f.write_all(b"fmt ")?;
    f.write_all(&16u32.to_le_bytes())?;       // chunk size
    f.write_all(&3u16.to_le_bytes())?;        // format: IEEE float
    f.write_all(&num_channels.to_le_bytes())?;
    f.write_all(&sample_rate.to_le_bytes())?;
    f.write_all(&byte_rate.to_le_bytes())?;
    f.write_all(&block_align.to_le_bytes())?;
    f.write_all(&bits_per_sample.to_le_bytes())?;

    // data chunk
    f.write_all(b"data")?;
    f.write_all(&data_size.to_le_bytes())?;
    for &(left, right) in samples {
        f.write_all(&left.to_le_bytes())?;
        f.write_all(&right.to_le_bytes())?;
    }

    println!("[WAV] Wrote {} samples ({:.1}s) to {}",
        samples.len(),
        samples.len() as f64 / sample_rate as f64,
        path);
    Ok(())
}

const _SAMPLE_RATE: u32 = 44100;

const WIDTH: u32 = 256;
const HEIGHT: u32 = 240;
const GAMEPAD_AXIS_THRESHOLD: f32 = 0.5;
const SAVE_SLOT_COLUMNS: usize = 4;
const SAVE_SLOT_ROWS: usize = 2;
const SAVE_SLOT_COUNT: usize = SAVE_SLOT_COLUMNS * SAVE_SLOT_ROWS;
const THUMBNAIL_WIDTH: usize = 128;
const THUMBNAIL_HEIGHT: usize = 120;
const MENU_COMBO_GRACE_PERIOD: Duration = Duration::from_millis(250);
const XBOX_MAC_BUTTON_A_CODE: u32 = (9 << 16) | 1;
const XBOX_MAC_BUTTON_X_CODE: u32 = (9 << 16) | 4;
const XBOX_MAC_BUTTON_BACK_CODE: u32 = (9 << 16) | 11;
const XBOX_MAC_BUTTON_START_CODE: u32 = (9 << 16) | 12;
const XBOX_MAC_BUTTON_MODE_CODE: u32 = (9 << 16) | 13;
const XBOX_MAC_AXIS_LEFT_X_CODE: u32 = (1 << 16) | 48;
const XBOX_MAC_AXIS_LEFT_Y_CODE: u32 = (1 << 16) | 49;
const XBOX_MAC_AXIS_RT_CODE: u32 = 131_268;
const XBOX_MAC_AXIS_DPAD_X_CODE: u32 = (1 << 16) | 57;
const XBOX_MAC_AXIS_LT_CODE: u32 = 131_269;
const XBOX_MAC_AXIS_DPAD_Y_CODE: u32 = (1 << 16) | 58;

#[derive(Clone, Copy)]
enum GamepadProfile {
    Default,
    XboxWirelessMac,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct PersistedStateSlot {
    saved_at_unix: i64,
    thumbnail_rgba: Vec<u8>,
    snapshot: NesSnapshot,
}

struct SaveSlotPreview {
    saved_at_unix: i64,
    thumbnail_rgba: Vec<u8>,
    texture: Option<TextureHandle>,
}

struct SaveMenu {
    visible: bool,
    selected_slot: usize,
    confirm_overwrite: bool,
    confirm_yes_selected: bool,
    close_after_load_release: bool,
    status_message: String,
    save_dir: PathBuf,
    slots: Vec<Option<SaveSlotPreview>>,
}

impl SaveMenu {
    fn new(save_dir: PathBuf) -> Self {
        let mut menu = Self {
            visible: false,
            selected_slot: 0,
            confirm_overwrite: false,
            confirm_yes_selected: false,
            close_after_load_release: false,
            status_message: String::new(),
            save_dir,
            slots: (0..SAVE_SLOT_COUNT).map(|_| None).collect(),
        };
        menu.refresh_slots();
        menu
    }

    fn slot_path(&self, slot_index: usize) -> PathBuf {
        self.save_dir.join(format!("slot{}.bin", slot_index + 1))
    }

    fn refresh_slots(&mut self) {
        self.slots = (0..SAVE_SLOT_COUNT)
            .map(|slot_index| {
                let path = self.slot_path(slot_index);
                std::fs::read(&path)
                    .ok()
                    .and_then(|bytes| bincode::deserialize::<PersistedStateSlot>(&bytes).ok())
                    .map(|slot| SaveSlotPreview {
                        saved_at_unix: slot.saved_at_unix,
                        thumbnail_rgba: normalize_thumbnail_rgba(slot.thumbnail_rgba),
                        texture: None,
                    })
            })
            .collect();
    }

    fn open(&mut self) {
        self.visible = true;
        self.confirm_overwrite = false;
        self.confirm_yes_selected = false;
        self.close_after_load_release = false;
        self.status_message.clear();
        self.refresh_slots();
    }

    fn close(&mut self) {
        self.visible = false;
        self.confirm_overwrite = false;
        self.confirm_yes_selected = false;
        self.close_after_load_release = false;
        self.status_message.clear();
    }

    fn move_selection(&mut self, row_delta: isize, col_delta: isize) {
        let row = self.selected_slot / SAVE_SLOT_COLUMNS;
        let col = self.selected_slot % SAVE_SLOT_COLUMNS;
        let next_row = ((row as isize + row_delta).rem_euclid(SAVE_SLOT_ROWS as isize)) as usize;
        let next_col =
            ((col as isize + col_delta).rem_euclid(SAVE_SLOT_COLUMNS as isize)) as usize;
        self.selected_slot = next_row * SAVE_SLOT_COLUMNS + next_col;
        self.confirm_overwrite = false;
        self.confirm_yes_selected = false;
        self.close_after_load_release = false;
        self.status_message.clear();
    }

    fn save_selected(&mut self, nes: &rust_emu::Nes) -> Result<()> {
        if self.slots[self.selected_slot].is_some() && !self.confirm_overwrite {
            self.confirm_overwrite = true;
            self.confirm_yes_selected = false;
            self.status_message = "OVERWRITE THIS SLOT?".to_string();
            return Ok(());
        }

        if self.confirm_overwrite && !self.confirm_yes_selected {
            self.confirm_overwrite = false;
            self.status_message = "SAVE CANCELED".to_string();
            return Ok(());
        }

        std::fs::create_dir_all(&self.save_dir)?;
        let slot = PersistedStateSlot {
            saved_at_unix: Local::now().timestamp(),
            thumbnail_rgba: capture_thumbnail(&nes.bus.ppu.frame_buffer),
            snapshot: nes.save_state(),
        };
        let bytes = bincode::serialize(&slot).map_err(Error::msg)?;
        std::fs::write(self.slot_path(self.selected_slot), bytes)?;
        self.confirm_overwrite = false;
        self.confirm_yes_selected = false;
        self.close_after_load_release = false;
        self.status_message = "STATE SAVED".to_string();
        self.refresh_slots();
        Ok(())
    }

    fn load_selected(&mut self) -> Result<Option<NesSnapshot>> {
        let path = self.slot_path(self.selected_slot);
        if !path.exists() {
            self.status_message = "EMPTY SLOT".to_string();
            return Ok(None);
        }

        let bytes = std::fs::read(path)?;
        let slot: PersistedStateSlot = bincode::deserialize(&bytes).map_err(Error::msg)?;
        self.confirm_overwrite = false;
        self.confirm_yes_selected = false;
        self.close_after_load_release = false;
        self.status_message = "STATE LOADED".to_string();
        Ok(Some(slot.snapshot))
    }
}

fn capture_thumbnail(frame: &[u8]) -> Vec<u8> {
    let mut thumbnail = vec![0; THUMBNAIL_WIDTH * THUMBNAIL_HEIGHT * 4];
    for y in 0..THUMBNAIL_HEIGHT {
        for x in 0..THUMBNAIL_WIDTH {
            let src_x = x * WIDTH as usize / THUMBNAIL_WIDTH;
            let src_y = y * HEIGHT as usize / THUMBNAIL_HEIGHT;
            let src_idx = (src_y * WIDTH as usize + src_x) * 4;
            let dst_idx = (y * THUMBNAIL_WIDTH + x) * 4;
            thumbnail[dst_idx..dst_idx + 4].copy_from_slice(&frame[src_idx..src_idx + 4]);
        }
    }
    thumbnail
}

fn normalize_thumbnail_rgba(thumbnail_rgba: Vec<u8>) -> Vec<u8> {
    let current_len = THUMBNAIL_WIDTH * THUMBNAIL_HEIGHT * 4;
    if thumbnail_rgba.len() == current_len {
        return thumbnail_rgba;
    }

    let legacy_width = 64usize;
    let legacy_height = 60usize;
    let legacy_len = legacy_width * legacy_height * 4;
    if thumbnail_rgba.len() == legacy_len {
        let mut upscaled = vec![0u8; current_len];
        for y in 0..THUMBNAIL_HEIGHT {
            for x in 0..THUMBNAIL_WIDTH {
                let src_x = x * legacy_width / THUMBNAIL_WIDTH;
                let src_y = y * legacy_height / THUMBNAIL_HEIGHT;
                let src_idx = (src_y * legacy_width + src_x) * 4;
                let dst_idx = (y * THUMBNAIL_WIDTH + x) * 4;
                upscaled[dst_idx..dst_idx + 4]
                    .copy_from_slice(&thumbnail_rgba[src_idx..src_idx + 4]);
            }
        }
        return upscaled;
    }

    vec![0u8; current_len]
}

fn format_saved_at(saved_at_unix: i64) -> String {
    Local
        .timestamp_opt(saved_at_unix, 0)
        .single()
        .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|| "UNKNOWN".to_string())
}

fn lerp_color(a: egui::Color32, b: egui::Color32, t: f32) -> egui::Color32 {
    let t = t.clamp(0.0, 1.0);
    let lerp = |start: u8, end: u8| -> u8 {
        egui::lerp(start as f32..=end as f32, t).round() as u8
    };
    egui::Color32::from_rgba_unmultiplied(
        lerp(a.r(), b.r()),
        lerp(a.g(), b.g()),
        lerp(a.b(), b.b()),
        lerp(a.a(), b.a()),
    )
}

fn show_save_menu_egui(ctx: &egui::Context, menu: &mut SaveMenu) {
    let screen_rect = ctx.input(|input| input.screen_rect());
    let pixels_per_point = ctx.pixels_per_point().max(1.0);
    let thumbnail_size = egui::vec2(256.0 / pixels_per_point, 180.0 / pixels_per_point);
    let window_size = egui::vec2(
        (screen_rect.width() - 48.0).min(940.0),
        (screen_rect.height() - 48.0).min(740.0),
    );

    egui::Area::new("save_menu_scrim")
        .order(egui::Order::Foreground)
        .fixed_pos(egui::pos2(0.0, 0.0))
        .show(ctx, |ui| {
            let rect = ui.max_rect();
            ui.painter().rect_filled(
                rect,
                0.0,
                egui::Color32::from_rgba_unmultiplied(18, 24, 38, 170),
            );
        });

    egui::Window::new("Save / Rewind")
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .collapsible(false)
        .resizable(false)
        .title_bar(true)
        .fixed_size(window_size)
        .frame(
            egui::Frame::window(&ctx.style())
                .fill(egui::Color32::from_rgb(36, 42, 58))
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(92, 108, 140)))
                .inner_margin(egui::Margin::same(12.0)),
        )
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(10.0, 8.0);
            ui.label(
                RichText::new("State slots are persisted per ROM. Game emulation is paused while this menu is open.")
                    .size(15.0)
                    .color(egui::Color32::from_rgb(220, 228, 242)),
            );
            ui.add_space(4.0);

            egui::Grid::new("save_slot_grid")
                .num_columns(SAVE_SLOT_COLUMNS)
                .spacing(egui::vec2(10.0, 10.0))
                .show(ui, |ui| {
                    for (index, slot) in menu.slots.iter_mut().enumerate() {
                        let selected = index == menu.selected_slot;
                        let selected_t = ctx.animate_bool(
                            egui::Id::new(("save_slot_selected", index)),
                            selected,
                        );
                        let fill = lerp_color(
                            egui::Color32::from_rgb(43, 50, 68),
                            egui::Color32::from_rgb(88, 126, 214),
                            selected_t,
                        );
                        let stroke = egui::Stroke::new(
                            egui::lerp(1.0..=2.5, selected_t),
                            lerp_color(
                                egui::Color32::from_rgb(76, 88, 118),
                                egui::Color32::from_rgb(245, 212, 110),
                                selected_t,
                            ),
                        );
                        egui::Frame::group(ui.style())
                            .fill(fill)
                            .stroke(stroke)
                            .inner_margin(egui::Margin::same(8.0))
                            .show(ui, |ui| {
                                ui.set_min_width(thumbnail_size.x + 16.0);
                                ui.vertical(|ui| {
                                    ui.label(
                                        RichText::new(format!("SLOT {}", index + 1))
                                            .strong()
                                            .size(18.0)
                                            .color(egui::Color32::from_rgb(240, 244, 255)),
                                    );
                                    ui.add_space(2.0);

                                    if let Some(slot) = slot {
                                        if slot.texture.is_none() {
                                            let image = ColorImage::from_rgba_unmultiplied(
                                                [THUMBNAIL_WIDTH, THUMBNAIL_HEIGHT],
                                                &slot.thumbnail_rgba,
                                            );
                                            slot.texture = Some(ctx.load_texture(
                                                format!("save_slot_thumb_{}", index),
                                                image,
                                                egui::TextureOptions::LINEAR,
                                            ));
                                        }
                                        if let Some(texture) = &slot.texture {
                                            ui.image(texture, thumbnail_size);
                                        }
                                        ui.add_space(2.0);
                                        ui.label(
                                            RichText::new(format_saved_at(slot.saved_at_unix))
                                                .size(14.0)
                                                .color(egui::Color32::from_rgb(224, 230, 244)),
                                        );
                                    } else {
                                        let (rect, _) = ui.allocate_exact_size(
                                            thumbnail_size,
                                            egui::Sense::hover(),
                                        );
                                        ui.painter().rect_filled(
                                            rect,
                                            8.0,
                                            egui::Color32::from_rgb(20, 25, 34),
                                        );
                                        ui.painter().text(
                                            rect.center(),
                                            egui::Align2::CENTER_CENTER,
                                            "EMPTY",
                                            egui::FontId::proportional(18.0),
                                            egui::Color32::from_rgb(190, 200, 220),
                                        );
                                        ui.add_space(2.0);
                                        ui.label(
                                            RichText::new("No save data")
                                                .size(14.0)
                                                .color(egui::Color32::from_rgb(214, 222, 238)),
                                        );
                                    }
                                });
                            });

                        if index % SAVE_SLOT_COLUMNS == SAVE_SLOT_COLUMNS - 1 {
                            ui.end_row();
                        }
                    }
                });

            ui.separator();
            ui.label(
                RichText::new("Up/Down/Left/Right: Select slot   Z: Save   X: Load   Esc: Close")
                    .size(16.0)
                    .color(egui::Color32::from_rgb(232, 238, 250)),
            );
            let status_text = if menu.status_message.is_empty() {
                " ".to_string()
            } else {
                menu.status_message.clone()
            };
            ui.colored_label(
                egui::Color32::from_rgb(255, 210, 92),
                RichText::new(status_text).size(18.0).strong(),
            );
        });

    if menu.confirm_overwrite {
        egui::Area::new("overwrite_modal")
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                egui::Frame::window(ui.style())
                    .fill(egui::Color32::from_rgb(54, 61, 82))
                    .stroke(egui::Stroke::new(2.0, egui::Color32::from_rgb(245, 212, 110)))
                    .rounding(egui::Rounding::same(10.0))
                    .inner_margin(egui::Margin::symmetric(20.0, 16.0))
                    .show(ui, |ui| {
                        ui.set_min_width(360.0);
                        ui.vertical_centered(|ui| {
                            ui.label(
                                RichText::new("Overwrite save data?")
                                    .size(24.0)
                                    .strong()
                                    .color(egui::Color32::from_rgb(248, 250, 255)),
                            );
                            ui.add_space(8.0);
                            ui.label(
                                RichText::new("This slot already contains a saved state.")
                                    .size(16.0)
                                    .color(egui::Color32::from_rgb(230, 236, 248)),
                            );
                            ui.add_space(14.0);
                            ui.horizontal(|ui| {
                                let no_selected = !menu.confirm_yes_selected;
                                let yes_selected = menu.confirm_yes_selected;
                                for (label, selected) in [("NO", no_selected), ("YES", yes_selected)] {
                                    let fill = if selected {
                                        egui::Color32::from_rgb(255, 210, 92)
                                    } else {
                                        egui::Color32::from_rgb(78, 88, 116)
                                    };
                                    let text = if selected {
                                        RichText::new(label)
                                            .size(18.0)
                                            .strong()
                                            .color(egui::Color32::BLACK)
                                    } else {
                                        RichText::new(label)
                                            .size(18.0)
                                            .strong()
                                            .color(egui::Color32::from_rgb(238, 242, 250))
                                    };
                                    ui.add(
                                        egui::Button::new(text)
                                            .fill(fill)
                                            .min_size(egui::vec2(112.0, 42.0)),
                                    );
                                }
                            });
                            ui.add_space(10.0);
                            ui.label(
                                RichText::new("LEFT/RIGHT: Choose   Z: Confirm   X: Cancel")
                                    .size(15.0)
                                    .color(egui::Color32::from_rgb(225, 232, 246)),
                            );
                        });
                    });
            });
    }
}

struct GamepadInput {
    gilrs: Gilrs,
    active_gamepad: Option<GamepadId>,
    profile: GamepadProfile,
    pressed_codes: HashSet<u32>,
    axis_values: HashMap<u32, f32>,
    menu_combo_pressed: bool,
    left_trigger_active: bool,
    right_trigger_active: bool,
    left_trigger_activated_at: Option<Instant>,
    right_trigger_activated_at: Option<Instant>,
}

impl GamepadInput {
    fn new() -> Result<Self> {
        let gilrs = Gilrs::new().map_err(|err| Error::msg(err.to_string()))?;
        let active_gamepad = gilrs
            .gamepads()
            .find(|(_, gamepad)| gamepad.is_connected())
            .map(|(id, _)| id);

        let profile = active_gamepad
            .map(|id| Self::detect_profile(gilrs.gamepad(id).name()))
            .unwrap_or(GamepadProfile::Default);

        Ok(Self {
            gilrs,
            active_gamepad,
            profile,
            pressed_codes: HashSet::new(),
            axis_values: HashMap::new(),
            menu_combo_pressed: false,
            left_trigger_active: false,
            right_trigger_active: false,
            left_trigger_activated_at: None,
            right_trigger_activated_at: None,
        })
    }

    fn detect_profile(gamepad_name: &str) -> GamepadProfile {
        if cfg!(target_os = "macos") && gamepad_name == "Xbox Wireless Controller" {
            GamepadProfile::XboxWirelessMac
        } else {
            GamepadProfile::Default
        }
    }

    fn set_active_gamepad(&mut self, active_gamepad: Option<GamepadId>) {
        self.active_gamepad = active_gamepad;
        self.pressed_codes.clear();
        self.axis_values.clear();
        self.menu_combo_pressed = false;
        self.left_trigger_active = false;
        self.right_trigger_active = false;
        self.left_trigger_activated_at = None;
        self.right_trigger_activated_at = None;
        self.profile = active_gamepad
            .map(|id| Self::detect_profile(self.gilrs.gamepad(id).name()))
            .unwrap_or(GamepadProfile::Default);
    }

    fn is_code_pressed(&self, code: u32) -> bool {
        self.pressed_codes.contains(&code)
    }

    fn axis_value(&self, code: u32) -> f32 {
        self.axis_values.get(&code).copied().unwrap_or(0.0)
    }

    fn take_menu_combo_pressed(&mut self) -> bool {
        let pressed = self.menu_combo_pressed;
        self.menu_combo_pressed = false;
        pressed
    }

    fn update_trigger_state(
        active: &mut bool,
        activated_at: &mut Option<Instant>,
        other_active: bool,
        other_activated_at: Option<Instant>,
        next_active: bool,
    ) -> bool {
        if next_active && !*active {
            let now = Instant::now();
            *active = true;
            *activated_at = Some(now);
            return other_active
                || other_activated_at
                    .map(|t| now.duration_since(t) <= MENU_COMBO_GRACE_PERIOD)
                    .unwrap_or(false);
        }

        if !next_active {
            *active = false;
        }

        false
    }

    fn update(&mut self) -> bool {
        let mut changed = false;
        while let Some(event) = self.gilrs.next_event() {
            changed = true;
            match event.event {
                EventType::Connected => {
                    if self.active_gamepad.is_none() {
                        self.set_active_gamepad(Some(event.id));
                    }
                }
                EventType::Disconnected => {
                    let disconnected_active = self.active_gamepad == Some(event.id);
                    if disconnected_active {
                        let next_gamepad = self
                            .gilrs
                            .gamepads()
                            .find(|(_, gamepad)| gamepad.is_connected())
                            .map(|(id, _)| id);
                        self.set_active_gamepad(next_gamepad);
                    }
                }
                EventType::ButtonPressed(_, code) => {
                    self.pressed_codes.insert(code.into_u32());
                }
                EventType::ButtonChanged(_, value, code) => {
                    // Some controllers surface digital inputs as value changes.
                    if value >= 0.5 {
                        self.pressed_codes.insert(code.into_u32());
                    } else {
                        self.pressed_codes.remove(&code.into_u32());
                    }
                }
                EventType::ButtonReleased(_, code) => {
                    self.pressed_codes.remove(&code.into_u32());
                }
                EventType::AxisChanged(_, value, code) => {
                    let raw = code.into_u32();
                    self.axis_values.insert(raw, value);
                    if matches!(self.profile, GamepadProfile::XboxWirelessMac) {
                        if raw == XBOX_MAC_AXIS_LT_CODE {
                            if Self::update_trigger_state(
                                &mut self.left_trigger_active,
                                &mut self.left_trigger_activated_at,
                                self.right_trigger_active,
                                self.right_trigger_activated_at,
                                value >= 0.5,
                            ) {
                                self.menu_combo_pressed = true;
                            }
                        } else if raw == XBOX_MAC_AXIS_RT_CODE {
                            if Self::update_trigger_state(
                                &mut self.right_trigger_active,
                                &mut self.right_trigger_activated_at,
                                self.left_trigger_active,
                                self.left_trigger_activated_at,
                                value >= 0.5,
                            ) {
                                self.menu_combo_pressed = true;
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        changed
    }

    fn button_held(&self, button: JoypadButton) -> bool {
        let Some(id) = self.active_gamepad else {
            return false;
        };

        let gamepad = self.gilrs.gamepad(id);
        match self.profile {
            GamepadProfile::XboxWirelessMac => self.button_held_xbox_wireless_mac(button),
            GamepadProfile::Default => {
                let pressed = |button| gamepad.is_pressed(button);
                let axis = |axis| gamepad.value(axis);

                if button == JoypadButton::BUTTON_A {
                    pressed(Button::South)
                } else if button == JoypadButton::BUTTON_B {
                    pressed(Button::West)
                } else if button == JoypadButton::SELECT {
                    pressed(Button::Select)
                } else if button == JoypadButton::START {
                    pressed(Button::Start)
                } else if button == JoypadButton::UP {
                    pressed(Button::DPadUp) || axis(Axis::LeftStickY) <= -GAMEPAD_AXIS_THRESHOLD
                } else if button == JoypadButton::DOWN {
                    pressed(Button::DPadDown) || axis(Axis::LeftStickY) >= GAMEPAD_AXIS_THRESHOLD
                } else if button == JoypadButton::LEFT {
                    pressed(Button::DPadLeft) || axis(Axis::LeftStickX) <= -GAMEPAD_AXIS_THRESHOLD
                } else if button == JoypadButton::RIGHT {
                    pressed(Button::DPadRight) || axis(Axis::LeftStickX) >= GAMEPAD_AXIS_THRESHOLD
                } else {
                    false
                }
            }
        }
    }

    fn menu_combo_held(&self) -> bool {
        let Some(id) = self.active_gamepad else {
            return false;
        };

        if matches!(self.profile, GamepadProfile::XboxWirelessMac) {
            let left_trigger_value = self.axis_value(XBOX_MAC_AXIS_LT_CODE);
            let right_trigger_value = self.axis_value(XBOX_MAC_AXIS_RT_CODE);
            let left_trigger = left_trigger_value >= 0.5;
            let right_trigger = right_trigger_value >= 0.5;
            return left_trigger && right_trigger;
        }

        let gamepad = self.gilrs.gamepad(id);
        let left_trigger = gamepad.is_pressed(Button::LeftTrigger2)
            || gamepad.is_pressed(Button::LeftTrigger)
            || gamepad.value(Axis::LeftZ) >= 0.5;
        let right_trigger = gamepad.is_pressed(Button::RightTrigger2)
            || gamepad.is_pressed(Button::RightTrigger)
            || gamepad.value(Axis::RightZ) >= 0.5;
        left_trigger && right_trigger
    }

    fn button_held_xbox_wireless_mac(&self, button: JoypadButton) -> bool {
        let dpad_x = self.axis_value(XBOX_MAC_AXIS_DPAD_X_CODE);
        let dpad_y = self.axis_value(XBOX_MAC_AXIS_DPAD_Y_CODE);
        let left_x = self.axis_value(XBOX_MAC_AXIS_LEFT_X_CODE);
        let left_y = self.axis_value(XBOX_MAC_AXIS_LEFT_Y_CODE);

        if button == JoypadButton::BUTTON_A {
            self.is_code_pressed(XBOX_MAC_BUTTON_A_CODE)
        } else if button == JoypadButton::BUTTON_B {
            self.is_code_pressed(XBOX_MAC_BUTTON_X_CODE)
        } else if button == JoypadButton::SELECT {
            self.is_code_pressed(XBOX_MAC_BUTTON_BACK_CODE)
                || self.is_code_pressed(XBOX_MAC_BUTTON_MODE_CODE)
        } else if button == JoypadButton::START {
            self.is_code_pressed(XBOX_MAC_BUTTON_START_CODE)
        } else if button == JoypadButton::UP {
            dpad_y >= 0.5 || left_y >= GAMEPAD_AXIS_THRESHOLD
        } else if button == JoypadButton::DOWN {
            dpad_y <= -0.5 || left_y <= -GAMEPAD_AXIS_THRESHOLD
        } else if button == JoypadButton::LEFT {
            dpad_x <= -0.5 || left_x <= -GAMEPAD_AXIS_THRESHOLD
        } else if button == JoypadButton::RIGHT {
            dpad_x >= 0.5 || left_x >= GAMEPAD_AXIS_THRESHOLD
        } else {
            false
        }
    }
}

fn write_save_if_needed(nes: &rust_emu::Nes, save_path: &Option<PathBuf>) {
    if let (Some(path), Some(save_data)) = (save_path, nes.battery_ram_data()) {
        let _ = std::fs::write(path, save_data);
    }
}

fn main() -> Result<()> {
    env_logger::init();
    let event_loop = EventLoop::new();
    let mut input = WinitInputHelper::new();
    let mut gamepad_input = GamepadInput::new().ok();

    let window = {
        let size = LogicalSize::new(WIDTH as f64 * 3.0, HEIGHT as f64 * 3.0);
        WindowBuilder::new()
            .with_title(format!("Rust NES Emulator v{}", env!("CARGO_PKG_VERSION")))
            .with_inner_size(size)
            .with_min_inner_size(LogicalSize::new(WIDTH as f64, HEIGHT as f64))
            .build(&event_loop)
            .map_err(Error::msg)?
    };

    let mut pixels = {
        let window_size = window.inner_size();
        let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, &window);
        Pixels::new(WIDTH, HEIGHT, surface_texture).map_err(Error::msg)?
    };
    let egui_ctx = egui::Context::default();
    let mut egui_state = EguiWinitState::new(&window);
    egui_state.set_pixels_per_point(window.scale_factor() as f32);
    egui_state.set_max_texture_side(pixels.context().device.limits().max_texture_dimension_2d as usize);
    let mut egui_renderer =
        EguiRenderer::new(&pixels.context().device, pixels.surface_texture_format(), None, 1);

    // Load ROM
    let args: Vec<String> = std::env::args().collect();
    let mut rom_path: Option<PathBuf> = None;
    let mut tracing = false;
    let mut mmc1_logging = false;
    let mut apu_solo: u8 = 0;
    let mut wav_dump_path: Option<String> = None;
    let mut lpf_cutoff: f32 = 14000.0; // default LPF cutoff frequency (Hz)
    let mut hpf1_cutoff: f32 = 90.0;    // default HPF stage 1 (DC blocking)
    let mut hpf2_cutoff: f32 = 440.0;   // default HPF stage 2
    for arg in args.iter().skip(1) {
        if arg == "--trace" {
            tracing = true;
        } else if arg == "--mmc1-log" {
            mmc1_logging = true;
        } else if arg.starts_with("--apu-solo=") {
            // --apu-solo=1..5 (1=pulse1, 2=pulse2, 3=triangle, 4=noise, 5=dmc)
            if let Ok(ch) = arg.trim_start_matches("--apu-solo=").parse::<u8>() {
                apu_solo = ch;
            }
        } else if arg.starts_with("--wav-dump=") {
            wav_dump_path = Some(arg.trim_start_matches("--wav-dump=").to_string());
        } else if arg.starts_with("--lpf=") {
            // --lpf=<freq> : LPF cutoff in Hz (default 14000)
            if let Ok(f) = arg.trim_start_matches("--lpf=").parse::<f32>() {
                lpf_cutoff = f.clamp(1000.0, 22000.0);
            }
        } else if arg.starts_with("--hpf=") {
            // --hpf=<freq1>,<freq2> or --hpf=<freq> (sets both stages)
            let val = arg.trim_start_matches("--hpf=");
            if let Some((a, b)) = val.split_once(',') {
                if let Ok(f1) = a.parse::<f32>() {
                    hpf1_cutoff = f1.clamp(0.0, 1000.0);
                }
                if let Ok(f2) = b.parse::<f32>() {
                    hpf2_cutoff = f2.clamp(0.0, 1000.0);
                }
            } else if let Ok(f) = val.parse::<f32>() {
                let f = f.clamp(0.0, 1000.0);
                hpf1_cutoff = f;
                hpf2_cutoff = f;
            }
        } else if !arg.starts_with("--") && rom_path.is_none() {
            rom_path = Some(PathBuf::from(arg));
        }
    }
    println!("[Audio] LPF: {} Hz, HPF: {} / {} Hz", lpf_cutoff, hpf1_cutoff, hpf2_cutoff);

    let rom_data = if let Some(path) = rom_path.as_ref() {
        std::fs::read(path).map_err(Error::msg)?
    } else {
        // Dummy ROM for testing if no file provided
        let rom = vec![0; 0x8000];
        // Header
        let header = vec![
            0x4E, 0x45, 0x53, 0x1A, // NES<EOF>
            0x02, // 2x 16KB PRG ROM
            0x01, // 1x 8KB CHR ROM
            0x00, // Mapper 0
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        let mut full_rom = Vec::new();
        full_rom.extend(header);
        full_rom.extend(rom);
        full_rom.extend(vec![0; 0x2000]); // CHR ROM
        full_rom
    };

    let save_path = rom_path.as_ref().map(|path| path.with_extension("sav"));
    let state_dir = rom_path
        .as_ref()
        .map(|path| path.with_extension("states"))
        .unwrap_or_else(|| PathBuf::from(".rust_emu_states/default"));

    let mut nes = rust_emu::Nes::new_with_rom(&rom_data);
    if mmc1_logging {
        nes.bus.set_mmc1_debug(true);
    }
    if apu_solo > 0 {
        let names = ["", "Pulse1", "Pulse2", "Triangle", "Noise", "DMC"];
        let name = names.get(apu_solo as usize).unwrap_or(&"?");
        println!("[APU] Solo channel: {} ({})", apu_solo, name);
        nes.bus.apu.solo_channel = apu_solo;
    }
    if let Some(path) = save_path.as_ref() {
        if let Ok(save_data) = std::fs::read(path) {
            nes.load_battery_ram(&save_data);
        }
    }
    nes.reset();

    // Audio Setup
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .expect("No output device available");
    let config = device.default_output_config().unwrap();
    let sample_rate = config.sample_rate().0;
    nes.audio_sample_rate = sample_rate as f32;

    let audio_buffer = Arc::new(Mutex::new(VecDeque::<f32>::new()));
    let audio_buffer_out = Arc::clone(&audio_buffer);
    let num_channels = config.channels();

    let stream = device
        .build_output_stream(
            &config.into(),
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                let mut buffer = audio_buffer_out.lock().unwrap();
                for frame in data.chunks_mut(num_channels as usize) {
                    if let Some(sample) = buffer.pop_front() {
                        for channel in frame {
                            *channel = sample;
                        }
                    } else {
                        for channel in frame {
                            *channel = 0.0;
                        }
                    }
                }
            },
            |err| error!("Audio stream error: {}", err),
            None,
        )
        .unwrap();
    stream.play().unwrap();

    let mut last_frame_time = Instant::now();
    let frame_duration = Duration::from_nanos(16639267); // NES NTSC ~60.098 Hz
    let mut save_state_slot: Option<NesSnapshot> = None;
    let mut save_menu = SaveMenu::new(state_dir);
    let mut prev_menu_combo_held = false;
    let mut prev_menu_up_held = false;
    let mut prev_menu_down_held = false;
    let mut prev_menu_left_held = false;
    let mut prev_menu_right_held = false;
    let mut prev_menu_confirm_held = false;
    let mut prev_menu_load_held = false;

    // WAV dump buffer: (raw, filtered) stereo pairs
    let mut wav_samples: Vec<(f32, f32)> = Vec::new();
    let wav_enabled = wav_dump_path.is_some();
    if wav_enabled {
        println!("[WAV] Capture enabled ↁE{}", wav_dump_path.as_deref().unwrap());
    }

    if tracing {
        // Run in headless mode for tracing
        nes.reset();

        loop {
            println!("{}", nes.cpu.trace(&mut nes.bus));
            nes.tick();

            // Optional: Break on infinite loop or specific PC
            // if nes.cpu.pc == 0xC66E { break; }
        }
    } else {
        event_loop.run(move |event, _, control_flow| {
            control_flow.set_poll();

            if let Event::WindowEvent { event, .. } = &event {
                let _ = egui_state.on_event(&egui_ctx, event);
            }

            if matches!(event, Event::MainEventsCleared) && save_menu.visible {
                window.request_redraw();
            }

            // Handle redraw requests
            if let Event::RedrawRequested(_) = event {
                let full_output = if save_menu.visible {
                    let raw_input = egui_state.take_egui_input(&window);
                    let output = egui_ctx.run(raw_input, |ctx| {
                        show_save_menu_egui(ctx, &mut save_menu);
                    });
                    egui_state.handle_platform_output(&window, &egui_ctx, output.platform_output.clone());
                    Some(output)
                } else {
                    None
                };

                let frame = pixels.frame_mut();
                nes.draw(frame);

                let render_result = pixels.render_with(|encoder, render_target, context| {
                    context.scaling_renderer.render(encoder, render_target);

                    if let Some(full_output) = &full_output {
                        for (id, image_delta) in &full_output.textures_delta.set {
                            egui_renderer.update_texture(&context.device, &context.queue, *id, image_delta);
                        }

                        let clipped_primitives =
                            egui_ctx.tessellate(full_output.shapes.clone());
                        let screen_descriptor = ScreenDescriptor {
                            size_in_pixels: [window.inner_size().width, window.inner_size().height],
                            pixels_per_point: egui_state.pixels_per_point(),
                        };

                        let _user_cmd_bufs = egui_renderer.update_buffers(
                            &context.device,
                            &context.queue,
                            encoder,
                            &clipped_primitives,
                            &screen_descriptor,
                        );

                        {
                            let mut render_pass =
                                encoder.begin_render_pass(&pixels::wgpu::RenderPassDescriptor {
                                    label: Some("egui_overlay_render_pass"),
                                    color_attachments: &[Some(pixels::wgpu::RenderPassColorAttachment {
                                        view: render_target,
                                        resolve_target: None,
                                        ops: pixels::wgpu::Operations {
                                            load: pixels::wgpu::LoadOp::Load,
                                            store: true,
                                        },
                                    })],
                                    depth_stencil_attachment: None,
                                });
                            egui_renderer.render(&mut render_pass, &clipped_primitives, &screen_descriptor);
                        }

                        for id in &full_output.textures_delta.free {
                            egui_renderer.free_texture(id);
                        }
                    }

                    Ok(())
                });

                if let Err(err) = render_result {
                    error!("pixels.render() failed: {}", err);
                    write_save_if_needed(&nes, &save_path);
                    control_flow.set_exit();
                    return;
                }
            }

            let input_updated = input.update(&event);
            let gamepad_changed = gamepad_input
                .as_mut()
                .map(|gamepad_input| gamepad_input.update())
                .unwrap_or(false);

            let menu_combo_held = gamepad_input
                .as_ref()
                .map(|gamepad_input| gamepad_input.menu_combo_held())
                .unwrap_or(false);
            let gamepad_menu_combo_pressed = gamepad_input
                .as_mut()
                .map(|gamepad_input| gamepad_input.take_menu_combo_pressed())
                .unwrap_or(false);
            let gamepad_held = |button| {
                gamepad_input
                    .as_ref()
                    .map(|gamepad_input| gamepad_input.button_held(button))
                    .unwrap_or(false)
            };

            let menu_up_held =
                input.key_held(VirtualKeyCode::Up) || gamepad_held(JoypadButton::UP);
            let menu_down_held =
                input.key_held(VirtualKeyCode::Down) || gamepad_held(JoypadButton::DOWN);
            let menu_left_held =
                input.key_held(VirtualKeyCode::Left) || gamepad_held(JoypadButton::LEFT);
            let menu_right_held =
                input.key_held(VirtualKeyCode::Right) || gamepad_held(JoypadButton::RIGHT);
            let menu_confirm_held =
                input.key_held(VirtualKeyCode::Z) || gamepad_held(JoypadButton::BUTTON_A);
            let menu_load_held =
                input.key_held(VirtualKeyCode::X) || gamepad_held(JoypadButton::BUTTON_B);

            if (input_updated || gamepad_changed)
                && (input.key_pressed(VirtualKeyCode::Tab)
                    || gamepad_menu_combo_pressed
                    || (menu_combo_held && !prev_menu_combo_held))
                && !input.key_pressed(VirtualKeyCode::Escape)
            {
                if save_menu.visible {
                    save_menu.close();
                } else {
                    save_menu.open();
                    prev_menu_up_held = menu_up_held;
                    prev_menu_down_held = menu_down_held;
                    prev_menu_left_held = menu_left_held;
                    prev_menu_right_held = menu_right_held;
                    prev_menu_confirm_held = menu_confirm_held;
                    prev_menu_load_held = menu_load_held;
                }
                window.request_redraw();
            }
            prev_menu_combo_held = menu_combo_held;

            // Handle input events
            if input_updated || gamepad_changed {
                if input.key_pressed(VirtualKeyCode::Escape) || input.close_requested() {
                    if save_menu.visible {
                        save_menu.close();
                        window.request_redraw();
                        return;
                    }
                    write_save_if_needed(&nes, &save_path);
                    if let Some(ref path) = wav_dump_path {
                        if let Err(e) = write_wav_file(path, sample_rate, &wav_samples) {
                            error!("Failed to write WAV: {}", e);
                        }
                    }
                    control_flow.set_exit();
                    return;
                }

                if save_menu.visible {
                    if save_menu.close_after_load_release && !menu_load_held {
                        save_menu.close();
                        window.request_redraw();
                    } else if !save_menu.close_after_load_release
                        && !save_menu.confirm_overwrite
                        && menu_up_held
                        && !prev_menu_up_held
                    {
                        save_menu.move_selection(-1, 0);
                        window.request_redraw();
                    } else if !save_menu.close_after_load_release
                        && !save_menu.confirm_overwrite
                        && menu_down_held
                        && !prev_menu_down_held
                    {
                        save_menu.move_selection(1, 0);
                        window.request_redraw();
                    } else if !save_menu.close_after_load_release
                        && !save_menu.confirm_overwrite
                        && menu_left_held
                        && !prev_menu_left_held
                    {
                        save_menu.move_selection(0, -1);
                        window.request_redraw();
                    } else if !save_menu.close_after_load_release
                        && !save_menu.confirm_overwrite
                        && menu_right_held
                        && !prev_menu_right_held
                    {
                        save_menu.move_selection(0, 1);
                        window.request_redraw();
                    }
                    if save_menu.confirm_overwrite && !save_menu.close_after_load_release {
                        if menu_left_held && !prev_menu_left_held {
                            save_menu.confirm_yes_selected = false;
                            window.request_redraw();
                        }
                        if menu_right_held && !prev_menu_right_held {
                            save_menu.confirm_yes_selected = true;
                            window.request_redraw();
                        }
                    }
                    prev_menu_up_held = menu_up_held;
                    prev_menu_down_held = menu_down_held;
                    prev_menu_left_held = menu_left_held;
                    prev_menu_right_held = menu_right_held;
                    let menu_confirm_pressed =
                        input.key_pressed(VirtualKeyCode::Z)
                            || (menu_confirm_held && !prev_menu_confirm_held);
                    let menu_load_pressed =
                        input.key_pressed(VirtualKeyCode::X)
                            || (menu_load_held && !prev_menu_load_held);
                    prev_menu_confirm_held = menu_confirm_held;
                    prev_menu_load_held = menu_load_held;

                    if !save_menu.close_after_load_release && menu_confirm_pressed {
                        if let Err(err) = save_menu.save_selected(&nes) {
                            save_menu.status_message = format!("SAVE FAILED: {}", err);
                        }
                        window.request_redraw();
                    }
                    if !save_menu.close_after_load_release && menu_load_pressed {
                        if save_menu.confirm_overwrite {
                            save_menu.confirm_overwrite = false;
                            save_menu.confirm_yes_selected = false;
                            save_menu.status_message = "SAVE CANCELED".to_string();
                        } else {
                            match save_menu.load_selected() {
                                Ok(Some(snapshot)) => {
                                    nes.load_state(&snapshot);
                                    save_state_slot = Some(snapshot);
                                    audio_buffer.lock().unwrap().clear();
                                    last_frame_time = Instant::now();
                                    save_menu.close_after_load_release = true;
                                }
                                Ok(None) => {}
                                Err(err) => save_menu.status_message = format!("LOAD FAILED: {}", err),
                            }
                        }
                        window.request_redraw();
                    }
                } else {
                    prev_menu_up_held = false;
                    prev_menu_down_held = false;
                    prev_menu_left_held = false;
                    prev_menu_right_held = false;
                    prev_menu_confirm_held = menu_confirm_held;
                    prev_menu_load_held = menu_load_held;

                    if input.key_pressed(VirtualKeyCode::F5) {
                        save_state_slot = Some(nes.save_state());
                        println!("[State] Saved");
                    }

                    if input.key_pressed(VirtualKeyCode::F8) {
                        if let Some(snapshot) = save_state_slot.as_ref() {
                            nes.load_state(snapshot);
                            audio_buffer.lock().unwrap().clear();
                            last_frame_time = Instant::now();
                            println!("[State] Loaded");
                        } else {
                            println!("[State] No saved state");
                        }
                    }
                }

                if let Some(size) = input.window_resized() {
                    if let Err(err) = pixels.resize_surface(size.width, size.height) {
                        error!("pixels.resize_surface() failed: {}", err);
                        write_save_if_needed(&nes, &save_path);
                        control_flow.set_exit();
                        return;
                    }
                }

                let menu_open = save_menu.visible;

                nes.set_joypad_button(
                    JoypadButton::BUTTON_A,
                    !menu_open
                        && (input.key_held(VirtualKeyCode::Z) || gamepad_held(JoypadButton::BUTTON_A)),
                );
                nes.set_joypad_button(
                    JoypadButton::BUTTON_B,
                    !menu_open
                        && (input.key_held(VirtualKeyCode::X) || gamepad_held(JoypadButton::BUTTON_B)),
                );
                nes.set_joypad_button(
                    JoypadButton::SELECT,
                    !menu_open
                        && (input.key_held(VirtualKeyCode::RShift) || gamepad_held(JoypadButton::SELECT)),
                );
                nes.set_joypad_button(
                    JoypadButton::START,
                    !menu_open
                        && (input.key_held(VirtualKeyCode::Return) || gamepad_held(JoypadButton::START)),
                );
                nes.set_joypad_button(
                    JoypadButton::UP,
                    !menu_open
                        && (input.key_held(VirtualKeyCode::Up) || gamepad_held(JoypadButton::UP)),
                );
                nes.set_joypad_button(
                    JoypadButton::DOWN,
                    !menu_open
                        && (input.key_held(VirtualKeyCode::Down) || gamepad_held(JoypadButton::DOWN)),
                );
                nes.set_joypad_button(
                    JoypadButton::LEFT,
                    !menu_open
                        && (input.key_held(VirtualKeyCode::Left) || gamepad_held(JoypadButton::LEFT)),
                );
                nes.set_joypad_button(
                    JoypadButton::RIGHT,
                    !menu_open
                        && (input.key_held(VirtualKeyCode::Right) || gamepad_held(JoypadButton::RIGHT)),
                );
            }

            // Step emulator for one frame if it's time
            if !save_menu.visible && last_frame_time.elapsed() >= frame_duration {
                let mut cycles = 0;
                while cycles < 29781 {
                    let step_cycles = nes.tick();
                    cycles += step_cycles;
                    let samples = nes.get_audio_samples();
                    if !samples.is_empty() {
                        let mut buffer = audio_buffer.lock().unwrap();
                        for sample in samples {
                            if buffer.len() < 4096 {
                                buffer.push_back(sample);
                            }
                            if wav_enabled {
                                wav_samples.push((sample, sample));
                            }
                        }
                    }
                }
                last_frame_time += frame_duration;
                // Avoid "death spiral" if the computer is too slow
                if last_frame_time.elapsed() > frame_duration * 2 {
                    last_frame_time = Instant::now();
                }
                window.request_redraw();
            }
        });
        // run diverges
    }
}
