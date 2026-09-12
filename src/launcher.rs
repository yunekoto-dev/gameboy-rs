//! A small retro, Game-Boy-shaped picker window shown when `gameboy-rs` is
//! launched without a ROM argument (e.g. by double-clicking the .exe).
//!
//! It draws a stylised Game Boy shell with a hand-built 3x5 pixel font (no
//! font files or extra rendering dependencies needed) and lets the user
//! browse their computer for a `.gb`/`.gbc` file via the OS's native file
//! picker (through the `rfd` crate).

use std::path::PathBuf;
use std::time::{Duration, Instant};

use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::mouse::MouseButton;
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::WindowCanvas;

const SHELL_BG: Color = Color::RGB(203, 199, 209);
const SHELL_EDGE: Color = Color::RGB(150, 148, 160);
const BEZEL: Color = Color::RGB(58, 58, 68);
const LCD_LIGHT: Color = Color::RGB(155, 188, 15);
const LCD_DARK: Color = Color::RGB(15, 56, 15);
const DPAD: Color = Color::RGB(45, 45, 52);
const AB_BUTTON: Color = Color::RGB(130, 20, 60);
const PILL: Color = Color::RGB(120, 118, 130);
const LOGO: Color = Color::RGB(80, 40, 120);
const FOOTER: Color = Color::RGB(110, 108, 118);

/// Returns a hand-drawn 3x5 pixel glyph for the (uppercase) characters this
/// UI actually needs. Unsupported characters (including space) are handled
/// by the caller, which simply advances the cursor.
fn glyph(c: char) -> Option<[u8; 5]> {
    Some(match c.to_ascii_uppercase() {
        'A' => [0b010, 0b101, 0b111, 0b101, 0b101],
        'B' => [0b110, 0b101, 0b110, 0b101, 0b110],
        'C' => [0b011, 0b100, 0b100, 0b100, 0b011],
        'D' => [0b110, 0b101, 0b101, 0b101, 0b110],
        'E' => [0b111, 0b100, 0b110, 0b100, 0b111],
        'G' => [0b011, 0b100, 0b101, 0b101, 0b011],
        'I' => [0b111, 0b010, 0b010, 0b010, 0b111],
        'L' => [0b100, 0b100, 0b100, 0b100, 0b111],
        'M' => [0b101, 0b111, 0b111, 0b101, 0b101],
        'N' => [0b101, 0b111, 0b111, 0b111, 0b101],
        'O' => [0b010, 0b101, 0b101, 0b101, 0b010],
        'P' => [0b110, 0b101, 0b110, 0b100, 0b100],
        'Q' => [0b010, 0b101, 0b101, 0b010, 0b001],
        'R' => [0b110, 0b101, 0b110, 0b101, 0b101],
        'S' => [0b011, 0b100, 0b010, 0b001, 0b110],
        'T' => [0b111, 0b010, 0b010, 0b010, 0b010],
        'U' => [0b101, 0b101, 0b101, 0b101, 0b010],
        'Y' => [0b101, 0b101, 0b010, 0b010, 0b010],
        '-' => [0b000, 0b000, 0b111, 0b000, 0b000],
        _ => return None,
    })
}

const GLYPH_W: i32 = 3;
const GLYPH_GAP: i32 = 1;

fn text_width(text: &str, scale: i32) -> i32 {
    text.chars().count() as i32 * (GLYPH_W + GLYPH_GAP) * scale
}

fn draw_text(canvas: &mut WindowCanvas, text: &str, x: i32, y: i32, scale: i32, color: Color) {
    canvas.set_draw_color(color);
    let mut cursor_x = x;
    for ch in text.chars() {
        if let Some(rows) = glyph(ch) {
            for (row, bits) in rows.iter().enumerate() {
                for col in 0..3 {
                    if (bits >> (2 - col)) & 1 == 1 {
                        let px = cursor_x + col * scale;
                        let py = y + row as i32 * scale;
                        let _ = canvas.fill_rect(Rect::new(px, py, scale as u32, scale as u32));
                    }
                }
            }
        }
        cursor_x += (GLYPH_W + GLYPH_GAP) * scale;
    }
}

fn draw_text_centered(canvas: &mut WindowCanvas, text: &str, center_x: i32, y: i32, scale: i32, color: Color) {
    let x = center_x - text_width(text, scale) / 2;
    draw_text(canvas, text, x, y, scale, color);
}

fn fill_circle(canvas: &mut WindowCanvas, cx: i32, cy: i32, r: i32, color: Color) {
    canvas.set_draw_color(color);
    for dy in -r..=r {
        let dx = (((r * r - dy * dy) as f64).sqrt()) as i32;
        if dx > 0 {
            let _ = canvas.fill_rect(Rect::new(cx - dx, cy + dy, (dx * 2) as u32, 1));
        }
    }
}

fn fill_pill(canvas: &mut WindowCanvas, x: i32, y: i32, w: i32, h: i32, color: Color) {
    let r = h / 2;
    fill_circle(canvas, x + r, y + r, r, color);
    fill_circle(canvas, x + w - r, y + r, r, color);
    canvas.set_draw_color(color);
    let _ = canvas.fill_rect(Rect::new(x + r, y, (w - h) as u32, h as u32));
}

fn draw_dpad(canvas: &mut WindowCanvas, cx: i32, cy: i32) {
    let thick = 18;
    let len = 54;
    canvas.set_draw_color(DPAD);
    let _ = canvas.fill_rect(Rect::new(cx - thick / 2, cy - len / 2, thick as u32, len as u32));
    let _ = canvas.fill_rect(Rect::new(cx - len / 2, cy - thick / 2, len as u32, thick as u32));
}

/// Opens the OS's native file picker, filtered to Game Boy ROMs.
fn open_file_dialog() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title("Select a Game Boy / Game Boy Color ROM")
        .add_filter("Game Boy ROM", &["gb", "gbc"])
        .add_filter("All files", &["*"])
        .pick_file()
}

fn is_gb_rom(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("gb") || e.eq_ignore_ascii_case("gbc"))
        .unwrap_or(false)
}

fn draw_frame(canvas: &mut WindowCanvas, elapsed: Duration, status: &Option<(&'static str, Instant)>) {
    let (w, _h) = canvas.output_size().unwrap_or((340, 560));
    let w = w as i32;
    let cx = w / 2;

    canvas.set_draw_color(SHELL_BG);
    canvas.clear();

    canvas.set_draw_color(SHELL_EDGE);
    let _ = canvas.fill_rect(Rect::new(0, 0, w as u32, 8));

    // Screen bezel + LCD.
    let bezel = Rect::new(30, 46, (w - 60) as u32, 210);
    canvas.set_draw_color(BEZEL);
    let _ = canvas.fill_rect(bezel);
    let screen = Rect::new(bezel.x() + 16, bezel.y() + 16, bezel.width() - 32, bezel.height() - 32);
    canvas.set_draw_color(LCD_LIGHT);
    let _ = canvas.fill_rect(screen);

    draw_text_centered(canvas, "NO CARTRIDGE", cx, screen.y() + 26, 3, LCD_DARK);

    let blink_on = (elapsed.as_millis() / 550).is_multiple_of(2);
    if let Some((msg, _)) = status {
        draw_text_centered(canvas, msg, cx, screen.y() + 70, 3, LCD_DARK);
    } else if blink_on {
        draw_text_centered(canvas, "PRESS ENTER", cx, screen.y() + 70, 3, LCD_DARK);
    }
    draw_text_centered(canvas, "TO SELECT", cx, screen.y() + 95, 3, LCD_DARK);
    draw_text_centered(canvas, "A GAME", cx, screen.y() + 120, 3, LCD_DARK);
    draw_text_centered(canvas, "GB AND GBC ONLY", cx, screen.y() + 155, 2, LCD_DARK);

    // Logo under the screen.
    draw_text_centered(canvas, "GAMEBOY-RS", cx, bezel.y() + bezel.height() as i32 + 26, 4, LOGO);

    // D-pad.
    draw_dpad(canvas, 88, 400);

    // A / B buttons.
    fill_circle(canvas, cx + 92, 375, 20, AB_BUTTON);
    fill_circle(canvas, cx + 52, 405, 20, AB_BUTTON);
    draw_text(canvas, "A", cx + 84, 345, 2, BEZEL);
    draw_text(canvas, "B", cx + 44, 435, 2, BEZEL);

    // Start / Select pills.
    fill_pill(canvas, cx - 60, 470, 54, 14, PILL);
    fill_pill(canvas, cx + 6, 470, 54, 14, PILL);
    draw_text_centered(canvas, "SELECT", cx - 33, 492, 2, BEZEL);
    draw_text_centered(canvas, "START", cx + 33, 492, 2, BEZEL);

    draw_text_centered(canvas, "ESC TO QUIT", cx, 530, 2, FOOTER);
}

/// Shows the retro picker window and blocks until the user either chooses a
/// ROM (`Some(path)`) or closes the window / presses Escape (`None`).
pub fn pick_rom() -> Option<PathBuf> {
    let sdl = sdl2::init().ok()?;
    let video = sdl.video().ok()?;
    let window = video
        .window("gameboy-rs", 340, 570)
        .position_centered()
        .build()
        .ok()?;
    let mut canvas = window.into_canvas().build().ok()?;
    let mut pump = sdl.event_pump().ok()?;

    let start = Instant::now();
    let mut status: Option<(&'static str, Instant)> = None;
    let mut chosen: Option<PathBuf> = None;

    'running: loop {
        for event in pump.poll_iter() {
            match event {
                Event::Quit { .. } => break 'running,
                Event::KeyDown { keycode: Some(Keycode::Escape), .. } => break 'running,
                Event::KeyDown { keycode: Some(Keycode::Return), repeat: false, .. }
                | Event::KeyDown { keycode: Some(Keycode::Space), repeat: false, .. }
                | Event::MouseButtonDown { mouse_btn: MouseButton::Left, .. } => {
                    if let Some(path) = open_file_dialog() {
                        if is_gb_rom(&path) {
                            chosen = Some(path);
                            break 'running;
                        } else {
                            status = Some(("GB / GBC ONLY", Instant::now()));
                        }
                    }
                }
                _ => {}
            }
        }

        if let Some((_, at)) = status {
            if at.elapsed() > Duration::from_secs(2) {
                status = None;
            }
        }

        draw_frame(&mut canvas, start.elapsed(), &status);
        canvas.present();
        std::thread::sleep(Duration::from_millis(16));
    }

    chosen
}
