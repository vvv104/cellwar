use std::collections::HashMap;
use std::io::{self, BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use cellwar_engine::{
    apply_action, end_player_turn, get_valid_actions, get_winner, new_game, remove_player,
    Action, Cell, GameConfig, GameState, Position, UnitType,
};

// ─── ANSI цвета ───────────────────────────────────────────────────────────

const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";

fn player_color(player_id: u8) -> &'static str {
    match player_id {
        1 => "\x1b[34m", // синий
        2 => "\x1b[31m", // красный
        3 => "\x1b[32m", // зелёный
        4 => "\x1b[33m", // жёлтый
        _ => "\x1b[35m", // фиолетовый
    }
}

// ─── Отображение поля ─────────────────────────────────────────────────────

fn render_board(state: &GameState, current_player: u8) {
    let w = state.config.width as usize;
    let h = state.config.height as usize;

    // Заголовок с номерами столбцов
    print!("   ");
    for x in 0..w {
        print!("{:>3}", x);
    }
    println!();

    for y in 0..h {
        print!("{:>2} ", y);
        for x in 0..w {
            let cell_str = match &state.cells[y][x] {
                Cell::Empty => format!("{DIM}.  {RESET}"),
                Cell::Unit(uid) => {
                    let unit = &state.units[uid];
                    let color = player_color(unit.owner);
                    let type_char = match &unit.unit_type {
                        UnitType::Infantry => "I".to_string(),
                        UnitType::Fortification { turns_standing } => {
                            format!("F{}", turns_standing)
                        }
                        UnitType::Factory => "A".to_string(),
                    };
                    let acted_mark = if unit.acted { "*" } else { " " };
                    format!("{color}{}{}{type_char}{acted_mark}{RESET}", unit.owner, BOLD)
                }
            };
            print!("{}", cell_str);
        }
        println!();
    }
    println!();

    // Список юнитов текущего игрока
    let color = player_color(current_player);
    print!("{}Player {} units:{RESET} ", color, current_player);
    let mut units: Vec<_> = state
        .units
        .values()
        .filter(|u| u.owner == current_player)
        .collect();
    units.sort_by_key(|u| u.id);
    for u in &units {
        let type_str = match &u.unit_type {
            UnitType::Infantry => "I".to_string(),
            UnitType::Fortification { turns_standing } => format!("F{}", turns_standing),
            UnitType::Factory => "A".to_string(),
        };
        let acted = if u.acted { "*" } else { "" };
        print!("  {}{}@({},{}) ", type_str, acted, u.position.x, u.position.y);
    }
    println!("\n");
}

// ─── Чтение строки с подсказкой ───────────────────────────────────────────

fn prompt(msg: &str) -> String {
    print!("{}", msg);
    io::stdout().flush().unwrap();
    let mut line = String::new();
    io::stdin().read_line(&mut line).unwrap();
    line.trim().to_string()
}

// ─── Ход человека ─────────────────────────────────────────────────────────

fn human_turn(state: &mut GameState, player_id: u8) {
    loop {
        // Проверяем: все ли боевые сходили?
        let combat_unacted: Vec<_> = state
            .units
            .values()
            .filter(|u| {
                u.owner == player_id
                    && !u.acted
                    && !matches!(u.unit_type, UnitType::Factory)
            })
            .map(|u| u.id)
            .collect();

        let factories_unacted: Vec<_> = state
            .units
            .values()
            .filter(|u| {
                u.owner == player_id
                    && !u.acted
                    && matches!(u.unit_type, UnitType::Factory)
            })
            .map(|u| u.id)
            .collect();

        // Если всё сходило — конец хода
        if combat_unacted.is_empty() && factories_unacted.is_empty() {
            break;
        }

        // Если боевые ещё не все сходили — предлагаем только боевые
        let available_ids = if !combat_unacted.is_empty() {
            combat_unacted.clone()
        } else {
            // Все боевые сходили — теперь фабрики
            println!("All combat units done. Factories:");
            for fid in &factories_unacted {
                let u = &state.units[fid];
                println!(
                    "  Factory@({},{})",
                    u.position.x, u.position.y
                );
            }
            factories_unacted.clone()
        };

        // Показываем доступные юниты
        println!("Units available to move:");
        let mut sorted = available_ids.clone();
        sorted.sort();
        for uid in &sorted {
            let u = &state.units[uid];
            let type_str = match &u.unit_type {
                UnitType::Infantry => "Infantry".to_string(),
                UnitType::Fortification { turns_standing } => {
                    format!("Fortification[{}]", turns_standing)
                }
                UnitType::Factory => "Factory".to_string(),
            };
            println!("  {} at ({},{})", type_str, u.position.x, u.position.y);
        }

        let input = prompt("Select unit (x,y) or 'q' to quit: ");
        if input == "q" {
            println!("Quitting.");
            std::process::exit(0);
        }

        // Парсим координаты
        let coords: Vec<i64> = input
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();
        if coords.len() != 2 {
            println!("Invalid input, use x,y");
            continue;
        }
        let (tx, ty) = (coords[0] as u32, coords[1] as u32);

        // Ищем юнит на этой позиции среди доступных
        let uid = match available_ids.iter().find(|&&id| {
            let u = &state.units[&id];
            u.position == (Position { x: tx, y: ty })
        }) {
            Some(&id) => id,
            None => {
                println!("No available unit at ({},{})", tx, ty);
                continue;
            }
        };

        let unit = &state.units[&uid];
        let type_str = match &unit.unit_type {
            UnitType::Infantry => "Infantry".to_string(),
            UnitType::Fortification { turns_standing } => {
                format!("Fortification[{}]", turns_standing)
            }
            UnitType::Factory => "Factory".to_string(),
        };
        println!("Unit: {} at ({},{})", type_str, unit.position.x, unit.position.y);

        // Показываем допустимые ходы
        let valid = get_valid_actions(state, player_id, uid);
        let moves_str: Vec<String> = valid
            .iter()
            .map(|a| format!("({},{})", a.dx, a.dy))
            .collect();
        println!("Valid moves: {}", moves_str.join(" "));

        let mv = prompt("Enter move (dx,dy): ");
        let parts: Vec<i32> = mv
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();
        if parts.len() != 2 {
            println!("Invalid move format");
            continue;
        }
        let (dx, dy) = (parts[0], parts[1]);

        // Проверяем что ход допустим
        if !valid.iter().any(|a| a.dx == dx && a.dy == dy) {
            println!("Invalid move ({},{})", dx, dy);
            continue;
        }

        match apply_action(state, player_id, uid, Action { unit_id: uid, dx, dy }) {
            Ok(()) => {
                render_board(state, player_id);
            }
            Err(e) => println!("Error: {}", e),
        }
    }
}

// ─── Python-бот ──────────────────────────────────────────────────────────

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
    use cellwar_engine::get_valid_actions;

    let units: Vec<serde_json::Value> = state
        .units
        .values()
        .filter(|u| u.owner == player_id)
        .map(|u| {
            let valid = get_valid_actions(state, player_id, u.id);
            let valid_json: Vec<serde_json::Value> = valid
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
                "valid_actions": valid_json,
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
                    Ok(()) => {
                        println!(
                            "  Bot(player {}): unit {} moved ({},{})",
                            player_id, uid, dx, dy
                        );
                    }
                    Err(e) => {
                        println!("  Bot error: {}", e);
                        // Пробуем продолжить — отправляем состояние заново
                    }
                }
            }
            "end_turn" => break,
            other => println!("  Bot sent unknown message type: {}", other),
        }
    }
}

// ─── Настройка игры ───────────────────────────────────────────────────────

enum PlayerKind {
    Human,
    Bot(String), // команда запуска
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

// ─── Главный цикл ─────────────────────────────────────────────────────────

fn main() {
    let (mut state, kinds) = setup();

    // Запускаем боты
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
            println!("{}{}=== Player {} wins! ==={RESET}", player_color(winner), BOLD, winner);
            break;
        }

        if state.players.is_empty() {
            println!("No players left.");
            break;
        }

        let current = state.players[state.current_player_index];
        let color = player_color(current);
        println!(
            "{}{}=== Round {} | Player {}'s turn ==={RESET}",
            color, BOLD, state.round, current
        );

        render_board(&state, current);

        if bots.contains_key(&current) {
            let bot = bots.get_mut(&current).unwrap();
            bot_turn(&mut state, current, bot);
        } else {
            human_turn(&mut state, current);
        }

        // Завершаем ход
        match end_player_turn(&mut state, current) {
            Ok(()) => {}
            Err(e) => {
                // Не все юниты сходили (не должно случиться при правильном human_turn)
                println!("end_turn error: {}", e);
                // Принудительно завершаем: пометим всех как сходивших
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

        // Проверяем победителя после хода
        if let Some(winner) = get_winner(&state) {
            println!("{}{}=== Player {} wins! ==={RESET}", player_color(winner), BOLD, winner);
            break;
        }

        // Если текущий игрок потерял всех юнитов — удаляем его
        let has_units = state.units.values().any(|u| u.owner == current);
        if !has_units && state.players.contains(&current) {
            println!("Player {} has no units left, removing.", current);
            remove_player(&mut state, current);
        }
    }
}
