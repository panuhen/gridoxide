use ratatui::style::Color;

/// Theme configuration for the TUI
#[derive(Debug, Clone)]
pub struct Theme {
    pub name: &'static str,
    pub bg: Color,
    pub fg: Color,
    pub grid_active: Color,
    pub grid_inactive: Color,
    pub grid_cursor: Color,
    pub track_label: Color,
    pub meter_low: Color,
    pub meter_mid: Color,
    pub meter_high: Color,
    pub border: Color,
    pub highlight: Color,
    pub dimmed: Color,
}

impl Theme {
    /// Default theme - uses terminal's ANSI colors
    pub fn default_theme() -> Self {
        Self {
            name: "default",
            bg: Color::Reset,
            fg: Color::Reset,
            grid_active: Color::Green,
            grid_inactive: Color::DarkGray,
            grid_cursor: Color::Yellow,
            track_label: Color::Cyan,
            meter_low: Color::Green,
            meter_mid: Color::Yellow,
            meter_high: Color::Red,
            border: Color::White,
            highlight: Color::Magenta,
            dimmed: Color::DarkGray,
        }
    }

    /// Classic green CRT phosphor look
    pub fn phosphor_green() -> Self {
        Self {
            name: "phosphor-green",
            bg: Color::Black,
            fg: Color::Rgb(0, 255, 0),
            grid_active: Color::Rgb(0, 255, 0),
            grid_inactive: Color::Rgb(0, 80, 0),
            grid_cursor: Color::Rgb(180, 255, 180),
            track_label: Color::Rgb(0, 200, 0),
            meter_low: Color::Rgb(0, 150, 0),
            meter_mid: Color::Rgb(0, 200, 0),
            meter_high: Color::Rgb(0, 255, 0),
            border: Color::Rgb(0, 180, 0),
            highlight: Color::Rgb(150, 255, 150),
            dimmed: Color::Rgb(0, 60, 0),
        }
    }

    /// Warm amber monochrome CRT
    pub fn amber_crt() -> Self {
        Self {
            name: "amber-crt",
            bg: Color::Black,
            fg: Color::Rgb(255, 176, 0),
            grid_active: Color::Rgb(255, 176, 0),
            grid_inactive: Color::Rgb(80, 55, 0),
            grid_cursor: Color::Rgb(255, 220, 150),
            track_label: Color::Rgb(200, 140, 0),
            meter_low: Color::Rgb(150, 100, 0),
            meter_mid: Color::Rgb(200, 140, 0),
            meter_high: Color::Rgb(255, 176, 0),
            border: Color::Rgb(180, 125, 0),
            highlight: Color::Rgb(255, 220, 150),
            dimmed: Color::Rgb(60, 40, 0),
        }
    }

    /// Cool blue terminal tones
    pub fn blue_terminal() -> Self {
        Self {
            name: "blue-terminal",
            bg: Color::Black,
            fg: Color::Rgb(100, 180, 255),
            grid_active: Color::Rgb(100, 180, 255),
            grid_inactive: Color::Rgb(30, 60, 100),
            grid_cursor: Color::Rgb(180, 220, 255),
            track_label: Color::Rgb(80, 150, 220),
            meter_low: Color::Rgb(50, 120, 180),
            meter_mid: Color::Rgb(80, 150, 220),
            meter_high: Color::Rgb(100, 180, 255),
            border: Color::Rgb(70, 130, 200),
            highlight: Color::Rgb(180, 220, 255),
            dimmed: Color::Rgb(25, 50, 80),
        }
    }

    /// Stark black and white high contrast
    pub fn high_contrast() -> Self {
        Self {
            name: "high-contrast",
            bg: Color::Black,
            fg: Color::White,
            grid_active: Color::White,
            grid_inactive: Color::Rgb(60, 60, 60),
            grid_cursor: Color::White,
            track_label: Color::White,
            meter_low: Color::Rgb(180, 180, 180),
            meter_mid: Color::Rgb(220, 220, 220),
            meter_high: Color::White,
            border: Color::White,
            highlight: Color::White,
            dimmed: Color::Rgb(80, 80, 80),
        }
    }

    /// Get theme by name
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "default" => Some(Self::default_theme()),
            "phosphor-green" => Some(Self::phosphor_green()),
            "amber-crt" => Some(Self::amber_crt()),
            "blue-terminal" => Some(Self::blue_terminal()),
            "high-contrast" => Some(Self::high_contrast()),
            _ => None,
        }
    }

    /// List all available theme names
    pub fn available_themes() -> &'static [&'static str] {
        &[
            "default",
            "phosphor-green",
            "amber-crt",
            "blue-terminal",
            "high-contrast",
        ]
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::default_theme()
    }
}
