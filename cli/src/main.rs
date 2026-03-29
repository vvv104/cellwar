use std::collections::{HashMap, HashSet};
use std::io::{stdout, BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute, queue,
    style::{
        Attribute, Color, Print, ResetColor, SetAttribute, SetBackgroundColor,
        SetForegroundColor,
    },
    terminal::{self, Clear, ClearType},
};

use cellwar_engine::{
    apply_action, end_player_turn, get_valid_actions, get_winner, new_game, remove_player,
    Action, Cell, GameConfig, GameState, Position, UnitType,
};

// ─── Raw mode guard ───────────────────────────────────────────────────────

struct RawModeGuard;

impl RawModeGuard {
    fn enter() -> Self {
        terminal::enable_raw_mode().expect("Failed to enable raw mode");
        execute!(stdout(), Hide).unwrap();
        RawModeGuard
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
        let _ = execute!(stdout(), Show);
    }
}

// ─── Visual helpers ───────────────────────────────────────────────────────

fn unit_symbol(unit_type: &UnitType) -> String {
    match unit_type {
        UnitType::Infantry => "♟".to_string(),
        UnitType::Fortification { turns_standing } => format!("▲{}", turns_standing),
        UnitType::Factory => "⚙".to_string(),
    }
}

fn player_fg(player_id: u8) -> Color {
    match player_id {
        1 => Color::Blue,
        2 => Color::Red,
        3 => Color::Green,
        4 => Color::Yellow,
        _ => Color::Magenta,
    }
}

fn vector_arrow(dx: i32, dy: i32) -> &'static str {
    match (dx, dy) {
        (-1, -1) => "↖", (0, -1) => "↑", (1, -1) => "↗",
        (-1,  0) => "←", (0,  0) => "·", (1,  0) => "→",
        (-1,  1) => "↙", (0,  1) => "↓", (1,  1) => "↘",
        _ => "?",
    }
}

// ─── Game helpers ─────────────────────────────────────────────────────────

/// Combat units first; factories only when all combat units have acted.
fn get_available_units(state: &GameState, player_id: u8) -> Vec<u32> {
    let mut combat: Vec<u32> = state
        .units
        .values()
        .filter(|u| {
            u.owner == player_id
                && !u.acted
                && !matches!(u.unit_type, UnitType::Factory)
        })
        .map(|u| u.id)
        .collect();
    if !combat.is_empty() {
        combat.sort();
        return combat;
    }
    let mut factories: Vec<u32> = state
        .units
        .values()
        .filter(|u| u.owner == player_id && !u.acted && matches!(u.unit_type, UnitType::Factory))
        .map(|u| u.id)
        .collect();
    factories.sort();
    factories
}

/// Valid (dx,dy) pairs for a unit as a set.
fn valid_vectors(state: &GameState, player_id: u8, unit_id: u32) -> HashSet<(i32, i32)> {
    get_valid_actions(state, player_id, unit_id)
        .into_iter()
        .map(|a| (a.dx, a.dy))
        .collect()
}

/// Navigate the 3×3 vector grid one step in `arrow` direction,
/// skipping invalid positions. Returns the current vector if no valid
/// position exists in that direction.
fn navigate_3x3(
    current: (i32, i32),
    arrow: (i32, i32),
    valid: &HashSet<(i32, i32)>,
) -> (i32, i32) {
    let (ax, ay) = arrow;
    let (mut cx, mut cy) = current;
    loop {
        let nx = cx + ax;
        let ny = cy + ay;
        if nx < -1 || nx > 1 || ny < -1 || ny > 1 {
            return current;
        }
        if valid.contains(&(nx, ny)) {
            return (nx, ny);
        }
        cx = nx;
        cy = ny;
    }
}

// ─── Rendering ────────────────────────────────────────────────────────────

/// Width of one cell in terminal columns.
const CELL_W: usize = 4;

fn render(
    state: &GameState,
    current_player: u8,
    selected_id: Option<u32>,
    vector: Option<(i32, i32)>,
    in_vector_mode: bool,
) {
    let out = &mut stdout();
    let w = state.config.width;
    let h = state.config.height;

    let sel_pos: Option<Position> = selected_id
        .and_then(|id| state.units.get(&id))
        .map(|u| u.position.clone());

    let target_pos: Option<Position> = vector.and_then(|(dx, dy)| {
        sel_pos.as_ref().map(|p| Position {
            x: (p.x as i64 + dx as i64) as u32,
            y: (p.y as i64 + dy as i64) as u32,
        })
    });

    execute!(out, Clear(ClearType::All), MoveTo(0, 0)).unwrap();

    // ── Header ────────────────────────────────────────────────────────────
    queue!(
        out,
        SetForegroundColor(player_fg(current_player)),
        SetAttribute(Attribute::Bold),
        Print(format!("Round {}  │  Player {}'s turn", state.round, current_player)),
        SetAttribute(Attribute::Reset),
        ResetColor,
        Print("\r\n\r\n"),
    )
    .unwrap();

    // ── Column numbers ────────────────────────────────────────────────────
    queue!(out, Print("    ")).unwrap();
    for x in 0..w {
        queue!(out, SetAttribute(Attribute::Dim), Print(format!("{:^4}", x)), SetAttribute(Attribute::Reset)).unwrap();
    }
    queue!(out, Print("\r\n")).unwrap();

    // ── Grid ──────────────────────────────────────────────────────────────
    for y in 0..h {
        queue!(out, SetAttribute(Attribute::Dim), Print(format!("{:>2}  ", y)), SetAttribute(Attribute::Reset)).unwrap();

        for x in 0..w {
            let pos = Position { x, y };
            let is_sel = sel_pos.as_ref().map(|p| *p == pos).unwrap_or(false);
            let is_tgt = target_pos.as_ref().map(|p| *p == pos).unwrap_or(false);

            // Background for selected / target cells
            if is_sel {
                queue!(
                    out,
                    SetBackgroundColor(Color::Grey),
                    SetForegroundColor(Color::Black),
                    SetAttribute(Attribute::Bold),
                )
                .unwrap();
            } else if is_tgt {
                queue!(
                    out,
                    SetBackgroundColor(Color::DarkYellow),
                    SetForegroundColor(Color::Black),
                    SetAttribute(Attribute::Bold),
                )
                .unwrap();
            }

            match &state.cells[y as usize][x as usize] {
                Cell::Empty => {
                    queue!(out, Print(format!("{:width$}", "·", width = CELL_W))).unwrap();
                }
                Cell::Unit(uid) => {
                    let u = &state.units[uid];
                    if !is_sel && !is_tgt {
                        queue!(out, SetForegroundColor(player_fg(u.owner))).unwrap();
                        if u.acted {
                            queue!(out, SetAttribute(Attribute::Dim)).unwrap();
                        }
                    }
                    let text = format!("{}{}", u.owner, unit_symbol(&u.unit_type));
                    // pad to CELL_W terminal columns
                    queue!(out, Print(format!("{:<width$}", text, width = CELL_W))).unwrap();
                }
            }

            // Reset after each cell
            queue!(out, SetAttribute(Attribute::Reset), ResetColor).unwrap();
        }
        queue!(out, Print("\r\n")).unwrap();
    }

    // ── Status bar ────────────────────────────────────────────────────────
    queue!(out, Print("\r\n")).unwrap();

    if let (Some(id), Some(unit)) = (selected_id, selected_id.and_then(|id| state.units.get(&id))) {
        let _ = id;
        let type_name = match &unit.unit_type {
            UnitType::Infantry => "Infantry".to_string(),
            UnitType::Fortification { turns_standing } => {
                format!("Fortification [{}]", turns_standing)
            }
            UnitType::Factory => "Factory".to_string(),
        };

        queue!(
            out,
            SetForegroundColor(player_fg(current_player)),
            SetAttribute(Attribute::Bold),
            Print(format!(
                "▶ {} at ({},{})",
                type_name, unit.position.x, unit.position.y
            )),
            SetAttribute(Attribute::Reset),
            ResetColor,
        )
        .unwrap();

        if in_vector_mode {
            if let Some((dx, dy)) = vector {
                let arrow = vector_arrow(dx, dy);

                let target_desc = if dx == 0 && dy == 0 {
                    match &unit.unit_type {
                        UnitType::Infantry => "Stay → become Fortification".to_string(),
                        UnitType::Fortification { turns_standing } => {
                            if *turns_standing >= 3 {
                                "Stay → become Factory".to_string()
                            } else {
                                format!("Stay → counter {}", turns_standing + 1)
                            }
                        }
                        UnitType::Factory => "—".to_string(),
                    }
                } else {
                    target_pos
                        .as_ref()
                        .map(|tp| match &state.cells[tp.y as usize][tp.x as usize] {
                            Cell::Empty => "Empty".to_string(),
                            Cell::Unit(tid) => {
                                let tu = &state.units[tid];
                                match &tu.unit_type {
                                    UnitType::Infantry => {
                                        format!("P{} Infantry → destroy", tu.owner)
                                    }
                                    UnitType::Fortification { .. } => {
                                        format!("P{} Fortification → 1st hit (attacker dies)", tu.owner)
                                    }
                                    UnitType::Factory => {
                                        format!("P{} Factory → destroy", tu.owner)
                                    }
                                }
                            }
                        })
                        .unwrap_or_default()
                };

                queue!(
                    out,
                    Print(format!("   {} ({},{})  │  {}", arrow, dx, dy, target_desc)),
                )
                .unwrap();
            }
        }
    }

    queue!(out, Print("\r\n")).unwrap();

    // Key hint line
    let hints = if in_vector_mode {
        "  [↑↓←→] Move vector   [Enter] Confirm   [Esc] Cancel"
    } else {
        "  [Tab / ⇧Tab] Select unit   [↑↓←→] Set direction   [q] Quit"
    };
    queue!(
        out,
        SetAttribute(Attribute::Dim),
        Print(hints),
        SetAttribute(Attribute::Reset),
        Print("\r\n"),
    )
    .unwrap();

    out.flush().unwrap();
}

// ─── Human turn (raw mode) ────────────────────────────────────────────────

fn human_turn(state: &mut GameState, player_id: u8) {
    let _raw = RawModeGuard::enter();

    let mut sel_idx: usize = 0;
    let mut vector: Option<(i32, i32)> = None;
    let mut in_vector_mode = false;

    loop {
        let available = get_available_units(state, player_id);
        if available.is_empty() {
            break;
        }
        sel_idx = sel_idx.min(available.len() - 1);
        let sel_id = available[sel_idx];
        // Compute valid moves once per iteration, reuse in Enter and arrow branches
        let vset = valid_vectors(state, player_id, sel_id);

        render(state, player_id, Some(sel_id), vector, in_vector_mode);

        let event = match event::read() {
            Ok(e) => e,
            Err(_) => break,
        };

        // Only handle key-down events; ignore Repeat and Release to prevent
        // a single keypress from triggering multiple vector moves.
        let key = match event {
            Event::Key(k) if k.kind == KeyEventKind::Press => k,
            _ => continue,
        };

        // Ctrl+C — quit
        if key.code == KeyCode::Char('c')
            && key.modifiers.contains(KeyModifiers::CONTROL)
        {
            drop(_raw);
            std::process::exit(0);
        }

        match key.code {
            // ── Quit ───────────────────────────────────────────────────
            KeyCode::Char('q') => {
                drop(_raw);
                std::process::exit(0);
            }

            // ── Cycle units ────────────────────────────────────────────
            KeyCode::Tab if !in_vector_mode => {
                sel_idx = (sel_idx + 1) % available.len();
                vector = None;
            }
            KeyCode::BackTab if !in_vector_mode => {
                sel_idx = if sel_idx == 0 {
                    available.len() - 1
                } else {
                    sel_idx - 1
                };
                vector = None;
            }

            // ── Cancel vector ──────────────────────────────────────────
            KeyCode::Esc => {
                vector = None;
                in_vector_mode = false;
            }

            // ── Confirm action ─────────────────────────────────────────
            // If no vector was set, default to (0,0) — valid for combat units.
            KeyCode::Enter => {
                let (dx, dy) = vector.unwrap_or((0, 0));
                if vset.contains(&(dx, dy)) {
                    if apply_action(
                        state,
                        player_id,
                        sel_id,
                        Action { unit_id: sel_id, dx, dy },
                    )
                    .is_ok()
                    {
                        vector = None;
                        in_vector_mode = false;
                    }
                }
            }

            // ── Arrow keys ─────────────────────────────────────────────
            KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right => {
                let arrow: (i32, i32) = match key.code {
                    KeyCode::Up => (0, -1),
                    KeyCode::Down => (0, 1),
                    KeyCode::Left => (-1, 0),
                    KeyCode::Right => (1, 0),
                    _ => unreachable!(),
                };

                if !in_vector_mode {
                    // First press: jump directly to that direction if valid
                    if vset.contains(&arrow) {
                        vector = Some(arrow);
                        in_vector_mode = true;
                    }
                } else if let Some(cur) = vector {
                    let new_vec = navigate_3x3(cur, arrow, &vset);
                    vector = Some(new_vec);
                }
            }

            _ => {}
        }
    }
    // _raw drops here → terminal restored
}

// ─── Python bot ───────────────────────────────────────────────────────────

struct PythonBot {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl PythonBot {
    fn start(cmd: &str) -> Self {
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        let mut child = Command::new(parts[0])
            .args(&parts[1..])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("Failed to start bot process");
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        PythonBot { child, stdin, stdout }
    }

    fn send(&mut self, msg: &serde_json::Value) {
        let s = serde_json::to_string(msg).unwrap();
        writeln!(self.stdin, "{}", s).unwrap();
        self.stdin.flush().unwrap();
    }

    fn recv(&mut self) -> serde_json::Value {
        let mut line = String::new();
        self.stdout.read_line(&mut line).unwrap();
        serde_json::from_str(line.trim()).expect("Bot sent invalid JSON")
    }
}

impl Drop for PythonBot {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

fn build_state_view(state: &GameState, player_id: u8) -> serde_json::Value {
    let units: Vec<serde_json::Value> = state
        .units
        .values()
        .filter(|u| u.owner == player_id)
        .map(|u| {
            let valid: Vec<serde_json::Value> = get_valid_actions(state, player_id, u.id)
                .iter()
                .map(|a| serde_json::json!({"dx": a.dx, "dy": a.dy}))
                .collect();
            let type_str = match &u.unit_type {
                UnitType::Infantry => "Infantry".to_string(),
                UnitType::Fortification { turns_standing } => {
                    format!("Fortification({})", turns_standing)
                }
                UnitType::Factory => "Factory".to_string(),
            };
            serde_json::json!({
                "id": u.id,
                "type": type_str,
                "x": u.position.x,
                "y": u.position.y,
                "acted": u.acted,
                "valid_actions": valid,
            })
        })
        .collect();

    serde_json::json!({
        "type": "state",
        "view": {
            "player_id": player_id,
            "current_player": state.players[state.current_player_index],
            "round": state.round,
            "width": state.config.width,
            "height": state.config.height,
            "my_units": units,
            "winner": get_winner(state),
        }
    })
}

fn bot_turn(state: &mut GameState, player_id: u8, bot: &mut PythonBot) {
    loop {
        let all_acted = state
            .units
            .values()
            .filter(|u| u.owner == player_id)
            .all(|u| u.acted);
        if all_acted {
            break;
        }

        let view = build_state_view(state, player_id);
        bot.send(&view);

        let msg = bot.recv();
        match msg["type"].as_str().unwrap_or("") {
            "action" => {
                let uid = msg["unit_id"].as_u64().unwrap() as u32;
                let dx = msg["dx"].as_i64().unwrap() as i32;
                let dy = msg["dy"].as_i64().unwrap() as i32;
                match apply_action(state, player_id, uid, Action { unit_id: uid, dx, dy }) {
                    Ok(()) => println!(
                        "  Bot (player {}): unit {} → ({},{})",
                        player_id, uid, dx, dy
                    ),
                    Err(e) => println!("  Bot error: {}", e),
                }
            }
            "end_turn" => break,
            other => println!("  Unknown bot message: {}", other),
        }
    }
}

// ─── Setup ────────────────────────────────────────────────────────────────

enum PlayerKind {
    Human,
    Bot(String),
}

fn prompt(msg: &str) -> String {
    print!("{}", msg);
    std::io::stdout().flush().unwrap();
    let mut line = String::new();
    std::io::stdin().read_line(&mut line).unwrap();
    line.trim().to_string()
}

fn setup() -> (GameState, Vec<PlayerKind>) {
    println!("=== CellWar CLI ===");

    let map_input = prompt("Map size (NxM): ");
    let dims: Vec<u32> = map_input
        .split('x')
        .filter_map(|s| s.trim().parse().ok())
        .collect();
    let (width, height) = if dims.len() == 2 {
        (dims[0], dims[1])
    } else {
        println!("Invalid size, using 10x10");
        (10, 10)
    };

    let n_str = prompt("Number of players: ");
    let n: u8 = n_str.trim().parse().unwrap_or(2).max(2);

    let mut kinds = Vec::new();
    for i in 1..=n {
        let choice = prompt(&format!("Player {}: [H]uman / [P]ython bot? ", i));
        if choice.eq_ignore_ascii_case("p") {
            let cmd = prompt("  Bot command: ");
            kinds.push(PlayerKind::Bot(cmd));
        } else {
            kinds.push(PlayerKind::Human);
        }
    }

    let config = GameConfig { width, height, player_count: n };
    let state = new_game(config);
    (state, kinds)
}

// ─── Main loop ────────────────────────────────────────────────────────────

fn main() {
    let (mut state, kinds) = setup();

    let mut bots: HashMap<u8, PythonBot> = HashMap::new();
    for (i, kind) in kinds.iter().enumerate() {
        let player_id = (i + 1) as u8;
        if let PlayerKind::Bot(cmd) = kind {
            println!("Starting bot for player {}...", player_id);
            bots.insert(player_id, PythonBot::start(cmd));
        }
    }

    println!();

    loop {
        if let Some(winner) = get_winner(&state) {
            println!("\n=== Player {} wins! ===\n", winner);
            break;
        }
        if state.players.is_empty() {
            println!("No players left.");
            break;
        }

        let current = state.players[state.current_player_index];

        if bots.contains_key(&current) {
            println!("--- Round {} | Player {}'s turn (bot) ---", state.round, current);
            let bot = bots.get_mut(&current).unwrap();
            bot_turn(&mut state, current, bot);
        } else {
            human_turn(&mut state, current);
        }

        match end_player_turn(&mut state, current) {
            Ok(()) => {}
            Err(_) => {
                // Force-mark all units as acted and retry
                let ids: Vec<u32> = state
                    .units
                    .values()
                    .filter(|u| u.owner == current)
                    .map(|u| u.id)
                    .collect();
                for id in ids {
                    state.units.get_mut(&id).unwrap().acted = true;
                }
                let _ = end_player_turn(&mut state, current);
            }
        }

        if let Some(winner) = get_winner(&state) {
            println!("\n=== Player {} wins! ===\n", winner);
            break;
        }

        // Remove players with no units left
        let dead: Vec<u8> = state
            .players
            .iter()
            .copied()
            .filter(|&p| !state.units.values().any(|u| u.owner == p))
            .collect();
        for p in dead {
            println!("Player {} has no units, removing.", p);
            remove_player(&mut state, p);
        }
    }
}
