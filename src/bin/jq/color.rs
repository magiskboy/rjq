use std::env;
use std::io::{self, IsTerminal};

/// Environment variable for a one-line JSON color scheme.
///
/// Format (comma-separated `role=color` pairs):
/// ```text
/// RJQ_COLORS=key=#569CD6,string=#CE9178,number=#B5CEA8,boolean=#569CD6,null=#808080,punct=#D4D4D4
/// ```
///
/// Colors are `#RRGGBB` (optional `#`). Unknown or invalid values fall back to
/// the built-in default scheme. Roles may be omitted to keep defaults.
pub const COLORS_ENV: &str = "RJQ_COLORS";

/// Terminal color capability, auto-detected from the environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorLevel {
    /// No ANSI colors (`NO_COLOR`, `TERM=dumb`, or non-TTY).
    None,
    /// Basic 16 ANSI colors (`TERM=xterm`, `ansi`, `screen`, …).
    Basic,
    /// 256-color palette (`TERM=*256color*`).
    Ansi256,
    /// 24-bit true color (`COLORTERM=truecolor|24bit`, `TERM=*-direct`).
    TrueColor,
}

impl ColorLevel {
    /// Detect color support for stdout.
    ///
    /// Priority:
    /// 1. `NO_COLOR` (any value) → [`None`](Self::None)
    /// 2. `FORCE_COLOR` → force a level even when not a TTY
    /// 3. non-TTY stdout → [`None`](Self::None)
    /// 4. `TERM=dumb` → [`None`](Self::None)
    /// 5. `COLORTERM=truecolor|24bit` or `TERM=*-direct` → [`TrueColor`](Self::TrueColor)
    /// 6. `TERM=*256color*` → [`Ansi256`](Self::Ansi256)
    /// 7. otherwise on a TTY → [`Basic`](Self::Basic) (xterm / vt100 / screen / …)
    pub fn detect() -> Self {
        detect_color_level(
            io::stdout().is_terminal(),
            env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty()),
            env::var("FORCE_COLOR").ok().as_deref(),
            env::var("COLORTERM").ok().as_deref(),
            env::var("TERM").ok().as_deref(),
        )
    }
}

fn detect_color_level(
    is_tty: bool,
    no_color: bool,
    force_color: Option<&str>,
    colorterm: Option<&str>,
    term: Option<&str>,
) -> ColorLevel {
    if no_color {
        return ColorLevel::None;
    }

    match force_color {
        Some("0") | Some("") => return ColorLevel::None,
        Some("1") => return ColorLevel::Basic,
        Some("2") => return ColorLevel::Ansi256,
        Some("3") | Some(_) => return ColorLevel::TrueColor,
        None => {}
    }

    if !is_tty {
        return ColorLevel::None;
    }

    if let Some(term) = term {
        if term == "dumb" {
            return ColorLevel::None;
        }
        if term.ends_with("-direct") || term.contains("-direct-") {
            return ColorLevel::TrueColor;
        }
    }

    if let Some(ct) = colorterm {
        let ct = ct.to_ascii_lowercase();
        if ct.contains("truecolor") || ct.contains("24bit") {
            return ColorLevel::TrueColor;
        }
    }

    if let Some(term) = term {
        if term.contains("256color") || term.contains("256colour") {
            return ColorLevel::Ansi256;
        }
    }

    ColorLevel::Basic
}

/// RGB color for one JSON token role.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// ANSI SGR open sequence for this color at the given terminal level.
    pub fn ansi_open(self, level: ColorLevel) -> String {
        match level {
            ColorLevel::None => String::new(),
            ColorLevel::Basic => {
                let code = rgb_to_ansi16(self);
                format!("\x1b[{code}m")
            }
            ColorLevel::Ansi256 => {
                let n = rgb_to_ansi256(self);
                format!("\x1b[38;5;{n}m")
            }
            ColorLevel::TrueColor => {
                format!("\x1b[38;2;{};{};{}m", self.r, self.g, self.b)
            }
        }
    }
}

/// Semantic JSON colorscheme (RGB roles), adapted to the active [`ColorLevel`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColorScheme {
    pub key: Rgb,
    pub string: Rgb,
    pub number: Rgb,
    pub boolean: Rgb,
    pub null: Rgb,
    pub punct: Rgb,
}

impl Default for ColorScheme {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl ColorScheme {
    /// Built-in default (VS Code Dark+ inspired).
    pub const DEFAULT: Self = Self {
        key: Rgb::new(0x56, 0x9C, 0xD6),
        string: Rgb::new(0xCE, 0x91, 0x78),
        number: Rgb::new(0xB5, 0xCE, 0xA8),
        boolean: Rgb::new(0x56, 0x9C, 0xD6),
        null: Rgb::new(0x80, 0x80, 0x80),
        punct: Rgb::new(0xD4, 0xD4, 0xD4),
    };

    /// Load from [`COLORS_ENV`], or [`DEFAULT`](Self::DEFAULT) when unset/invalid.
    pub fn from_env() -> Self {
        match env::var(COLORS_ENV) {
            Ok(raw) if !raw.trim().is_empty() => Self::parse(&raw).unwrap_or(Self::DEFAULT),
            _ => Self::DEFAULT,
        }
    }

    /// Parse a one-line scheme: `key=#RRGGBB,string=#RRGGBB,...`
    ///
    /// Missing roles keep [`DEFAULT`](Self::DEFAULT) values. Returns `None` if
    /// the string is empty or contains an invalid `role=color` entry.
    pub fn parse(input: &str) -> Option<Self> {
        let input = input.trim();
        if input.is_empty() {
            return None;
        }

        let mut scheme = Self::DEFAULT;
        let mut saw_any = false;

        for part in input.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            let (role, color) = part.split_once('=')?;
            let role = role.trim();
            let color = parse_hex_color(color.trim())?;
            match role.to_ascii_lowercase().as_str() {
                "key" | "objectkey" | "objectkeys" => scheme.key = color,
                "string" | "strings" => scheme.string = color,
                "number" | "numbers" => scheme.number = color,
                "boolean" | "bool" | "true" | "false" => scheme.boolean = color,
                "null" => scheme.null = color,
                "punct" | "punctuation" | "array" | "object" => scheme.punct = color,
                _ => return None,
            }
            saw_any = true;
        }

        saw_any.then_some(scheme)
    }
}

fn parse_hex_color(raw: &str) -> Option<Rgb> {
    let hex = raw.strip_prefix('#').unwrap_or(raw);
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(Rgb::new(r, g, b))
}

fn rgb_to_ansi16(rgb: Rgb) -> u8 {
    // Classic + bright 16-color palette (approximate VGA).
    const PALETTE: [(u8, u8, u8, u8); 16] = [
        (0x00, 0x00, 0x00, 30), // black
        (0x80, 0x00, 0x00, 31), // red
        (0x00, 0x80, 0x00, 32), // green
        (0x80, 0x80, 0x00, 33), // yellow
        (0x00, 0x00, 0x80, 34), // blue
        (0x80, 0x00, 0x80, 35), // magenta
        (0x00, 0x80, 0x80, 36), // cyan
        (0xc0, 0xc0, 0xc0, 37), // white
        (0x80, 0x80, 0x80, 90), // bright black
        (0xff, 0x00, 0x00, 91), // bright red
        (0x00, 0xff, 0x00, 92), // bright green
        (0xff, 0xff, 0x00, 93), // bright yellow
        (0x00, 0x00, 0xff, 94), // bright blue
        (0xff, 0x00, 0xff, 95), // bright magenta
        (0x00, 0xff, 0xff, 96), // bright cyan
        (0xff, 0xff, 0xff, 97), // bright white
    ];

    let mut best = PALETTE[0].3;
    let mut best_dist = u32::MAX;
    for (r, g, b, code) in PALETTE {
        let dr = rgb.r as i32 - r as i32;
        let dg = rgb.g as i32 - g as i32;
        let db = rgb.b as i32 - b as i32;
        let dist = (dr * dr + dg * dg + db * db) as u32;
        if dist < best_dist {
            best_dist = dist;
            best = code;
        }
    }
    best
}

fn rgb_to_ansi256(rgb: Rgb) -> u8 {
    // Prefer grayscale ramp when nearly gray.
    let max = rgb.r.max(rgb.g).max(rgb.b);
    let min = rgb.r.min(rgb.g).min(rgb.b);
    if (max as i16 - min as i16) < 8 {
        let gray = (rgb.r as u16 + rgb.g as u16 + rgb.b as u16) / 3;
        if gray < 8 {
            return 16;
        }
        if gray > 248 {
            return 231;
        }
        return (232 + ((gray as i16 - 8) / 10).clamp(0, 23) as u8).min(231);
    }

    let to_cube = |c: u8| -> u8 {
        if c < 48 {
            0
        } else if c < 114 {
            1
        } else {
            ((c - 35) / 40).min(5)
        }
    };
    16 + 36 * to_cube(rgb.r) + 6 * to_cube(rgb.g) + to_cube(rgb.b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_respects_no_color() {
        assert_eq!(
            detect_color_level(true, true, None, Some("truecolor"), Some("xterm-256color")),
            ColorLevel::None
        );
    }

    #[test]
    fn detect_force_color_levels() {
        assert_eq!(
            detect_color_level(false, false, Some("1"), None, None),
            ColorLevel::Basic
        );
        assert_eq!(
            detect_color_level(false, false, Some("2"), None, None),
            ColorLevel::Ansi256
        );
        assert_eq!(
            detect_color_level(false, false, Some("3"), None, None),
            ColorLevel::TrueColor
        );
    }

    #[test]
    fn detect_non_tty_is_none() {
        assert_eq!(
            detect_color_level(false, false, None, Some("truecolor"), Some("xterm")),
            ColorLevel::None
        );
    }

    #[test]
    fn detect_dumb_term() {
        assert_eq!(
            detect_color_level(true, false, None, None, Some("dumb")),
            ColorLevel::None
        );
    }

    #[test]
    fn detect_truecolor_via_colorterm() {
        assert_eq!(
            detect_color_level(true, false, None, Some("truecolor"), Some("xterm-256color")),
            ColorLevel::TrueColor
        );
    }

    #[test]
    fn detect_256color_via_term() {
        assert_eq!(
            detect_color_level(true, false, None, None, Some("xterm-256color")),
            ColorLevel::Ansi256
        );
    }

    #[test]
    fn detect_basic_xterm() {
        assert_eq!(
            detect_color_level(true, false, None, None, Some("xterm")),
            ColorLevel::Basic
        );
    }

    #[test]
    fn parse_scheme_full_and_partial() {
        let full = ColorScheme::parse(
            "key=#112233,string=#445566,number=#778899,boolean=#aabbcc,null=#ddeeff,punct=#010203",
        )
        .unwrap();
        assert_eq!(full.key, Rgb::new(0x11, 0x22, 0x33));
        assert_eq!(full.punct, Rgb::new(0x01, 0x02, 0x03));

        let partial = ColorScheme::parse("string=ff0000").unwrap();
        assert_eq!(partial.string, Rgb::new(0xff, 0x00, 0x00));
        assert_eq!(partial.key, ColorScheme::DEFAULT.key);
    }

    #[test]
    fn parse_scheme_rejects_invalid() {
        assert!(ColorScheme::parse("").is_none());
        assert!(ColorScheme::parse("key=zzz").is_none());
        assert!(ColorScheme::parse("unknown=#ffffff").is_none());
        assert!(ColorScheme::parse("key").is_none());
    }

    #[test]
    fn ansi_open_truecolor() {
        let open = Rgb::new(1, 2, 3).ansi_open(ColorLevel::TrueColor);
        assert_eq!(open, "\x1b[38;2;1;2;3m");
    }
}
