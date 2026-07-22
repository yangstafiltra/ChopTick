use anyhow::{anyhow, Result};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Paragraph, Wrap},
    Frame, Terminal,
};
use tui_big_text::BigText;
use std::{
    io::stdout,
    process::Command,
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::Duration,
};

const WORD_LIST_URL: &str =
    "https://cdn.jsdelivr.net/gh/lyc8503/baicizhan-word-meaning-API/data/list.json";
const WORD_DATA_URL: &str =
    "https://cdn.jsdelivr.net/gh/lyc8503/baicizhan-word-meaning-API/data/words";
const REFRESH_INTERVAL: u64 = 60;

static DIGITS: [[&str; 5]; 10] = [
    ["█████", "█   █", "█   █", "█   █", "█████"],
    ["  █  ", "  █  ", "  █  ", "  █  ", "  █  "],
    ["█████", "    █", "█████", "█    ", "█████"],
    ["█████", "    █", "█████", "    █", "█████"],
    ["█   █", "█   █", "█████", "    █", "    █"],
    ["█████", "█    ", "█████", "    █", "█████"],
    ["█████", "█    ", "█████", "█   █", "█████"],
    ["█████", "    █", "    █", "    █", "    █"],
    ["█████", "█   █", "█████", "█   █", "█████"],
    ["█████", "█   █", "█████", "    █", "█████"],
];

#[derive(Clone, Debug)]
struct WordData {
    word: String,
    accent: String,
    mean_cn: String,
    mean_en: String,
    sentence: String,
    sentence_trans: String,
    extra_meanings: Vec<String>,
}

#[derive(Clone)]
struct AppState {
    word: Option<WordData>,
}

fn digit_color(row: usize, ch: char) -> Color {
    if ch == ' ' {
        return Color::Reset;
    }
    let r = (180u16.saturating_sub((row as u16) * 15)).max(40) as u8;
    let g = (220u16.saturating_sub((row as u16) * 15)).max(60) as u8;
    let b = (255u16.saturating_sub((row as u16) * 20)).max(80) as u8;
    Color::Rgb(r, g, b)
}

fn render_clock_part(d1: usize, d2: usize) -> Vec<Line<'static>> {
    let mut out = Vec::new();
    for row in 0..5 {
        let mut s = String::new();
        for ch in DIGITS[d1][row].chars() {
            s.push(if ch == ' ' { ' ' } else { '█' });
        }
        s.push(' ');
        for ch in DIGITS[d2][row].chars() {
            s.push(if ch == ' ' { ' ' } else { '█' });
        }
        let spans: Vec<Span> = s
            .chars()
            .map(|ch| {
                if ch == ' ' { Span::raw(" ") }
                else { Span::styled("█", Style::default().fg(digit_color(row, '█'))) }
            })
            .collect();
        out.push(Line::from(spans));
    }
    out
}

fn render_vertical_clock<'a>() -> Vec<Line<'a>> {
    let now = chrono_now();
    let chars: Vec<char> = now.chars().collect();
    let h1 = chars[0].to_digit(10).unwrap_or(0) as usize;
    let h2 = chars[1].to_digit(10).unwrap_or(0) as usize;
    let m1 = chars[3].to_digit(10).unwrap_or(0) as usize;
    let m2 = chars[4].to_digit(10).unwrap_or(0) as usize;

    let mut all = Vec::new();

    all.extend(render_clock_part(h1, h2));

    let colon_line = Line::from(Span::styled(
        "  ••  ",
        Style::default().fg(Color::Rgb(100, 140, 180)),
    ));
    all.push(colon_line);

    all.extend(render_clock_part(m1, m2));

    all
}

fn chrono_now() -> String {
    let d = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let s = (d.as_secs() + 8 * 3600) % 86400;
    format!("{:02}:{:02}", s / 3600, (s % 3600) / 60)
}

fn curl_get(url: &str) -> Result<String> {
    let out = Command::new("curl")
        .args(["-sL", "--connect-timeout", "8", "--max-time", "12"])
        .arg(url)
        .output()?;
    if !out.status.success() {
        return Err(anyhow!("curl: {}", String::from_utf8_lossy(&out.stderr)));
    }
    Ok(String::from_utf8(out.stdout)?)
}

fn fetch_word_list() -> Result<Vec<String>> {
    let body = curl_get(WORD_LIST_URL)?;
    let json: serde_json::Value = serde_json::from_str(&body)?;
    let list = json["list"].as_array().ok_or_else(|| anyhow!("no list"))?;
    Ok(list.iter().filter_map(|v| v.as_str().map(String::from)).collect())
}

fn fetch_word_data(word: &str) -> Result<WordData> {
    let key: String = word
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | ' ' => '_',
            _ => c,
        })
        .collect();

    let body = curl_get(&format!("{}/{}.json", WORD_DATA_URL, key))?;
    let json: serde_json::Value = serde_json::from_str(&body)?;

    let mean_cn = json["mean_cn"].as_str().unwrap_or("").to_string();
    let mean_en = json["mean_en"].as_str().unwrap_or("").to_string();

    let mut extra_cn: Vec<String> = Vec::new();
    let parts: Vec<&str> = mean_cn.split('；').collect();
    if parts.len() > 1 {
        for p in parts.iter().skip(1) {
            let t = p.trim().to_string();
            if !t.is_empty() && t != parts[0].trim() {
                extra_cn.push(t);
            }
        }
    }

    let mut extra_en = Vec::new();
    if let Ok(body2) = curl_get(&format!("https://api.dictionaryapi.dev/api/v2/entries/en/{}", word)) {
        if let Ok(arr) = serde_json::from_str::<serde_json::Value>(&body2) {
            if let Some(meanings) = arr[0]["meanings"].as_array() {
                for m in meanings {
                    let pos = m["partOfSpeech"].as_str().unwrap_or("");
                    if let Some(defs) = m["definitions"].as_array() {
                        for (i, d) in defs.iter().enumerate() {
                            if i == 0 { continue; }
                            if let Some(def) = d["definition"].as_str() {
                                if def.len() < 80 {
                                    extra_en.push(format!("{}. {}", pos, def));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let mut all_extra = extra_cn;
    all_extra.extend(extra_en);

    Ok(WordData {
        word: json["word"].as_str().unwrap_or(word).to_string(),
        accent: json["accent"].as_str().unwrap_or("").to_string(),
        mean_cn,
        mean_en,
        sentence: json["sentence"].as_str().unwrap_or("").to_string(),
        sentence_trans: json["sentence_trans"].as_str().unwrap_or("").to_string(),
        extra_meanings: all_extra,
    })
}

fn fetch_random_word(words: &[String]) -> (Option<WordData>, String) {
    let idx = rand::random::<usize>() % words.len();
    match words.get(idx).and_then(|w| fetch_word_data(w).ok()) {
        Some(data) => {
            let msg = format!("✓ {}", data.word);
            (Some(data), msg)
        }
        None => (None, "✗ fetch failed".into()),
    }
}

fn main() -> Result<()> {
    let orig_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |p| {
        let _ = disable_raw_mode();
        let _ = execute!(std::io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
        orig_hook(p);
    }));

    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen, EnableMouseCapture)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    let (tx, rx): (Sender<AppState>, Receiver<AppState>) = mpsc::channel();
    let mut state = AppState { word: None };

    let words_ref: std::sync::Arc<std::sync::Mutex<Option<Vec<String>>>> =
        std::sync::Arc::new(std::sync::Mutex::new(None));
    let words_for_fetch = words_ref.clone();

    thread::spawn(move || {
        let list = fetch_word_list().ok();
        *words_for_fetch.lock().unwrap() = list;
    });

    let mut first_done = false;

    loop {
        if !first_done {
            if let Some(ref words) = *words_ref.lock().unwrap() {
                let (word, _) = fetch_random_word(words);
                state.word = word;
                first_done = true;

                let wc = words.clone();
                let tc = tx.clone();
                thread::spawn(move || loop {
                    thread::sleep(Duration::from_secs(REFRESH_INTERVAL));
                    let (word, _) = fetch_random_word(&wc);
                    let _ = tc.send(AppState { word });
                });
            }
        }

        terminal.draw(|f| ui(f, &state))?;

        if let Ok(new_state) = rx.try_recv() {
            state = new_state;
        }

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                let is_ctrl_c = key.code == KeyCode::Char('c')
                    && key.modifiers.contains(KeyModifiers::CONTROL);
                if key.code == KeyCode::Char('q') || key.code == KeyCode::Esc || is_ctrl_c {
                    break;
                }
                if key.code == KeyCode::Char('r') && first_done {
                    if let Some(ref words) = *words_ref.lock().unwrap() {
                        let (word, _) = fetch_random_word(words);
                        state.word = word;
                    }
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(stdout(), LeaveAlternateScreen, DisableMouseCapture)?;
    Ok(())
}

fn ui(f: &mut Frame, state: &AppState) {
    let area = f.area();
    if area.width < 40 || area.height < 10 {
        return;
    }

    let horiz = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(25), Constraint::Percentage(75)])
        .split(area);

    let clock_lines = render_vertical_clock();
    let clock_w = clock_lines.first().map(|l| l.width() as u16).unwrap_or(0);
    let clock_h = clock_lines.len() as u16;

    let left = horiz[0];
    let cx = left.x + (left.width.saturating_sub(clock_w)) / 2;
    let cy = left.y + (left.height.saturating_sub(clock_h)) / 2;

    let clock_para = Paragraph::new(Text::from(clock_lines)).alignment(Alignment::Center);
    f.render_widget(
        clock_para,
        Rect { x: cx, y: cy, width: std::cmp::max(clock_w, 1), height: clock_h },
    );

    if let Some(ref w) = state.word {
        let right = horiz[1];
        let pad = 2u16;
        let rw = right.width.saturating_sub(pad * 2);

        let big = BigText::builder()
            .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
            .lines(vec![Line::from(w.word.as_str())])
            .build();

        let big_area = Rect {
            x: right.x + pad,
            y: right.y + 1,
            width: rw,
            height: 8,
        };
        f.render_widget(big, big_area);

        let mut lines = Vec::new();

        if !w.accent.is_empty() {
            lines.push(Line::from(Span::styled(
                &w.accent,
                Style::default().fg(Color::DarkGray),
            )));
        }
        if !w.mean_cn.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                &w.mean_cn,
                Style::default().fg(Color::White),
            )));
        }
        if !w.mean_en.is_empty() {
            lines.push(Line::from(Span::styled(
                &w.mean_en,
                Style::default().fg(Color::DarkGray),
            )));
        }
        if !w.extra_meanings.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                " 熟词生义:",
                Style::default().fg(Color::Magenta),
            )));
            for m in w.extra_meanings.iter().take(3) {
                lines.push(Line::from(Span::styled(
                    format!("   • {}", m),
                    Style::default().fg(Color::Magenta),
                )));
            }
        }
        if !w.sentence.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                &w.sentence,
                Style::default().fg(Color::Green),
            )));
        }
        if !w.sentence_trans.is_empty() {
            lines.push(Line::from(Span::styled(
                format!("  ─ {}", w.sentence_trans),
                Style::default().fg(Color::DarkGray),
            )));
        }

        let info_y = big_area.y + big_area.height;
        let info_area = Rect {
            x: right.x + pad,
            y: info_y,
            width: rw,
            height: right.height.saturating_sub(info_y - right.y),
        };
        if info_area.width > 10 && info_area.height > 2 {
            f.render_widget(
                Paragraph::new(Text::from(lines)).wrap(Wrap { trim: false }),
                info_area,
            );
        }
    }
}
