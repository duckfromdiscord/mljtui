use ansi_to_tui::IntoText;
use clap::{Arg, Command};
use crossbeam::channel::unbounded;
use crossterm::{
    event::{self, Event, KeyCode},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use mljcl::{
    charts,
    history::{scrobbles_async, Scrobble},
    range::Range,
    MalojaCredentials,
};
use ratatui::{prelude::*, widgets::*};
use std::{
    io::{self, stdout},
    time::Duration,
};

mod art_backends;

use crate::art_backends::*;

struct AlbumCharts {
    albums: Vec<(String, String, u64, Option<AlbumArt>)>, // ID, Name, Rank
}

struct App {
    items: Vec<Scrobble>,
    albums: AlbumCharts,
    receiver: crossbeam::channel::Receiver<AlbumArt>,
}

#[tokio::main]
pub async fn main() -> io::Result<()> {
    let matches = Command::new("mljtui")
        .arg(
            Arg::new("ip")
                .short('i')
                .value_name("IP")
                .help("Maloja API IP"),
        )
        .arg(
            Arg::new("port")
                .short('p')
                .value_name("PORT")
                .help("Maloja API Port"),
        )
        .arg(
            Arg::new("https")
                .short('s')
                .value_name("HTTPS")
                .help("Whether to use HTTPS")
                .num_args(0),
        )
        .get_matches();

    let creds = MalojaCredentials {
        https: matches.get_flag("https"),
        skip_cert_verification: matches.get_flag("https"),
        ip: matches
            .get_one::<String>("ip")
            .expect("IP required")
            .to_string(),
        port: matches
            .get_one::<String>("port")
            .unwrap_or(&"42010".to_string())
            .parse::<u16>()
            .expect("Port must be an integer between 1-65535"),
        api_key: None,
    };

    let client = mljcl::get_client_async(&creds);
    let mut recent_scrobbles = scrobbles_async(
        None,
        Range::AllTime,
        Some(0),
        Some(30),
        creds.clone(),
        client.clone(),
    )
    .await
    .unwrap();
    recent_scrobbles.reverse();

    let (s, r) = unbounded();

    let mut album_charts =
        charts::charts_albums_async(Range::AllTime, None, creds.clone(), client.clone())
            .await
            .unwrap();
    let mut albums: Vec<(String, String, u64, Option<AlbumArt>)> = vec![];

    album_charts.albums.truncate(4);
    for album in album_charts.albums {
        albums.push((album.clone().0.id, album.clone().0.name, album.1, None));
        let album_client = client.clone();
        let album_creds = creds.clone();
        let s_clone = s.clone();
        tokio::spawn(async move {
            art_backends::get_art_for(album, s_clone, album_creds, album_client).await;
        });
    }

    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    let mut app = App {
        items: recent_scrobbles,
        albums: AlbumCharts { albums },
        receiver: r,
    };
    let mut should_quit = false;
    while !should_quit {
        terminal.draw(|f| ui(f, &mut app))?;
        should_quit = handle_events()?;
    }

    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}

fn handle_events() -> io::Result<bool> {
    if event::poll(std::time::Duration::from_millis(50))? {
        if let Event::Key(key) = event::read()? {
            if key.kind == event::KeyEventKind::Press && key.code == KeyCode::Char('q') {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn ui(frame: &mut Frame, app: &mut App) {
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(frame.size());

    let halves = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(main_layout[1]);

    frame.render_widget(
        Block::default().borders(Borders::ALL).title("Charts"),
        halves[0],
    );

    let chart_sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(33),
            Constraint::Percentage(34),
        ])
        .split(halves[0]);

    let new_sec = ratatui::layout::Rect::new(
        chart_sections[0].x + 1,
        chart_sections[0].y + 1,
        chart_sections[0].width - 2,
        chart_sections[0].height + 1,
    );

    frame.render_widget(
        Block::default().borders(Borders::ALL).title(format!("Top Albums {}/4", app.albums.albums.len())),
        new_sec,
    );

    let top_albums = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(10),
            Constraint::Percentage(10),
            Constraint::Percentage(10),
            Constraint::Percentage(10),
        ])
        .split(new_sec);

    let mut album_boxes: Vec<Rect> = vec![];

    for i in 0..=3 {
        album_boxes.push(ratatui::layout::Rect::new(
            top_albums[i].x + 1 + (12u16 * i as u16),
            top_albums[0].y,
            16,
            10,
        ));
    }

    let mut album_num = 0;
    for album in album_boxes {
        let text = match app.albums.albums.get(album_num) {
            Some(album) => match &album.3 {
                Some(art) => art.display_string(),
                None => truncate(album.clone().1, 3),
            },
            None => "".to_string(),
        };

        let text = text.into_text().unwrap();
        let paragraph = Paragraph::new(text);

        let block = Block::new().borders(Borders::NONE).title("");
        frame.render_widget(paragraph.clone().block(block), album);
        album_num += 1;
    }

    let history_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(halves[1]);

    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .title("Last Scrobbles"),
        history_layout[0],
    );

    let scrobbles: Vec<ListItem> = app
        .items
        .iter()
        .rev()
        .map(|entry| {
            // TODO: Change the date into something more like "119 minutes ago"
            let date: String = format!("{}", entry.time.format("%Y/%d/%m %H:%M"));
            let header = Line::from(vec![
                Span::styled(date, Style::new().gray().italic()),
                "  ".into(),
                Span::styled(entry.track.artists[0].clone(), Style::new().gray()),
                " -- ".into(),
                Span::styled(entry.track.name.clone(), Style::new().white()),
            ]);

            ListItem::new(vec![header])
        })
        .collect();

    let events_list = List::new(scrobbles)
        .block(Block::default().borders(Borders::ALL).title("List"))
        .start_corner(Corner::TopLeft);
    frame.render_widget(events_list, history_layout[0]);

    frame.render_widget(
        Block::default().borders(Borders::ALL).title("Pulse"),
        history_layout[1],
    );

    let recv_result = app.receiver.recv_timeout(Duration::from_micros(1000));
    if let Ok(art) = recv_result {
        for album in &mut app.albums.albums {
            if album.0 == art.clone().album_id {
                album.3 = Some(art.clone());
            }
        }
    }
}
