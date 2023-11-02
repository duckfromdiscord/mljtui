use mljcl::{MalojaCredentials, history::{Scrobble, scrobbles}};
use clap::{Arg, Command};
use std::io::{self, stdout};
use crossterm::{
    event::{self, Event, KeyCode},
    ExecutableCommand,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen}
};
use ratatui::{prelude::*, widgets::*};

struct App {
    items: Vec<Scrobble>,
}

pub fn main() -> io::Result<()> {
    let matches = Command::new("mljtui")
    .arg(
        Arg::new("ip")
        .short('i')
        .value_name("IP")
        .help("Maloja API IP")
    )
    .arg(
        Arg::new("port")
        .short('p')
        .value_name("PORT")
        .help("Maloja API Port")
    )
    .arg(
        Arg::new("https")
        .short('s')
        .value_name("HTTPS")
        .help("Whether to use HTTPS")
        .num_args(0)
    )
    .get_matches();

    let creds = MalojaCredentials {
        https: matches.get_flag("https"),
        skip_cert_verification: matches.get_flag("https"),
        ip: matches.get_one::<String>("ip").expect("IP required").to_string(),
        port: matches.get_one::<String>("port").unwrap_or(&"42010".to_string()).parse::<u16>().expect("Port must be an integer between 1-65535"),
        api_key: None,
    };

    let mut recent_scrobbles = scrobbles(None, mljcl::range::Range::AllTime, Some(0), Some(30), creds).unwrap();
    recent_scrobbles.reverse();

    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    let mut app = App {
        items: recent_scrobbles,
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

    let history_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(halves[1]);
    
    

    frame.render_widget(
        Block::default().borders(Borders::ALL).title("Last Scrobbles"),
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

        ListItem::new(vec![
            header,
        ])
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
    

    
}