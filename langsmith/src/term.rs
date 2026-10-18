use terminal_colorsaurus::{color_palette, QueryOptions};

#[derive(Debug, Clone, Copy)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub fn hex(&self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

pub struct Palette {
    pub fg: Rgb,
    pub bg: Rgb,
}

impl Palette {
    /// Ask the actual terminal what its foreground/background colors are
    /// right now (via the OSC 10/11 "dynamic colors" query), so a generated
    /// glyph renders in the person's real theme instead of a fixed
    /// black-on-white default. Falls back to black-on-white if the terminal
    /// doesn't answer -- not a real TTY, `TERM=dumb`, output redirected to a
    /// file, etc.
    pub fn detect() -> Palette {
        match color_palette(QueryOptions::default()) {
            Ok(p) => Palette {
                fg: to_rgb(p.foreground.scale_to_8bit()),
                bg: to_rgb(p.background.scale_to_8bit()),
            },
            Err(_) => Palette {
                fg: Rgb { r: 0, g: 0, b: 0 },
                bg: Rgb {
                    r: 255,
                    g: 255,
                    b: 255,
                },
            },
        }
    }
}

fn to_rgb((r, g, b): (u8, u8, u8)) -> Rgb {
    Rgb { r, g, b }
}
