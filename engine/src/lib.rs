use std::collections::HashMap;
use serde::{Deserialize, Serialize};

// Координата на поле
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Position {
    pub x: u32,
    pub y: u32,
}

impl std::fmt::Display for Position {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({}, {})", self.x, self.y)
    }
}

// Тип юнита
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum UnitType {
    Infantry,
    Fortification { turns_standing: u8 }, // 1, 2, 3 (на 3 → Factory)
    Factory,
}

impl std::fmt::Display for UnitType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UnitType::Infantry => write!(f, "Infantry"),
            UnitType::Fortification { turns_standing } => {
                write!(f, "Fortification({})", turns_standing)
            }
            UnitType::Factory => write!(f, "Factory"),
        }
    }
}

// Юнит
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Unit {
    pub id: u32,
    pub owner: u8,       // player_id
    pub unit_type: UnitType,
    pub position: Position,
    pub acted: bool,     // сходил ли в этом ходу
}

impl std::fmt::Display for Unit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Unit(id={}, owner={}, type={}, pos={}, acted={})",
            self.id, self.owner, self.unit_type, self.position, self.acted
        )
    }
}

// Действие юнита — всегда вектор
// (0,0) = стоять, (dx,dy) = двигаться/атаковать/производить
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Action {
    pub unit_id: u32,
    pub dx: i32, // -1, 0, 1
    pub dy: i32, // -1, 0, 1
}

impl std::fmt::Display for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Action(unit={}, dx={}, dy={})", self.unit_id, self.dx, self.dy)
    }
}

// Конфигурация игры
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameConfig {
    pub width: u32,
    pub height: u32,
    pub player_count: u8,
}

impl std::fmt::Display for GameConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "GameConfig({}x{}, {} players)",
            self.width, self.height, self.player_count
        )
    }
}

// Клетка поля
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Cell {
    Empty,
    Unit(u32), // unit_id
}

impl std::fmt::Display for Cell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Cell::Empty => write!(f, "Empty"),
            Cell::Unit(id) => write!(f, "Unit({})", id),
        }
    }
}

// Снапшот для rollback
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub player_id: u8,
    pub round: u32,
    pub cells: Vec<Vec<Cell>>,
    pub units: HashMap<u32, Unit>,
}

// Полное состояние игры (только для сервера)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameState {
    pub config: GameConfig,
    pub cells: Vec<Vec<Cell>>,         // [y][x]
    pub units: HashMap<u32, Unit>,
    pub next_unit_id: u32,
    pub players: Vec<u8>,              // активные игроки в порядке хода
    pub current_player_index: usize,
    pub round: u32,
    pub snapshots: Vec<Snapshot>,      // снапшоты после каждого хода игрока
}

// Видимое состояние для игрока (с туманом войны)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TileVisibility {
    Visible(Cell),   // видно прямо сейчас
    LastKnown(Cell), // последнее известное состояние
    Fog,             // неизвестно
}

impl std::fmt::Display for TileVisibility {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TileVisibility::Visible(cell) => write!(f, "Visible({})", cell),
            TileVisibility::LastKnown(cell) => write!(f, "LastKnown({})", cell),
            TileVisibility::Fog => write!(f, "Fog"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerView {
    pub player_id: u8,
    pub width: u32,
    pub height: u32,
    pub tiles: Vec<Vec<TileVisibility>>,
    pub my_units: Vec<Unit>,
    pub current_player: u8,
    pub round: u32,
    pub winner: Option<u8>,
}

// Ошибки
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GameError {
    InvalidAction(String),
    NotYourTurn,
    UnitAlreadyActed,
    UnitNotFound,
    GameOver,
}

impl std::fmt::Display for GameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GameError::InvalidAction(msg) => write!(f, "InvalidAction: {}", msg),
            GameError::NotYourTurn => write!(f, "NotYourTurn"),
            GameError::UnitAlreadyActed => write!(f, "UnitAlreadyActed"),
            GameError::UnitNotFound => write!(f, "UnitNotFound"),
            GameError::GameOver => write!(f, "GameOver"),
        }
    }
}

impl std::error::Error for GameError {}

// ─── Вспомогательные функции ───────────────────────────────────────────────

/// Радиус видимости юнита
fn visibility_radius(unit_type: &UnitType) -> u32 {
    match unit_type {
        UnitType::Infantry => 1,
        UnitType::Fortification { .. } => 2,
        UnitType::Factory => 1,
    }
}

/// Применить вектор к позиции; вернуть None если выходит за границы
fn apply_delta(pos: &Position, dx: i32, dy: i32, width: u32, height: u32) -> Option<Position> {
    let nx = pos.x as i64 + dx as i64;
    let ny = pos.y as i64 + dy as i64;
    if nx < 0 || ny < 0 || nx >= width as i64 || ny >= height as i64 {
        None
    } else {
        Some(Position { x: nx as u32, y: ny as u32 })
    }
}

// ─── Публичные функции движка ──────────────────────────────────────────────

/// Создаёт новую игру: расставляет по одному пехотинцу на игрока.
/// Для 2 игроков — левый верхний и правый нижний углы с отступом 1.
/// Для 3+ — равномерно по периметру поля с отступом 1.
pub fn new_game(config: GameConfig) -> GameState {
    let w = config.width;
    let h = config.height;
    let n = config.player_count as usize;

    // Стартовые позиции: равномерно по углам/периметру с отступом 1
    let positions: Vec<Position> = if n == 2 {
        vec![
            Position { x: 1, y: 1 },
            Position { x: w - 2, y: h - 2 },
        ]
    } else {
        // Равномерно по периметру (по часовой стрелке), отступ 1
        let perim_points: Vec<Position> = {
            let mut pts = Vec::new();
            // верх
            for x in 1..w - 1 { pts.push(Position { x, y: 1 }); }
            // право
            for y in 1..h - 1 { pts.push(Position { x: w - 2, y }); }
            // низ (обратно)
            for x in (1..w - 1).rev() { pts.push(Position { x, y: h - 2 }); }
            // лево (обратно)
            for y in (1..h - 1).rev() { pts.push(Position { x: 1, y }); }
            pts
        };
        let step = perim_points.len() / n;
        (0..n).map(|i| perim_points[i * step].clone()).collect()
    };

    let mut cells: Vec<Vec<Cell>> = (0..h)
        .map(|_| (0..w).map(|_| Cell::Empty).collect())
        .collect();
    let mut units: HashMap<u32, Unit> = HashMap::new();
    let mut next_unit_id = 1u32;

    for (i, pos) in positions.iter().enumerate() {
        let player_id = (i + 1) as u8;
        let unit = Unit {
            id: next_unit_id,
            owner: player_id,
            unit_type: UnitType::Infantry,
            position: pos.clone(),
            acted: false,
        };
        cells[pos.y as usize][pos.x as usize] = Cell::Unit(next_unit_id);
        units.insert(next_unit_id, unit);
        next_unit_id += 1;
    }

    let players: Vec<u8> = (1..=config.player_count).collect();

    GameState {
        config,
        cells,
        units,
        next_unit_id,
        players,
        current_player_index: 0,
        round: 1,
        snapshots: Vec::new(),
    }
}

/// Возвращает все допустимые действия для юнита.
pub fn get_valid_actions(state: &GameState, player_id: u8, unit_id: u32) -> Vec<Action> {
    let unit = match state.units.get(&unit_id) {
        Some(u) if u.owner == player_id => u,
        _ => return vec![],
    };

    let w = state.config.width;
    let h = state.config.height;
    let pos = &unit.position;

    match &unit.unit_type {
        UnitType::Infantry | UnitType::Fortification { .. } => {
            let mut actions = Vec::new();
            // (0,0) — стоять на месте (всегда допустимо для боевых юнитов)
            actions.push(Action { unit_id, dx: 0, dy: 0 });
            // 8 направлений
            for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    if dx == 0 && dy == 0 { continue; }
                    if let Some(target) = apply_delta(pos, dx, dy, w, h) {
                        let cell = &state.cells[target.y as usize][target.x as usize];
                        match cell {
                            Cell::Empty => {
                                actions.push(Action { unit_id, dx, dy });
                            }
                            Cell::Unit(tid) => {
                                if let Some(tu) = state.units.get(tid) {
                                    if tu.owner != player_id {
                                        // атака врага
                                        actions.push(Action { unit_id, dx, dy });
                                    }
                                    // свой юнит — невозможный ход, не добавляем
                                }
                            }
                        }
                    }
                }
            }
            actions
        }
        UnitType::Factory => {
            let mut actions = Vec::new();
            for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    if dx == 0 && dy == 0 { continue; }
                    if let Some(target) = apply_delta(pos, dx, dy, w, h) {
                        if state.cells[target.y as usize][target.x as usize] == Cell::Empty {
                            actions.push(Action { unit_id, dx, dy });
                        }
                    }
                }
            }
            actions
        }
    }
}

/// Вычисляет PlayerView с туманом войны для указанного игрока.
pub fn get_visible_state(state: &GameState, player_id: u8) -> PlayerView {
    let w = state.config.width;
    let h = state.config.height;

    // Собираем множество видимых клеток
    let mut visible: Vec<Vec<bool>> = (0..h)
        .map(|_| vec![false; w as usize])
        .collect();

    for unit in state.units.values() {
        if unit.owner != player_id { continue; }
        let r = visibility_radius(&unit.unit_type) as i64;
        let px = unit.position.x as i64;
        let py = unit.position.y as i64;
        for dy in -r..=r {
            for dx in -r..=r {
                let nx = px + dx;
                let ny = py + dy;
                if nx >= 0 && ny >= 0 && nx < w as i64 && ny < h as i64 {
                    visible[ny as usize][nx as usize] = true;
                }
            }
        }
    }

    let tiles: Vec<Vec<TileVisibility>> = (0..h as usize)
        .map(|y| {
            (0..w as usize)
                .map(|x| {
                    if visible[y][x] {
                        TileVisibility::Visible(state.cells[y][x].clone())
                    } else {
                        TileVisibility::Fog
                    }
                })
                .collect()
        })
        .collect();

    let my_units: Vec<Unit> = state
        .units
        .values()
        .filter(|u| u.owner == player_id)
        .cloned()
        .collect();

    let current_player = state.players[state.current_player_index];

    PlayerView {
        player_id,
        width: w,
        height: h,
        tiles,
        my_units,
        current_player,
        round: state.round,
        winner: get_winner(state),
    }
}

/// Возвращает победителя, если он есть.
pub fn get_winner(state: &GameState) -> Option<u8> {
    let active: Vec<u8> = state
        .players
        .iter()
        .filter(|&&pid| state.units.values().any(|u| u.owner == pid))
        .cloned()
        .collect();
    if active.len() == 1 {
        Some(active[0])
    } else {
        None
    }
}

// ─── Применение действий ──────────────────────────────────────────────────

/// Применяет действие юнита. Вектор (dx,dy) интерпретируется по типу юнита и
/// состоянию целевой клетки согласно правилам игры.
pub fn apply_action(
    state: &mut GameState,
    player_id: u8,
    unit_id: u32,
    action: Action,
) -> Result<(), GameError> {
    // Базовые проверки
    if get_winner(state).is_some() {
        return Err(GameError::GameOver);
    }
    if state.players[state.current_player_index] != player_id {
        return Err(GameError::NotYourTurn);
    }
    {
        let unit = state.units.get(&unit_id).ok_or(GameError::UnitNotFound)?;
        if unit.owner != player_id {
            return Err(GameError::UnitNotFound);
        }
        if unit.acted {
            return Err(GameError::UnitAlreadyActed);
        }
    }

    let dx = action.dx;
    let dy = action.dy;

    // Определяем тип юнита и его позицию
    let (unit_type, pos) = {
        let u = &state.units[&unit_id];
        (u.unit_type.clone(), u.position.clone())
    };

    match &unit_type {
        // ── Пехотинец ──────────────────────────────────────────────────────
        UnitType::Infantry => {
            if dx == 0 && dy == 0 {
                // Стоять → стать укреплением
                let unit = state.units.get_mut(&unit_id).unwrap();
                unit.unit_type = UnitType::Fortification { turns_standing: 1 };
                unit.acted = true;
            } else {
                let target = apply_delta(&pos, dx, dy, state.config.width, state.config.height)
                    .ok_or_else(|| GameError::InvalidAction("Out of bounds".into()))?;
                apply_infantry_move(state, unit_id, &pos, &target)?;
            }
        }

        // ── Укрепление ─────────────────────────────────────────────────────
        UnitType::Fortification { turns_standing } => {
            if dx == 0 && dy == 0 {
                // Стоять → счётчик +1, при 3 → фабрика
                let ts = *turns_standing;
                let unit = state.units.get_mut(&unit_id).unwrap();
                if ts >= 3 {
                    unit.unit_type = UnitType::Factory;
                } else {
                    unit.unit_type = UnitType::Fortification { turns_standing: ts + 1 };
                }
                unit.acted = true;
            } else {
                let target = apply_delta(&pos, dx, dy, state.config.width, state.config.height)
                    .ok_or_else(|| GameError::InvalidAction("Out of bounds".into()))?;
                // Сначала стать пехотинцем
                state.units.get_mut(&unit_id).unwrap().unit_type = UnitType::Infantry;
                apply_infantry_move(state, unit_id, &pos, &target)?;
            }
        }

        // ── Фабрика ────────────────────────────────────────────────────────
        UnitType::Factory => {
            if dx == 0 && dy == 0 {
                return Err(GameError::InvalidAction("Factory cannot stay".into()));
            }
            let target = apply_delta(&pos, dx, dy, state.config.width, state.config.height)
                .ok_or_else(|| GameError::InvalidAction("Out of bounds".into()))?;
            let cell = state.cells[target.y as usize][target.x as usize].clone();
            match cell {
                Cell::Empty => {
                    let new_id = state.next_unit_id;
                    state.next_unit_id += 1;
                    let new_unit = Unit {
                        id: new_id,
                        owner: player_id,
                        unit_type: UnitType::Infantry,
                        position: target.clone(),
                        acted: true, // не ходит в ход появления
                    };
                    state.cells[target.y as usize][target.x as usize] = Cell::Unit(new_id);
                    state.units.insert(new_id, new_unit);
                    state.units.get_mut(&unit_id).unwrap().acted = true;
                }
                _ => {
                    return Err(GameError::InvalidAction("Factory target cell not empty".into()));
                }
            }
        }
    }

    Ok(())
}

/// Применяет ход пехотинца (уже Infantry) от `from` к `target`.
/// Мутирует state. Возвращает ошибку если ход невозможен.
fn apply_infantry_move(
    state: &mut GameState,
    unit_id: u32,
    from: &Position,
    target: &Position,
) -> Result<(), GameError> {
    let player_id = state.units[&unit_id].owner;
    let cell = state.cells[target.y as usize][target.x as usize].clone();

    match cell {
        Cell::Empty => {
            // Перемещение
            state.cells[from.y as usize][from.x as usize] = Cell::Empty;
            state.cells[target.y as usize][target.x as usize] = Cell::Unit(unit_id);
            let unit = state.units.get_mut(&unit_id).unwrap();
            unit.position = target.clone();
            unit.acted = true;
        }
        Cell::Unit(target_id) => {
            let target_owner = state.units[&target_id].owner;
            if target_owner == player_id {
                return Err(GameError::InvalidAction("Cannot attack own unit".into()));
            }
            let target_type = state.units[&target_id].unit_type.clone();
            match target_type {
                UnitType::Infantry => {
                    // Враг-пехотинец: уничтожен, атакующий занимает клетку
                    state.units.remove(&target_id);
                    state.cells[from.y as usize][from.x as usize] = Cell::Empty;
                    state.cells[target.y as usize][target.x as usize] = Cell::Unit(unit_id);
                    let unit = state.units.get_mut(&unit_id).unwrap();
                    unit.position = target.clone();
                    unit.acted = true;
                }
                UnitType::Fortification { .. } => {
                    // Атакующий уничтожается, укрепление → пехотинец
                    state.units.remove(&unit_id);
                    state.cells[from.y as usize][from.x as usize] = Cell::Empty;
                    // Укрепление становится пехотинцем (остаётся на месте, acted не меняем)
                    state.units.get_mut(&target_id).unwrap().unit_type = UnitType::Infantry;
                }
                UnitType::Factory => {
                    // Фабрика уничтожена, атакующий занимает клетку
                    state.units.remove(&target_id);
                    state.cells[from.y as usize][from.x as usize] = Cell::Empty;
                    state.cells[target.y as usize][target.x as usize] = Cell::Unit(unit_id);
                    let unit = state.units.get_mut(&unit_id).unwrap();
                    unit.position = target.clone();
                    unit.acted = true;
                }
            }
        }
    }

    Ok(())
}

/// Завершает ход игрока: проверяет что все юниты сходили, делает снапшот,
/// убивает фабрики без свободных соседних клеток, переходит к следующему игроку.
pub fn end_player_turn(state: &mut GameState, player_id: u8) -> Result<(), GameError> {
    if get_winner(state).is_some() {
        return Err(GameError::GameOver);
    }
    if state.players[state.current_player_index] != player_id {
        return Err(GameError::NotYourTurn);
    }

    // Проверяем что все юниты игрока сходили
    let unacted: Vec<u32> = state
        .units
        .values()
        .filter(|u| u.owner == player_id && !u.acted)
        .map(|u| u.id)
        .collect();
    if !unacted.is_empty() {
        return Err(GameError::InvalidAction(format!(
            "Units have not acted: {:?}",
            unacted
        )));
    }

    // Убиваем фабрики без свободных соседних клеток
    let factory_ids: Vec<u32> = state
        .units
        .values()
        .filter(|u| u.owner == player_id && matches!(u.unit_type, UnitType::Factory))
        .map(|u| u.id)
        .collect();

    for fid in factory_ids {
        let pos = state.units[&fid].position.clone();
        let has_free = (-1i32..=1)
            .flat_map(|dy| (-1i32..=1).map(move |dx| (dx, dy)))
            .filter(|&(dx, dy)| !(dx == 0 && dy == 0))
            .any(|(dx, dy)| {
                apply_delta(&pos, dx, dy, state.config.width, state.config.height)
                    .map(|t| state.cells[t.y as usize][t.x as usize] == Cell::Empty)
                    .unwrap_or(false)
            });
        if !has_free {
            state.cells[pos.y as usize][pos.x as usize] = Cell::Empty;
            state.units.remove(&fid);
        }
    }

    // Снапшот после хода
    state.snapshots.push(Snapshot {
        player_id,
        round: state.round,
        cells: state.cells.clone(),
        units: state.units.clone(),
    });

    // Переход к следующему игроку
    state.current_player_index = (state.current_player_index + 1) % state.players.len();
    if state.current_player_index == 0 {
        state.round += 1;
    }

    // Сбросить acted для юнитов следующего игрока
    let next_player = state.players[state.current_player_index];
    for unit in state.units.values_mut() {
        if unit.owner == next_player {
            unit.acted = false;
        }
    }

    Ok(())
}

// ─── Отключение игрока ────────────────────────────────────────────────────

/// Удаляет игрока: убирает его юниты с поля, исключает из списка активных.
/// Если сейчас был его ход — переходит к следующему игроку.
pub fn remove_player(state: &mut GameState, player_id: u8) {
    // Удаляем все юниты игрока
    let ids: Vec<u32> = state.units.values()
        .filter(|u| u.owner == player_id)
        .map(|u| u.id)
        .collect();
    for id in ids {
        let pos = state.units[&id].position.clone();
        state.cells[pos.y as usize][pos.x as usize] = Cell::Empty;
        state.units.remove(&id);
    }

    // Был ли его ход сейчас?
    let was_current = state.players[state.current_player_index] == player_id;

    // Удаляем из списка активных игроков
    let old_index = state.current_player_index;
    state.players.retain(|&p| p != player_id);

    if state.players.is_empty() {
        state.current_player_index = 0;
        return;
    }

    if was_current {
        // Индекс уже указывает на следующего (после retain элементы сдвинулись)
        // Корректируем на случай выхода за пределы
        state.current_player_index = old_index.min(state.players.len() - 1);
    } else {
        // Скорректировать индекс если удалённый игрок был раньше текущего
        let removed_index = state.players.iter()
            .position(|&p| p == state.players[old_index.saturating_sub(1)])
            .unwrap_or(0);
        // После retain: если удалённый стоял до current_player_index — индекс уменьшился на 1
        // Пересчитываем: ищем текущего игрока по значению
        let current_player = if old_index < state.players.len() + 1 {
            // До удаления текущий игрок имел индекс old_index.
            // После retain, если удалённый был перед ним, индекс уменьшился на 1.
            // Надёжнее найти текущего игрока по значению.
            // Но мы уже потеряли его значение — возьмём из players по скорректированному индексу.
            let _ = removed_index;
            state.players[old_index.min(state.players.len() - 1)]
        } else {
            state.players[0]
        };
        state.current_player_index = state.players
            .iter()
            .position(|&p| p == current_player)
            .unwrap_or(0);
    }
}

/// Восстанавливает состояние из снапшота по индексу.
/// Список активных игроков не восстанавливается (выбывшие остаются выбывшими).
pub fn rollback_to_snapshot(state: &mut GameState, snapshot_index: usize) {
    let snap = state.snapshots[snapshot_index].clone();

    // Список активных игроков сохраняем
    let active_players = state.players.clone();

    state.cells = snap.cells;
    state.round = snap.round;

    // Восстанавливаем только юниты активных игроков
    state.units = snap.units.into_iter()
        .filter(|(_, u)| active_players.contains(&u.owner))
        .collect();

    // Синхронизируем cells: убираем юниты выбывших игроков с поля
    for row in &mut state.cells {
        for cell in row.iter_mut() {
            if let Cell::Unit(uid) = cell {
                if !state.units.contains_key(uid) {
                    *cell = Cell::Empty;
                }
            }
        }
    }

    // Снапшоты обрезаем до текущего
    state.snapshots.truncate(snapshot_index + 1);

    // Следующий ход — игрок после того, чей снапшот восстановлен
    let snap_player = snap.player_id;
    let next_index = active_players
        .iter()
        .position(|&p| p == snap_player)
        .map(|i| (i + 1) % active_players.len())
        .unwrap_or(0);
    state.current_player_index = next_index;
    state.players = active_players;

    // Сбрасываем acted для юнитов следующего игрока
    let next_player = state.players[state.current_player_index];
    for unit in state.units.values_mut() {
        if unit.owner == next_player {
            unit.acted = false;
        }
    }
}

// ─── Тесты ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config(w: u32, h: u32, players: u8) -> GameConfig {
        GameConfig { width: w, height: h, player_count: players }
    }

    #[test]
    fn test_new_game_places_units() {
        let state = new_game(make_config(10, 10, 2));
        // Ровно 2 юнита
        assert_eq!(state.units.len(), 2);
        // Игрок 1 и 2 имеют по одному пехотинцу
        let p1: Vec<_> = state.units.values().filter(|u| u.owner == 1).collect();
        let p2: Vec<_> = state.units.values().filter(|u| u.owner == 2).collect();
        assert_eq!(p1.len(), 1);
        assert_eq!(p2.len(), 1);
        assert!(matches!(p1[0].unit_type, UnitType::Infantry));
        assert!(matches!(p2[0].unit_type, UnitType::Infantry));
        // Позиции различаются
        assert_ne!(p1[0].position, p2[0].position);
        // Клетки на поле соответствуют юнитам
        let pos1 = &p1[0].position;
        let pos2 = &p2[0].position;
        assert_eq!(state.cells[pos1.y as usize][pos1.x as usize], Cell::Unit(p1[0].id));
        assert_eq!(state.cells[pos2.y as usize][pos2.x as usize], Cell::Unit(p2[0].id));
        // Первый ход — раунд 1, игрок 1
        assert_eq!(state.round, 1);
        assert_eq!(state.players[state.current_player_index], 1);
    }

    #[test]
    fn test_new_game_three_players() {
        let state = new_game(make_config(10, 10, 3));
        assert_eq!(state.units.len(), 3);
        // Все позиции уникальны
        let positions: Vec<_> = state.units.values().map(|u| u.position.clone()).collect();
        for i in 0..positions.len() {
            for j in i + 1..positions.len() {
                assert_ne!(positions[i], positions[j]);
            }
        }
    }

    #[test]
    fn test_valid_actions_infantry() {
        let state = new_game(make_config(10, 10, 2));
        // Найти пехотинца игрока 1
        let unit = state.units.values().find(|u| u.owner == 1).unwrap();
        let actions = get_valid_actions(&state, 1, unit.id);
        // (0,0) должен быть
        assert!(actions.iter().any(|a| a.dx == 0 && a.dy == 0));
        // Все действия принадлежат этому юниту
        assert!(actions.iter().all(|a| a.unit_id == unit.id));
        // Нет дублей
        let mut seen = std::collections::HashSet::new();
        for a in &actions {
            assert!(seen.insert((a.dx, a.dy)), "Duplicate action ({}, {})", a.dx, a.dy);
        }
    }

    #[test]
    fn test_valid_actions_infantry_corner() {
        // Пехотинец в левом верхнем углу (0,0) — у него меньше направлений
        let mut state = new_game(make_config(5, 5, 2));
        // Переместим первого пехотинца в угол (0,0)
        let uid = state.units.values().find(|u| u.owner == 1).unwrap().id;
        let old_pos = state.units[&uid].position.clone();
        state.cells[old_pos.y as usize][old_pos.x as usize] = Cell::Empty;
        state.units.get_mut(&uid).unwrap().position = Position { x: 0, y: 0 };
        state.cells[0][0] = Cell::Unit(uid);

        let actions = get_valid_actions(&state, 1, uid);
        // Из угла (0,0): (1,0),(0,1),(1,1) + (0,0) = 4 действия максимум
        // (враг далеко — не атака, только пустые клетки)
        assert!(actions.len() <= 4);
        assert!(actions.iter().any(|a| a.dx == 0 && a.dy == 0));
        // Не должно быть выхода за пределы
        for a in &actions {
            let nx = 0i32 + a.dx;
            let ny = 0i32 + a.dy;
            assert!(nx >= 0 && ny >= 0);
        }
    }

    #[test]
    fn test_valid_actions_factory_no_stay() {
        let mut state = new_game(make_config(10, 10, 2));
        let uid = state.units.values().find(|u| u.owner == 1).unwrap().id;
        // Превратим юнит в фабрику напрямую
        state.units.get_mut(&uid).unwrap().unit_type = UnitType::Factory;
        let actions = get_valid_actions(&state, 1, uid);
        // Фабрика не может стоять на месте
        assert!(!actions.iter().any(|a| a.dx == 0 && a.dy == 0));
        // Все направления ведут на пустые клетки
        assert!(!actions.is_empty());
    }

    #[test]
    fn test_fog_of_war() {
        let state = new_game(make_config(10, 10, 2));
        let view1 = get_visible_state(&state, 1);
        let view2 = get_visible_state(&state, 2);

        // Игрок 1 видит свою позицию
        let u1 = state.units.values().find(|u| u.owner == 1).unwrap();
        let pos = &u1.position;
        assert!(matches!(
            view1.tiles[pos.y as usize][pos.x as usize],
            TileVisibility::Visible(_)
        ));

        // Позиция игрока 2 в тумане для игрока 1 (они далеко друг от друга)
        let u2 = state.units.values().find(|u| u.owner == 2).unwrap();
        let pos2 = &u2.position;
        assert!(matches!(
            view1.tiles[pos2.y as usize][pos2.x as usize],
            TileVisibility::Fog
        ));

        // Каждый видит своих юнитов
        assert!(!view1.my_units.is_empty());
        assert!(!view2.my_units.is_empty());
        assert!(view1.my_units.iter().all(|u| u.owner == 1));
        assert!(view2.my_units.iter().all(|u| u.owner == 2));
    }

    #[test]
    fn test_fog_radius_infantry_vs_fortification() {
        let mut state = new_game(make_config(10, 10, 2));
        let uid = state.units.values().find(|u| u.owner == 1).unwrap().id;
        let pos = state.units[&uid].position.clone();

        // Пехотинец видит радиус 1
        let view = get_visible_state(&state, 1);
        // Клетка на расстоянии 2 — не видна
        if pos.x + 2 < 10 {
            assert!(matches!(
                view.tiles[pos.y as usize][(pos.x + 2) as usize],
                TileVisibility::Fog
            ));
        }

        // Укрепление видит радиус 2
        state.units.get_mut(&uid).unwrap().unit_type =
            UnitType::Fortification { turns_standing: 1 };
        let view2 = get_visible_state(&state, 1);
        if pos.x + 2 < 10 {
            assert!(matches!(
                view2.tiles[pos.y as usize][(pos.x + 2) as usize],
                TileVisibility::Visible(_)
            ));
        }
    }

    #[test]
    fn test_get_winner_none_at_start() {
        let state = new_game(make_config(8, 8, 2));
        assert_eq!(get_winner(&state), None);
    }

    // ── Вспомогательная функция для тестов задачи 4 ──────────────────────

    /// Создаёт состояние 5×5 с двумя пехотинцами рядом: игрок 1 в (1,1), игрок 2 в (3,1)
    fn make_state_two_infantry_adjacent() -> (GameState, u32, u32) {
        let config = make_config(5, 5, 2);
        let mut state = GameState {
            config,
            cells: vec![vec![Cell::Empty; 5]; 5],
            units: HashMap::new(),
            next_unit_id: 1,
            players: vec![1, 2],
            current_player_index: 0,
            round: 1,
            snapshots: Vec::new(),
        };
        let u1 = Unit { id: 1, owner: 1, unit_type: UnitType::Infantry,
                        position: Position { x: 1, y: 1 }, acted: false };
        let u2 = Unit { id: 2, owner: 2, unit_type: UnitType::Infantry,
                        position: Position { x: 3, y: 1 }, acted: false };
        state.cells[1][1] = Cell::Unit(1);
        state.cells[1][3] = Cell::Unit(2);
        state.units.insert(1, u1);
        state.units.insert(2, u2);
        state.next_unit_id = 3;
        (state, 1, 2)
    }

    #[test]
    fn test_infantry_attacks_infantry() {
        let (mut state, u1, u2) = make_state_two_infantry_adjacent();
        // Пехотинец 1 атакует пехотинца 2 (dx=2 — слишком далеко; поставим рядом)
        // Переставим u2 на (2,1) — рядом с u1 в (1,1)
        state.cells[1][3] = Cell::Empty;
        state.units.get_mut(&u2).unwrap().position = Position { x: 2, y: 1 };
        state.cells[1][2] = Cell::Unit(u2);

        let action = Action { unit_id: u1, dx: 1, dy: 0 };
        apply_action(&mut state, 1, u1, action).unwrap();

        // Враг уничтожен
        assert!(!state.units.contains_key(&u2));
        // Атакующий на позиции врага
        assert_eq!(state.units[&u1].position, Position { x: 2, y: 1 });
        assert_eq!(state.cells[1][2], Cell::Unit(u1));
        assert_eq!(state.cells[1][1], Cell::Empty);
        assert!(state.units[&u1].acted);
    }

    #[test]
    fn test_infantry_attacks_fortification_first_hit() {
        let (mut state, u1, u2) = make_state_two_infantry_adjacent();
        // u2 — укрепление
        state.units.get_mut(&u2).unwrap().unit_type = UnitType::Fortification { turns_standing: 1 };
        // Поставим рядом
        state.cells[1][3] = Cell::Empty;
        state.units.get_mut(&u2).unwrap().position = Position { x: 2, y: 1 };
        state.cells[1][2] = Cell::Unit(u2);

        let action = Action { unit_id: u1, dx: 1, dy: 0 };
        apply_action(&mut state, 1, u1, action).unwrap();

        // Атакующий уничтожен
        assert!(!state.units.contains_key(&u1));
        // Укрепление стало пехотинцем, осталось на месте
        assert!(state.units.contains_key(&u2));
        assert!(matches!(state.units[&u2].unit_type, UnitType::Infantry));
        assert_eq!(state.units[&u2].position, Position { x: 2, y: 1 });
        // Клетки корректны
        assert_eq!(state.cells[1][1], Cell::Empty);
        assert_eq!(state.cells[1][2], Cell::Unit(u2));
    }

    #[test]
    fn test_two_infantry_destroy_fortification() {
        // Два пехотинца игрока 1 атакуют одно укрепление игрока 2
        let config = make_config(5, 5, 2);
        let mut state = GameState {
            config,
            cells: vec![vec![Cell::Empty; 5]; 5],
            units: HashMap::new(),
            next_unit_id: 1,
            players: vec![1, 2],
            current_player_index: 0,
            round: 1,
            snapshots: Vec::new(),
        };
        // Два пехотинца игрока 1 — слева и снизу от укрепления
        let fort_pos = Position { x: 2, y: 2 };
        let a1 = Unit { id: 1, owner: 1, unit_type: UnitType::Infantry,
                        position: Position { x: 1, y: 2 }, acted: false };
        let a2 = Unit { id: 2, owner: 1, unit_type: UnitType::Infantry,
                        position: Position { x: 2, y: 3 }, acted: false };
        let fort = Unit { id: 3, owner: 2, unit_type: UnitType::Fortification { turns_standing: 1 },
                          position: fort_pos.clone(), acted: false };
        state.cells[2][1] = Cell::Unit(1);
        state.cells[3][2] = Cell::Unit(2);
        state.cells[2][2] = Cell::Unit(3);
        state.units.insert(1, a1);
        state.units.insert(2, a2);
        state.units.insert(3, fort);
        state.next_unit_id = 4;

        // Первый удар: пехотинец 1 атакует укрепление (погибает, укрепление → пехотинец)
        apply_action(&mut state, 1, 1, Action { unit_id: 1, dx: 1, dy: 0 }).unwrap();
        assert!(!state.units.contains_key(&1));
        assert!(matches!(state.units[&3].unit_type, UnitType::Infantry));

        // Второй удар: пехотинец 2 атакует теперь-пехотинца (враг уничтожен)
        apply_action(&mut state, 1, 2, Action { unit_id: 2, dx: 0, dy: -1 }).unwrap();
        assert!(!state.units.contains_key(&3));
        assert!(state.units.contains_key(&2));
        assert_eq!(state.units[&2].position, fort_pos);
    }

    #[test]
    fn test_fortification_becomes_factory() {
        let mut state = new_game(make_config(8, 8, 2));
        let uid = state.units.values().find(|u| u.owner == 1).unwrap().id;

        // turns_standing 1 → 2 → 3 → Factory
        // Ставим укрепление с turns_standing=2 и делаем (0,0) → станет 3 → Factory
        state.units.get_mut(&uid).unwrap().unit_type = UnitType::Fortification { turns_standing: 2 };

        apply_action(&mut state, 1, uid, Action { unit_id: uid, dx: 0, dy: 0 }).unwrap();
        assert!(matches!(
            state.units[&uid].unit_type,
            UnitType::Fortification { turns_standing: 3 }
        ));
        assert!(state.units[&uid].acted);

        // Завершаем ход игрока 1, даём ход игроку 2
        let u2 = state.units.values().find(|u| u.owner == 2).unwrap().id;
        apply_action(&mut state, 1, uid, Action { unit_id: uid, dx: 0, dy: 0 })
            .expect_err("UnitAlreadyActed");
        end_player_turn(&mut state, 1).unwrap();

        // Ход игрока 2 — он делает свой ход и завершает
        apply_action(&mut state, 2, u2, Action { unit_id: u2, dx: 0, dy: 0 }).unwrap();
        end_player_turn(&mut state, 2).unwrap();

        // Снова ход игрока 1 — делаем (0,0) → turns_standing 3 → Factory
        state.units.get_mut(&uid).unwrap().acted = false;
        apply_action(&mut state, 1, uid, Action { unit_id: uid, dx: 0, dy: 0 }).unwrap();
        assert!(matches!(state.units[&uid].unit_type, UnitType::Factory));
    }

    #[test]
    fn test_factory_produces_unit() {
        let mut state = new_game(make_config(8, 8, 2));
        let uid = state.units.values().find(|u| u.owner == 1).unwrap().id;
        state.units.get_mut(&uid).unwrap().unit_type = UnitType::Factory;
        let pos = state.units[&uid].position.clone();

        // Производим пехотинца вправо
        let count_before = state.units.len();
        apply_action(&mut state, 1, uid, Action { unit_id: uid, dx: 1, dy: 0 }).unwrap();

        assert_eq!(state.units.len(), count_before + 1);
        let new_unit = state.units.values()
            .find(|u| u.owner == 1 && u.id != uid)
            .unwrap();
        assert!(matches!(new_unit.unit_type, UnitType::Infantry));
        assert_eq!(new_unit.position, Position { x: pos.x + 1, y: pos.y });
        assert!(new_unit.acted); // не ходит в ход появления
        assert_eq!(state.cells[pos.y as usize][(pos.x + 1) as usize], Cell::Unit(new_unit.id));
    }

    #[test]
    fn test_factory_dies_no_space() {
        // Окружаем фабрику своими юнитами со всех сторон
        let config = make_config(5, 5, 2);
        let mut state = GameState {
            config,
            cells: vec![vec![Cell::Empty; 5]; 5],
            units: HashMap::new(),
            next_unit_id: 1,
            players: vec![1, 2],
            current_player_index: 0,
            round: 1,
            snapshots: Vec::new(),
        };
        // Фабрика в центре (2,2)
        let factory = Unit { id: 1, owner: 1, unit_type: UnitType::Factory,
                              position: Position { x: 2, y: 2 }, acted: true };
        state.cells[2][2] = Cell::Unit(1);
        state.units.insert(1, factory);

        // Заполняем все соседние клетки юнитами игрока 1
        let mut next_id = 2u32;
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                if dx == 0 && dy == 0 { continue; }
                let nx = (2i32 + dx) as u32;
                let ny = (2i32 + dy) as u32;
                let blocker = Unit { id: next_id, owner: 1, unit_type: UnitType::Infantry,
                                     position: Position { x: nx, y: ny }, acted: true };
                state.cells[ny as usize][nx as usize] = Cell::Unit(next_id);
                state.units.insert(next_id, blocker);
                next_id += 1;
            }
        }

        // Игрок 2 — один пехотинец далеко
        let u2 = Unit { id: next_id, owner: 2, unit_type: UnitType::Infantry,
                         position: Position { x: 0, y: 4 }, acted: false };
        state.cells[4][0] = Cell::Unit(next_id);
        state.units.insert(next_id, u2);
        next_id += 1;
        state.next_unit_id = next_id;

        // end_player_turn для игрока 1: фабрика должна погибнуть
        end_player_turn(&mut state, 1).unwrap();

        assert!(!state.units.contains_key(&1), "Factory should have died");
        assert_eq!(state.cells[2][2], Cell::Empty);
    }

    #[test]
    fn test_end_turn_advances_player_and_round() {
        let mut state = new_game(make_config(8, 8, 2));
        let u1 = state.units.values().find(|u| u.owner == 1).unwrap().id;
        let u2 = state.units.values().find(|u| u.owner == 2).unwrap().id;

        // Раунд 1, ход игрока 1
        assert_eq!(state.round, 1);
        assert_eq!(state.players[state.current_player_index], 1);

        apply_action(&mut state, 1, u1, Action { unit_id: u1, dx: 0, dy: 0 }).unwrap();
        end_player_turn(&mut state, 1).unwrap();

        // Теперь ход игрока 2, раунд 1
        assert_eq!(state.round, 1);
        assert_eq!(state.players[state.current_player_index], 2);
        assert!(!state.units[&u2].acted);

        apply_action(&mut state, 2, u2, Action { unit_id: u2, dx: 0, dy: 0 }).unwrap();
        end_player_turn(&mut state, 2).unwrap();

        // Раунд 2, ход игрока 1
        assert_eq!(state.round, 2);
        assert_eq!(state.players[state.current_player_index], 1);
        assert!(!state.units[&u1].acted);
    }

    #[test]
    fn test_get_winner_when_one_player_left() {
        let mut state = new_game(make_config(5, 5, 2));
        // Удалить всех юнитов игрока 2
        let u2_ids: Vec<u32> = state.units.values()
            .filter(|u| u.owner == 2)
            .map(|u| u.id)
            .collect();
        for id in u2_ids {
            let pos = state.units[&id].position.clone();
            state.cells[pos.y as usize][pos.x as usize] = Cell::Empty;
            state.units.remove(&id);
        }
        assert_eq!(get_winner(&state), Some(1));
    }

    #[test]
    fn test_fortification_move_becomes_infantry() {
        let mut state = new_game(make_config(8, 8, 2));
        let uid = state.units.values().find(|u| u.owner == 1).unwrap().id;
        let orig_pos = state.units[&uid].position.clone();

        // Стать укреплением
        state.units.get_mut(&uid).unwrap().unit_type = UnitType::Fortification { turns_standing: 1 };

        // Двинуться → стать пехотинцем и переместиться
        apply_action(&mut state, 1, uid, Action { unit_id: uid, dx: 1, dy: 0 }).unwrap();

        assert!(matches!(state.units[&uid].unit_type, UnitType::Infantry));
        assert_eq!(state.units[&uid].position, Position { x: orig_pos.x + 1, y: orig_pos.y });
        assert_eq!(state.cells[orig_pos.y as usize][orig_pos.x as usize], Cell::Empty);
        assert_eq!(
            state.cells[orig_pos.y as usize][(orig_pos.x + 1) as usize],
            Cell::Unit(uid)
        );
    }

    // ── Тесты задачи 5 ───────────────────────────────────────────────────

    #[test]
    fn test_remove_player_removes_units_and_cells() {
        let mut state = new_game(make_config(8, 8, 2));
        let u2_pos = state.units.values().find(|u| u.owner == 2).unwrap().position.clone();

        remove_player(&mut state, 2);

        // Юниты игрока 2 удалены
        assert!(state.units.values().all(|u| u.owner != 2));
        // Клетка очищена
        assert_eq!(state.cells[u2_pos.y as usize][u2_pos.x as usize], Cell::Empty);
        // Игрок 2 удалён из списка
        assert!(!state.players.contains(&2));
    }

    #[test]
    fn test_remove_player_during_turn() {
        let mut state = new_game(make_config(8, 8, 2));
        // Сейчас ход игрока 1
        assert_eq!(state.players[state.current_player_index], 1);

        remove_player(&mut state, 1);

        // Игрок 1 удалён, остался только игрок 2
        assert!(!state.players.contains(&1));
        // Ход перешёл к игроку 2 (единственному оставшемуся)
        let current = state.players[state.current_player_index];
        assert_eq!(current, 2);
    }

    #[test]
    fn test_remove_player_not_during_turn() {
        let mut state = new_game(make_config(8, 8, 3));
        // Сейчас ход игрока 1, удаляем игрока 3 (не его ход)
        assert_eq!(state.players[state.current_player_index], 1);

        remove_player(&mut state, 3);

        // Ход по-прежнему у игрока 1
        assert_eq!(state.players[state.current_player_index], 1);
        assert!(!state.players.contains(&3));
    }

    #[test]
    fn test_rollback_restores_state() {
        let mut state = new_game(make_config(8, 8, 2));
        let u1 = state.units.values().find(|u| u.owner == 1).unwrap().id;
        let u2 = state.units.values().find(|u| u.owner == 2).unwrap().id;
        let orig_pos1 = state.units[&u1].position.clone();

        // Игрок 1 двигается и завершает ход (снапшот #0)
        apply_action(&mut state, 1, u1, Action { unit_id: u1, dx: 1, dy: 0 }).unwrap();
        end_player_turn(&mut state, 1).unwrap();
        let pos_after_move = state.units[&u1].position.clone();
        assert_ne!(pos_after_move, orig_pos1);

        // Игрок 2 делает ход (снапшот #1)
        apply_action(&mut state, 2, u2, Action { unit_id: u2, dx: 0, dy: 0 }).unwrap();
        end_player_turn(&mut state, 2).unwrap();

        // Откатываемся к снапшоту #0 (после хода игрока 1)
        rollback_to_snapshot(&mut state, 0);

        // Юнит игрока 1 на позиции после его хода (снапшот взят после хода)
        assert_eq!(state.units[&u1].position, pos_after_move);
        // Ход теперь у игрока 2 (следующий после игрока 1)
        assert_eq!(state.players[state.current_player_index], 2);
        // Снапшотов стало 1
        assert_eq!(state.snapshots.len(), 1);
    }

    #[test]
    fn test_rollback_excludes_removed_player() {
        let mut state = new_game(make_config(8, 8, 2));
        let u1 = state.units.values().find(|u| u.owner == 1).unwrap().id;
        let u2 = state.units.values().find(|u| u.owner == 2).unwrap().id;

        // Игрок 1 ходит, снапшот #0
        apply_action(&mut state, 1, u1, Action { unit_id: u1, dx: 1, dy: 0 }).unwrap();
        end_player_turn(&mut state, 1).unwrap();

        // Игрок 2 ходит, снапшот #1
        apply_action(&mut state, 2, u2, Action { unit_id: u2, dx: 0, dy: 0 }).unwrap();
        end_player_turn(&mut state, 2).unwrap();

        // Игрок 2 отключился — удаляем его
        remove_player(&mut state, 2);

        // Откат к снапшоту #0 (когда игрок 2 ещё был, но мы его не восстанавливаем)
        rollback_to_snapshot(&mut state, 0);

        // Юниты игрока 2 не восстановлены (он удалён)
        assert!(state.units.values().all(|u| u.owner != 2));
        // Игрок 2 не вернулся в список
        assert!(!state.players.contains(&2));
    }

    #[test]
    fn test_winner_after_disconnect() {
        let mut state = new_game(make_config(8, 8, 2));

        // Игрок 2 отключился — удаляем его юниты
        remove_player(&mut state, 2);

        // Победитель — игрок 1
        assert_eq!(get_winner(&state), Some(1));
    }
}
